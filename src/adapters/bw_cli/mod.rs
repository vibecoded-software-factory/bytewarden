//! [`crate::ports::VaultPort`] implementation that shells out to the
//! Bitwarden CLI (`bw`).
//!
//! ## Sub-modules
//!
//! * [`process`] — wraps `Command` with timeout / output helpers.
//! * [`codec`]   — base64 encoding (used to pass JSON payloads to `bw`).
//! * [`json`]    — small JSON helpers shared across read paths.

pub mod codec;
pub mod json;
pub mod process;

use crate::domain::{
    Collection, Folder, Item, LoginOutcome, Organization, TwoFactorMethod, VaultInfo, VaultStatus,
};
use crate::ports::{BwError, ParallelSessionData, VaultPort};
use serde_json::json;
use std::sync::Arc;
use zeroize::Zeroizing;

use codec::base64_encode;
use json::opt_str;
use process::{
    BW_PASSWORD_ENV, bw_run, bw_run_timeout, bw_run_with_password,
    bw_run_with_password_and_stdin_timeout, bw_run_with_password_timeout, bw_run_with_session,
    bw_run_with_session_timeout, stderr_str, stdout_str,
};

/// Classifies a non-zero `bw` exit into [`BwError::Exit`], carrying the
/// process's stderr verbatim (what the user needs to read) plus the exit
/// code when one is available. The single "`bw` failed" constructor.
fn bw_exit(out: &std::process::Output) -> BwError {
    BwError::exit(stderr_str(out), out.status.code())
}

// ── Timeout budgets ──────────────────────────────────────────────────────
//
// Numbers are deliberate, not magic: each one is the longest a healthy
// network round-trip should plausibly take, plus a few seconds of slack
// for users on flaky connections. A timeout firing means "give up
// gracefully and let the user retry" — never "silently mask a problem".

/// `bw status` — local-only metadata read; bounded so a stuck CLI does
/// not delay the splash screen indefinitely.
const STATUS_TIMEOUT: u64 = 4;
/// Cheap online ops: setting the server URL, logout (which talks to
/// the backend to revoke the device token).
const QUICK_NET_TIMEOUT: u64 = 10;
/// Auth flows: master-password login, API-key login, OTP login. MFA
/// adds a roundtrip so we keep this generous.
const AUTH_TIMEOUT: u64 = 30;
/// Per-item online operations: create / edit / delete / restore item,
/// folder CRUD, send_text, HIBP exposed check, get_item_json.
const ITEM_OP_TIMEOUT: u64 = 15;
/// Vault sync — list of items can be large and the server may be slow
/// at peak times.
const SYNC_TIMEOUT: u64 = 30;
/// Bulk operations that can legitimately move several megabytes of
/// data: full export/import, attachment up/download.
const BULK_TIMEOUT: u64 = 60;
/// Fallback wall-clock budget for `bw list items` / `bw list items
/// --trash` when the caller didn't override it. Decrypts every record
/// and serializes to JSON, which on large vaults can take a few
/// seconds — but never minutes once the pipe-buffer deadlock in
/// `process::wait_with_timeout` is fixed. We default to 60 s, well
/// above any realistic decrypt cost while still bounding a genuinely
/// wedged child. Override via `list_items_timeout_secs` in
/// `config.toml` if you ever hit the ceiling.
const DEFAULT_LIST_ITEMS_TIMEOUT: u64 = 60;

/// Patterns that indicate `bw login` is challenging us with the
/// **permanent** second factor enrolled on the user's account
/// (Authenticator app, YubiKey, Email 2FA, …).
///
/// Resolved by [`VaultPort::login_with_two_factor`], which passes
/// `--method N` to `bw login` so the CLI knows which factor to use.
///
/// Each pattern is matched as a case-insensitive substring against
/// the combined stdout + stderr of the failed `login` invocation.
const TWO_FACTOR_PROMPT_PATTERNS: &[&str] = &[
    "two-step login",
    "two-step token",
    "authenticator app",
    "additional authentication",
];

/// Patterns that indicate `bw login` is asking for a one-time
/// **device-verification** code (the e-mailed OTP that fires when bw
/// doesn't recognise the source device).
///
/// Resolved by [`VaultPort::login_with_otp`] — no `--method` flag is
/// involved; bw matches the prompt automatically.
///
/// The list is checked **after** [`TWO_FACTOR_PROMPT_PATTERNS`] so a
/// 2FA prompt that happens to mention "verification code" is routed
/// down the right path.
const DEVICE_VERIFICATION_PROMPT_PATTERNS: &[&str] = &[
    "new device",
    "device verification",
    "verification required",
    "verification code",
    "enter otp",
];

/// Classifies a failed `bw login` output into one of the interactive
/// outcomes (or `None` if the failure is just bad credentials).
///
/// The two-factor list is consulted first — `"verification code"` is
/// generic enough to appear inside 2FA prompts too, and we'd rather
/// miss a device verification (the user can retry) than misroute a
/// 2FA prompt down the no-method-flag path (which silently fails).
fn combined_outcome(text: &str) -> Option<LoginOutcome> {
    let lower = text.to_lowercase();
    if TWO_FACTOR_PROMPT_PATTERNS.iter().any(|p| lower.contains(p)) {
        return Some(LoginOutcome::NeedsTwoFactor);
    }
    if DEVICE_VERIFICATION_PROMPT_PATTERNS
        .iter()
        .any(|p| lower.contains(p))
    {
        return Some(LoginOutcome::NeedsDeviceVerification);
    }
    None
}

/// Vault adapter that drives the `bw` CLI.
///
/// Holds the session key returned by `bw unlock` / `bw login --raw` so
/// later `bw` invocations can be fed the same key via the
/// `BW_SESSION` environment variable (see
/// [`process::bw_run_with_session_timeout`]). The env-var path keeps
/// the key out of `argv`, mirroring the master-password hygiene.
///
/// The session key is wrapped in [`Zeroizing`] so the underlying bytes
/// are overwritten with zeroes when the field is dropped — either
/// because the adapter is dropped or because [`Self::lock`] /
/// [`Self::logout`] reset it to `None`. That closes the window in
/// which a heap dump or a swap-out could leak the unlocked-vault
/// authorisation token after the user has already locked.
///
/// Wrapped in [`Arc`] so [`Self::parallel_session_data`] can hand
/// each worker thread a cheap clone that shares **the same**
/// underlying allocation — instead of N deep copies of the secret in
/// memory simultaneously. The `Zeroizing` wrapper still fires when
/// the last `Arc` reference is dropped, so the zero-on-drop
/// guarantee is intact; we just narrow the heap-dump exposure window
/// to a single copy regardless of parallelism.
#[derive(Clone)]
pub struct BwCliAdapter {
    session_key: Option<Arc<Zeroizing<String>>>,
    /// Wall-clock budget applied to `bw list items` and `bw list items
    /// --trash`. Sourced from [`crate::ports::UserSettings::list_items_timeout_secs`]
    /// at boot via [`Self::with_list_items_timeout`]; falls back to
    /// [`DEFAULT_LIST_ITEMS_TIMEOUT`] when the constructor is used
    /// without an explicit override (tests, defaults).
    list_items_timeout: u64,
}

impl BwCliAdapter {
    /// Creates a new adapter, reading any pre-existing `BW_SESSION`
    /// from the environment.
    ///
    /// Equivalent to `Self::new_with(None)` — kept for callers that do
    /// not need the seed argument.
    pub fn new() -> Self {
        Self::new_with(None)
    }

    /// Creates a new adapter, optionally seeded with a session key the
    /// caller already has in hand (typically the keep-session file
    /// loaded by `main`).
    ///
    /// Resolution order:
    /// 1. `seed_key` if `Some` and non-empty.
    /// 2. `$BW_SESSION` from the environment, if set and non-empty.
    /// 3. `None` — the user will have to log in.
    ///
    /// Taking the seed as an argument lets `main` avoid mutating the
    /// process environment (which is `unsafe` from edition 2024 onwards
    /// and brittle in the presence of threads). The adapter still
    /// validates the key by calling [`VaultPort::status`] / listing
    /// items at first use; a stale seed simply falls back to login.
    pub fn new_with(seed_key: Option<Zeroizing<String>>) -> Self {
        let session_key = seed_key
            .filter(|s| !s.is_empty())
            .or_else(|| {
                std::env::var("BW_SESSION")
                    .ok()
                    .map(|s| Zeroizing::new(s.trim().to_string()))
                    .filter(|s| !s.is_empty())
            })
            .map(Arc::new);
        Self {
            session_key,
            list_items_timeout: DEFAULT_LIST_ITEMS_TIMEOUT,
        }
    }

    /// Overrides the `bw list items` timeout (seconds). Builder-style
    /// so the composition root can read user settings and chain it
    /// onto the constructor without a second mutation step. A value of
    /// `0` is treated as "use the default" — never disable the
    /// timeout, since an unbounded wait would let a wedged child hang
    /// the TUI forever.
    pub fn with_list_items_timeout(mut self, secs: u64) -> Self {
        if secs > 0 {
            self.list_items_timeout = secs;
        }
        self
    }

    /// Returns the current session key or a "Vault is locked" error,
    /// suitable for use as a `?`-able preamble in vault operations.
    fn session(&self) -> Result<&str, BwError> {
        self.session_key
            .as_ref()
            .map(|z| z.as_str())
            .ok_or_else(|| BwError::Internal("Vault is locked".to_string()))
    }
}

impl Default for BwCliAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl VaultPort for BwCliAdapter {
    // ── Authentication ────────────────────────────────────────────────────

    fn status(&mut self) -> Result<VaultInfo, BwError> {
        // `bw status` is local-only and should be fast, but Node startup
        // can be slow — bound it to 4s, then fall through to the login flow.
        let out = bw_run_timeout(&["status"], STATUS_TIMEOUT)?;
        let val: serde_json::Value = serde_json::from_str(&stdout_str(&out))
            .map_err(|e| BwError::InvalidJson(format!("bw status JSON parse error: {e}")))?;

        let status = match val["status"].as_str().unwrap_or("unauthenticated") {
            "unlocked" => VaultStatus::Unlocked,
            "locked" => VaultStatus::Locked,
            _ => VaultStatus::Unauthenticated,
        };
        Ok(VaultInfo {
            status,
            user_email: opt_str(&val, "userEmail"),
            last_sync: opt_str(&val, "lastSync"),
            server_url: opt_str(&val, "serverUrl"),
        })
    }

    fn login(&mut self, email: &str, password: &str) -> LoginOutcome {
        // Password is fed via $BW_PASS_INPUT, not argv, so it is invisible
        // in `ps aux`. See `process::bw_run_with_password`.
        let out = match bw_run_with_password_timeout(
            &["login", email, "--passwordenv", BW_PASSWORD_ENV, "--raw"],
            password,
            AUTH_TIMEOUT,
        ) {
            Ok(o) => o,
            Err(e) => return LoginOutcome::Failed(e.to_string()),
        };
        if out.status.success() {
            let key = stdout_str(&out);
            // Stash the zeroizing copy first so the long-lived storage
            // is the protected one; the LoginOutcome value is discarded
            // by every current caller.
            self.session_key = Some(Arc::new(Zeroizing::new(key.clone())));
            return LoginOutcome::Success(key);
        }
        // `bw` exits with an error when it needs an interactive code but
        // stdin is empty; detect it via the prompt text on stdout/stderr.
        // The follow-up classification (device-verification vs permanent
        // 2FA) lives in [`combined_outcome`].
        let combined = format!("{}\n{}", stdout_str(&out), stderr_str(&out));
        match combined_outcome(&combined) {
            Some(o) => o,
            None => LoginOutcome::Failed(stderr_str(&out)),
        }
    }

    fn login_with_otp(
        &mut self,
        email: &str,
        password: &str,
        otp: &str,
    ) -> Result<String, BwError> {
        // The OTP is fed via stdin instead of `--code <otp>` so it
        // never appears in `argv` (and therefore never in `ps`). That
        // requires dropping `--nointeraction` for this single call so
        // bw's interactive prompt reads the code off stdin. We
        // intentionally **do not** prepend the global `--nointeraction`
        // here.
        //
        // A trailing newline is required — `inquirer` (the prompt
        // library bw uses) treats it as the line terminator.
        // Wrap the `format!` result so the intermediate buffer
        // carrying the verification code is overwritten with zeros
        // when it goes out of scope, instead of being freed-but-not-
        // scrubbed by the allocator.
        let stdin_payload = Zeroizing::new(format!("{otp}\n"));
        let out = bw_run_with_password_and_stdin_timeout(
            &["login", email, "--passwordenv", BW_PASSWORD_ENV, "--raw"],
            password,
            &stdin_payload,
            AUTH_TIMEOUT,
        )?;
        if out.status.success() {
            let key = stdout_str(&out);
            self.session_key = Some(Arc::new(Zeroizing::new(key.clone())));
            Ok(key)
        } else {
            Err(bw_exit(&out))
        }
    }

    fn login_with_two_factor(
        &mut self,
        email: &str,
        password: &str,
        code: &str,
        method: TwoFactorMethod,
    ) -> Result<String, BwError> {
        // Same stdin-fed approach as `login_with_otp` — the code stays
        // out of argv/`ps`. The `--method N` flag tells bw which
        // factor to validate (`0` Authenticator, `1` Email, `3`
        // YubiKey).
        //
        // bw's argument parser does not accept `--method` together with
        // `--nointeraction`, so the global flag is dropped here just
        // like in the device-verification path.
        let method_str = method.as_u8().to_string();
        // Same zeroization rationale as `login_with_otp` for the
        // stdin payload — the 2FA code is a short-lived secret that
        // we still don't want lingering in the heap after the call
        // returns.
        let stdin_payload = Zeroizing::new(format!("{code}\n"));
        let out = bw_run_with_password_and_stdin_timeout(
            &[
                "login",
                email,
                "--passwordenv",
                BW_PASSWORD_ENV,
                "--method",
                &method_str,
                "--raw",
            ],
            password,
            &stdin_payload,
            AUTH_TIMEOUT,
        )?;
        if out.status.success() {
            let key = stdout_str(&out);
            self.session_key = Some(Arc::new(Zeroizing::new(key.clone())));
            Ok(key)
        } else {
            Err(bw_exit(&out))
        }
    }

    fn login_with_api_key(&mut self) -> Result<(), BwError> {
        // `bw login --apikey` consumes BW_CLIENTID and BW_CLIENTSECRET
        // from the parent environment. We do not need to forward them
        // explicitly — std::process::Command inherits the parent env
        // by default.
        let out = bw_run_timeout(&["login", "--apikey"], AUTH_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn login_with_sso(&mut self) -> Result<(), BwError> {
        // `bw login --sso` opens the user's browser and blocks until
        // the callback arrives. Intentionally **no timeout** — the user
        // can take as long as they need to authenticate in the browser.
        // The TUI looks frozen during that window; there's no way
        // around it without extra plumbing (we'd have to fork bw and
        // stream its progress).
        let out = bw_run(&["login", "--sso"])?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn unlock(&mut self, password: &str) -> Result<String, BwError> {
        // Unlock is a local crypto operation — no network involved, no
        // timeout needed.
        let out = bw_run_with_password(
            &["unlock", "--passwordenv", BW_PASSWORD_ENV, "--raw"],
            password,
        )?;
        if out.status.success() {
            let key = stdout_str(&out);
            self.session_key = Some(Arc::new(Zeroizing::new(key.clone())));
            Ok(key)
        } else {
            Err(bw_exit(&out))
        }
    }

    fn lock(&mut self) {
        // Local-only: clears the cached symmetric key.
        let _ = bw_run(&["lock"]);
        self.session_key = None;
    }

    fn logout(&mut self) -> Result<(), BwError> {
        let out = bw_run_timeout(&["logout"], QUICK_NET_TIMEOUT)?;
        // Drop the session even if the CLI complained — we cannot use
        // a key whose account just got removed locally.
        self.session_key = None;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn session_key(&self) -> Option<&str> {
        self.session_key.as_ref().map(|z| z.as_str())
    }

    // ── Configuration ─────────────────────────────────────────────────────

    fn set_server(&mut self, url: &str) -> Result<(), BwError> {
        // Talks to the backend to validate the URL.
        let out = bw_run_timeout(&["config", "server", url], QUICK_NET_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    // ── Vault data ────────────────────────────────────────────────────────

    fn list_items(&mut self) -> Result<Vec<Item>, BwError> {
        // Local-only — reads the cached vault populated by the last sync.
        // Decrypts every record and serializes to JSON; large vaults
        // can take several seconds, so we use the configurable
        // `list_items_timeout` (default 60 s) instead of the generic
        // 10 s local-op fallback that fires before the legitimate
        // decrypt completes.
        //
        // Session key is fed via `BW_SESSION` env var (see
        // `bw_run_with_session_timeout`) instead of the equivalent
        // `--session <key>` flag — the env-var path keeps the key
        // out of `argv` and `ps aux`. Same hygiene as the master-
        // password path.
        let session = self.session()?.to_string();
        let timeout = self.list_items_timeout;
        let out = bw_run_with_session_timeout(&["list", "items"], &session, timeout)?;
        if out.status.success() {
            serde_json::from_str::<Vec<Item>>(&stdout_str(&out))
                .map_err(|e| BwError::InvalidJson(format!("Error parsing items JSON: {e}")))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn list_trash(&mut self) -> Result<Vec<Item>, BwError> {
        // Same decrypt cost as `list_items` — see that method for the
        // timeout rationale.
        let session = self.session()?.to_string();
        let timeout = self.list_items_timeout;
        let out = bw_run_with_session_timeout(&["list", "items", "--trash"], &session, timeout)?;
        if out.status.success() {
            serde_json::from_str::<Vec<Item>>(&stdout_str(&out))
                .map_err(|e| BwError::InvalidJson(format!("Error parsing trash JSON: {e}")))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn sync(&mut self) -> Result<(), BwError> {
        let session = self.session()?.to_string();
        let out = bw_run_with_session_timeout(&["sync"], &session, SYNC_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    // ── Single-field reads ────────────────────────────────────────────────

    fn get_totp(&mut self, item_id: &str) -> Result<String, BwError> {
        // TOTP is computed locally from the cached seed — no network.
        let session = self.session()?.to_string();
        let out = bw_run_with_session(&["get", "totp", item_id], &session)?;
        if out.status.success() {
            Ok(stdout_str(&out))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn get_item_json(&mut self, item_id: &str) -> Result<Zeroizing<String>, BwError> {
        // Local-only — reads the cached item.
        //
        // The returned JSON contains the item's plaintext credentials
        // (login password, TOTP seed, SSH private key, card CVV, …).
        // Wrap it in `Zeroizing` so the buffer is overwritten with
        // zeroes when the caller is done with it, instead of being
        // freed-but-not-scrubbed by the allocator.
        let session = self.session()?.to_string();
        let out = bw_run_with_session(&["get", "item", item_id], &session)?;
        if out.status.success() {
            Ok(Zeroizing::new(
                String::from_utf8_lossy(&out.stdout).to_string(),
            ))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn check_exposed(&mut self, item_id: &str) -> Result<u32, BwError> {
        // Network: bw queries HaveIBeenPwned (k-anonymity API).
        let session = self.session()?.to_string();
        let out =
            bw_run_with_session_timeout(&["get", "exposed", item_id], &session, ITEM_OP_TIMEOUT)?;
        if !out.status.success() {
            return Err(bw_exit(&out));
        }
        // bw prints just an integer to stdout — parse it strictly so
        // any unexpected output bubbles up as an error rather than a
        // silent zero.
        let text = stdout_str(&out);
        text.parse::<u32>()
            .map_err(|_| BwError::Shape(format!("Unexpected `bw get exposed` output: {text}")))
    }

    // ── Item CRUD ─────────────────────────────────────────────────────────

    fn create_item(&mut self, item_json: &str) -> Result<Item, BwError> {
        let session = self.session()?.to_string();
        let encoded = base64_encode(item_json);
        let out =
            bw_run_with_session_timeout(&["create", "item", &encoded], &session, ITEM_OP_TIMEOUT)?;
        if out.status.success() {
            serde_json::from_str::<Item>(&stdout_str(&out))
                .map_err(|e| BwError::InvalidJson(format!("Error parsing created item: {e}")))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn edit_item(&mut self, item_id: &str, item_json: &str) -> Result<Item, BwError> {
        let session = self.session()?.to_string();
        let encoded = base64_encode(item_json);
        let out = bw_run_with_session_timeout(
            &["edit", "item", item_id, &encoded],
            &session,
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            serde_json::from_str::<Item>(&stdout_str(&out))
                .map_err(|e| BwError::InvalidJson(format!("Error parsing edited item: {e}")))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn delete_item(&mut self, item_id: &str, permanent: bool) -> Result<(), BwError> {
        let session = self.session()?.to_string();
        let mut args: Vec<&str> = vec!["delete", "item", item_id];
        if permanent {
            args.push("--permanent");
        }
        let out = bw_run_with_session_timeout(&args, &session, ITEM_OP_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn restore_item(&mut self, item_id: &str) -> Result<(), BwError> {
        let session = self.session()?.to_string();
        let out =
            bw_run_with_session_timeout(&["restore", "item", item_id], &session, ITEM_OP_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    // ── Folder CRUD ───────────────────────────────────────────────────────

    fn list_folders(&mut self) -> Result<Vec<Folder>, BwError> {
        // Local-only — reads the cached folder list.
        let session = self.session()?.to_string();
        let out = bw_run_with_session(&["list", "folders"], &session)?;
        if out.status.success() {
            serde_json::from_str::<Vec<Folder>>(&stdout_str(&out))
                .map_err(|e| BwError::InvalidJson(format!("Error parsing folders JSON: {e}")))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn create_folder(&mut self, name: &str) -> Result<Folder, BwError> {
        let session = self.session()?.to_string();
        let payload = json!({ "name": name }).to_string();
        let encoded = base64_encode(&payload);
        let out = bw_run_with_session_timeout(
            &["create", "folder", &encoded],
            &session,
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            serde_json::from_str::<Folder>(&stdout_str(&out))
                .map_err(|e| BwError::InvalidJson(format!("Error parsing created folder: {e}")))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn edit_folder(&mut self, folder_id: &str, name: &str) -> Result<Folder, BwError> {
        let session = self.session()?.to_string();
        let payload = json!({ "name": name }).to_string();
        let encoded = base64_encode(&payload);
        let out = bw_run_with_session_timeout(
            &["edit", "folder", folder_id, &encoded],
            &session,
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            serde_json::from_str::<Folder>(&stdout_str(&out))
                .map_err(|e| BwError::InvalidJson(format!("Error parsing edited folder: {e}")))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn delete_folder(&mut self, folder_id: &str) -> Result<(), BwError> {
        let session = self.session()?.to_string();
        let out = bw_run_with_session_timeout(
            &["delete", "folder", folder_id],
            &session,
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn export(&mut self, format: &str, output_path: &str) -> Result<(), BwError> {
        // Bulk: a full vault export can run for several seconds on
        // large accounts.
        let session = self.session()?.to_string();
        let out = bw_run_with_session_timeout(
            &["export", "--format", format, "--output", output_path],
            &session,
            BULK_TIMEOUT,
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn get_fingerprint(&mut self) -> Result<String, BwError> {
        // Local-only — derived from the cached public key.
        let session = self.session()?.to_string();
        let out = bw_run_with_session(&["get", "fingerprint", "me"], &session)?;
        if out.status.success() {
            Ok(stdout_str(&out))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn move_item(
        &mut self,
        item_id: &str,
        organization_id: &str,
        collection_ids: &[String],
    ) -> Result<(), BwError> {
        // bw expects the collection-ids list as a base64-encoded
        // JSON array, mirroring how `bw create item` takes its
        // payload. We do the encoding in-process so the
        // command line stays free of credential-shaped strings
        // (matters less here than for passwords, but consistency
        // keeps the adapter simple).
        let session = self.session()?.to_string();
        let json = serde_json::to_string(collection_ids).map_err(|e| {
            BwError::InvalidJson(format!("Could not serialize collection ids: {e}"))
        })?;
        let encoded = base64_encode(&json);
        let out = bw_run_with_session_timeout(
            &["move", item_id, organization_id, &encoded],
            &session,
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn list_import_formats(&mut self) -> Result<Vec<String>, BwError> {
        // Local-only — bw prints the static list it was compiled
        // with. No session needed.
        let out = bw_run(&["import", "--formats"])?;
        if !out.status.success() {
            return Err(bw_exit(&out));
        }
        let stdout = stdout_str(&out);
        // bw's output has changed across versions (sometimes a bare
        // list, sometimes a table with headings). We extract the
        // first identifier-like token from every line, then dedup
        // and keep the original order.
        let mut seen = std::collections::HashSet::new();
        let mut out: Vec<String> = Vec::new();
        for line in stdout.lines() {
            let token: String = line
                .trim()
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            // Heuristic: real format identifiers are `[a-z][a-z0-9]+`
            // — skip CLI banners ("Available formats:") and section
            // markers, which would either start with uppercase or
            // be too short to be a valid format.
            if token.len() >= 4
                && token.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                && seen.insert(token.clone())
            {
                out.push(token);
            }
        }
        if out.is_empty() {
            return Err(BwError::Shape(
                "bw import --formats returned no usable formats".into(),
            ));
        }
        Ok(out)
    }

    fn import(&mut self, format: &str, input_path: &str) -> Result<(), BwError> {
        // Bulk: import can upload thousands of items in one go.
        let session = self.session()?.to_string();
        let out =
            bw_run_with_session_timeout(&["import", format, input_path], &session, BULK_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn upload_attachment(&mut self, item_id: &str, file_path: &str) -> Result<Item, BwError> {
        // Bulk: file uploads can be megabytes.
        let session = self.session()?.to_string();
        let out = bw_run_with_session_timeout(
            &[
                "create",
                "attachment",
                "--file",
                file_path,
                "--itemid",
                item_id,
            ],
            &session,
            BULK_TIMEOUT,
        )?;
        if out.status.success() {
            serde_json::from_str::<Item>(&stdout_str(&out)).map_err(|e| {
                BwError::InvalidJson(format!("Error parsing item with new attachment: {e}"))
            })
        } else {
            Err(bw_exit(&out))
        }
    }

    fn download_attachment(
        &mut self,
        item_id: &str,
        file_name: &str,
        output_path: &str,
    ) -> Result<(), BwError> {
        // Bulk: file downloads can be megabytes.
        let session = self.session()?.to_string();
        let out = bw_run_with_session_timeout(
            &[
                "get",
                "attachment",
                file_name,
                "--itemid",
                item_id,
                "--output",
                output_path,
            ],
            &session,
            BULK_TIMEOUT,
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn delete_attachment(&mut self, item_id: &str, attachment_id: &str) -> Result<(), BwError> {
        let session = self.session()?.to_string();
        let out = bw_run_with_session_timeout(
            &["delete", "attachment", attachment_id, "--itemid", item_id],
            &session,
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn list_organizations(&mut self) -> Result<Vec<Organization>, BwError> {
        // Local-only — reads the cached membership list.
        let session = self.session()?.to_string();
        let out = bw_run_with_session(&["list", "organizations"], &session)?;
        if out.status.success() {
            serde_json::from_str::<Vec<Organization>>(&stdout_str(&out))
                .map_err(|e| BwError::InvalidJson(format!("Error parsing organizations JSON: {e}")))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn list_collections(&mut self) -> Result<Vec<Collection>, BwError> {
        // Local-only.
        let session = self.session()?.to_string();
        let out = bw_run_with_session(&["list", "collections"], &session)?;
        if out.status.success() {
            serde_json::from_str::<Vec<Collection>>(&stdout_str(&out))
                .map_err(|e| BwError::InvalidJson(format!("Error parsing collections JSON: {e}")))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn send_text(
        &mut self,
        name: &str,
        days_to_expire: u8,
        content: &str,
    ) -> Result<String, BwError> {
        let session = self.session()?.to_string();
        let days = days_to_expire.clamp(1, 31).to_string();
        // bw prints just the URL on stdout when --fullObject is omitted.
        let out = bw_run_with_session_timeout(
            &["send", "-n", name, "-d", &days, content],
            &session,
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            let url = stdout_str(&out);
            if url.is_empty() {
                Err(BwError::Shape("bw send returned an empty URL".into()))
            } else {
                Ok(url)
            }
        } else {
            Err(bw_exit(&out))
        }
    }

    /// Overrides the default sequential implementation: spawns one
    /// worker thread per query so the four `bw` invocations (folders,
    /// orgs, collections, import-formats) overlap their Node.js
    /// cold-starts and finish in `max(t_i)` instead of `sum(t_i)`.
    /// On a typical login that drops the post-login wait by ~3-4 s.
    ///
    /// Each thread holds its own clone of the adapter (so the
    /// session key is shared by deep copy, not by `&mut`), making
    /// the parallel reads sound under the existing `&mut self` trait
    /// signature. The clones are dropped when the threads return,
    /// zeroing their session-key copies.
    ///
    /// Thread-panic recovery: a poisoned join is reported as an
    /// `Err` for that specific result; the other three still come
    /// through cleanly.
    fn parallel_session_data(&mut self) -> ParallelSessionData {
        let f = self.clone();
        let o = self.clone();
        let c = self.clone();
        let i = self.clone();
        let folders = std::thread::spawn(move || {
            let mut a = f;
            a.list_folders()
        });
        let orgs = std::thread::spawn(move || {
            let mut a = o;
            a.list_organizations()
        });
        let cols = std::thread::spawn(move || {
            let mut a = c;
            a.list_collections()
        });
        let formats = std::thread::spawn(move || {
            let mut a = i;
            a.list_import_formats()
        });
        ParallelSessionData {
            folders: folders
                .join()
                .unwrap_or_else(|_| Err(BwError::Internal("folders worker panicked".into()))),
            organizations: orgs
                .join()
                .unwrap_or_else(|_| Err(BwError::Internal("organizations worker panicked".into()))),
            collections: cols
                .join()
                .unwrap_or_else(|_| Err(BwError::Internal("collections worker panicked".into()))),
            import_formats: formats.join().unwrap_or_else(|_| {
                Err(BwError::Internal("import-formats worker panicked".into()))
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::VaultPort;

    #[test]
    fn classifies_device_verification_prompts() {
        assert!(matches!(
            combined_outcome("New device detected"),
            Some(LoginOutcome::NeedsDeviceVerification)
        ));
        assert!(matches!(
            combined_outcome("Verification required to continue"),
            Some(LoginOutcome::NeedsDeviceVerification)
        ));
        assert!(matches!(
            combined_outcome("Please enter the verification code"),
            Some(LoginOutcome::NeedsDeviceVerification)
        ));
        assert!(matches!(
            combined_outcome("Enter OTP:"),
            Some(LoginOutcome::NeedsDeviceVerification)
        ));
    }

    #[test]
    fn classifies_two_factor_prompts() {
        assert!(matches!(
            combined_outcome("Two-step Login Code:"),
            Some(LoginOutcome::NeedsTwoFactor)
        ));
        assert!(matches!(
            combined_outcome("Two-step token"),
            Some(LoginOutcome::NeedsTwoFactor)
        ));
        assert!(matches!(
            combined_outcome("Two-step Login (Authenticator app)"),
            Some(LoginOutcome::NeedsTwoFactor)
        ));
        assert!(matches!(
            combined_outcome("Additional authentication required"),
            Some(LoginOutcome::NeedsTwoFactor)
        ));
    }

    #[test]
    fn two_factor_takes_precedence_over_device_verification() {
        // bw 2FA prompts often include the substring "verification
        // code" too — those must be classified as 2FA, not as a
        // device verification (which would skip the --method flag and
        // fail silently).
        let mixed = "Two-step Login. Enter the verification code:";
        assert!(matches!(
            combined_outcome(mixed),
            Some(LoginOutcome::NeedsTwoFactor)
        ));
    }

    #[test]
    fn unrelated_errors_classify_as_none() {
        assert!(combined_outcome("Invalid email address").is_none());
        assert!(combined_outcome("Username or password is incorrect.").is_none());
        assert!(combined_outcome("").is_none());
    }

    /// `lock` must drop the cached session key. The zeroizing wrapper
    /// only helps if `lock` actually triggers the drop, so guard
    /// against a refactor that accidentally keeps the field populated.
    /// We seed `session_key` directly (no real `bw login` involved)
    /// and only inspect the in-memory state afterwards — `bw lock`
    /// errors from the spawned child are irrelevant here.
    #[test]
    fn lock_clears_cached_session_key() {
        let mut a = BwCliAdapter {
            session_key: Some(Arc::new(Zeroizing::new("test-key-DO-NOT-USE".into()))),
            list_items_timeout: DEFAULT_LIST_ITEMS_TIMEOUT,
        };
        assert!(a.session_key().is_some());
        a.lock();
        assert!(a.session_key().is_none());
    }

    /// Compile-time guard that `session_key` is the zeroizing wrapper
    /// behind an `Arc`. The wrapper-on-drop guarantees the bytes are
    /// scrubbed when the last reference is released; the `Arc` keeps
    /// parallel `parallel_session_data` workers from each holding a
    /// deep-copied second allocation of the secret. If a future
    /// refactor swaps either layer this fails to compile.
    #[test]
    fn session_key_field_type_is_arc_zeroizing() {
        fn assert_is_arc_zeroizing(_: &Option<Arc<Zeroizing<String>>>) {}
        let a = BwCliAdapter {
            session_key: None,
            list_items_timeout: DEFAULT_LIST_ITEMS_TIMEOUT,
        };
        assert_is_arc_zeroizing(&a.session_key);
    }

    /// Cloning the adapter must share the same session-key allocation
    /// (one byte buffer, one `Arc`-counted refcount), not deep-copy
    /// it. This is the security-relevant invariant behind switching
    /// from `Option<Zeroizing<String>>` to
    /// `Option<Arc<Zeroizing<String>>>`: parallel session reads spawn
    /// 4 worker threads at login time, and 4 deep copies of the
    /// session key would widen the heap-dump exposure window 5×.
    #[test]
    fn clone_shares_session_key_allocation() {
        let a = BwCliAdapter {
            session_key: Some(Arc::new(Zeroizing::new("shared-key".into()))),
            list_items_timeout: DEFAULT_LIST_ITEMS_TIMEOUT,
        };
        let b = a.clone();
        let arc_a = a.session_key.as_ref().unwrap();
        let arc_b = b.session_key.as_ref().unwrap();
        assert!(Arc::ptr_eq(arc_a, arc_b), "Arc must share the allocation");
        assert_eq!(Arc::strong_count(arc_a), 2);
    }

    #[test]
    fn with_list_items_timeout_overrides_default() {
        let a = BwCliAdapter::new_with(None).with_list_items_timeout(42);
        assert_eq!(a.list_items_timeout, 42);
    }

    #[test]
    fn with_list_items_timeout_ignores_zero() {
        // Zero would mean "kill bw immediately" — the guard keeps the
        // existing default instead of disabling the safety net.
        let a = BwCliAdapter::new_with(None).with_list_items_timeout(0);
        assert_eq!(a.list_items_timeout, DEFAULT_LIST_ITEMS_TIMEOUT);
    }

    #[test]
    fn new_with_adopts_provided_seed() {
        let seed = Zeroizing::new("seed-key-XYZ".to_string());
        let a = BwCliAdapter::new_with(Some(seed));
        assert_eq!(a.session_key(), Some("seed-key-XYZ"));
    }

    #[test]
    fn new_with_drops_empty_seed() {
        // An empty seed must not paper over the env-fallback path,
        // and must not pretend the vault is unlocked.
        let seed = Zeroizing::new(String::new());
        // Avoid leaning on the ambient $BW_SESSION; the constructor
        // would happily pick that up. Treat the assertion as "either
        // env-supplied or None" so the test stays stable in CI.
        let a = BwCliAdapter::new_with(Some(seed));
        let env_present = std::env::var("BW_SESSION")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .is_some();
        if env_present {
            assert!(a.session_key().is_some());
        } else {
            assert!(a.session_key().is_none());
        }
    }
}
