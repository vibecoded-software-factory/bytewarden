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

use crate::domain::{Collection, Folder, Item, LoginOutcome, Organization, VaultInfo, VaultStatus};
use crate::ports::VaultPort;
use serde_json::json;
use zeroize::Zeroizing;

use codec::base64_encode;
use json::opt_str;
use process::{
    BW_PASSWORD_ENV, bw_run, bw_run_timeout, bw_run_with_password,
    bw_run_with_password_and_stdin_timeout, bw_run_with_password_timeout, stderr_str, stdout_str,
};

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

/// Patterns that indicate `bw login` wants a one-time device
/// verification code (or a 2FA code) on the next attempt.
///
/// These come from the actual `bw` CLI prompts and error messages.
/// Each one is matched as a case-insensitive substring against the
/// combined stdout + stderr of the failed `login` invocation.
///
/// New entries are cheap to add — when in doubt, include the phrase
/// rather than miss a future copy change.
const OTP_PROMPT_PATTERNS: &[&str] = &[
    "new device",
    "enter otp",
    "verification required",
    "verification code",
    "two-step login",
    "two-step token",
    "additional authentication",
];

/// Heuristic — does the combined stdout+stderr indicate that `bw login`
/// wants a one-time device-verification code on the next attempt?
///
/// The check is intentionally a substring search: `bw` prompt text has
/// changed across versions and we'd rather match too eagerly than miss
/// a real prompt and dump the user back at "Invalid credentials".
fn combined_needs_otp(text: &str) -> bool {
    let lower = text.to_lowercase();
    OTP_PROMPT_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Vault adapter that drives the `bw` CLI.
///
/// Holds the session key returned by `bw unlock` / `bw login --raw` so
/// later `bw` invocations can be fed the same key via `--session`.
///
/// The session key is wrapped in [`Zeroizing`] so the underlying bytes
/// are overwritten with zeroes when the field is dropped — either
/// because the adapter is dropped or because [`Self::lock`] /
/// [`Self::logout`] reset it to `None`. That closes the window in
/// which a heap dump or a swap-out could leak the unlocked-vault
/// authorisation token after the user has already locked.
pub struct BwCliAdapter {
    session_key: Option<Zeroizing<String>>,
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
        let session_key = seed_key.filter(|s| !s.is_empty()).or_else(|| {
            std::env::var("BW_SESSION")
                .ok()
                .map(|s| Zeroizing::new(s.trim().to_string()))
                .filter(|s| !s.is_empty())
        });
        Self { session_key }
    }

    /// Returns the current session key or a "Vault is locked" error,
    /// suitable for use as a `?`-able preamble in vault operations.
    fn session(&self) -> Result<&str, String> {
        self.session_key
            .as_ref()
            .map(|z| z.as_str())
            .ok_or_else(|| "Vault is locked".to_string())
    }
}

impl Default for BwCliAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl VaultPort for BwCliAdapter {
    // ── Authentication ────────────────────────────────────────────────────

    fn status(&mut self) -> Result<VaultInfo, String> {
        // `bw status` is local-only and should be fast, but Node startup
        // can be slow — bound it to 4s, then fall through to the login flow.
        let out = bw_run_timeout(&["status"], STATUS_TIMEOUT)?;
        let val: serde_json::Value = serde_json::from_str(&stdout_str(&out))
            .map_err(|e| format!("bw status JSON parse error: {e}"))?;

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
            Err(e) => return LoginOutcome::Failed(e),
        };
        if out.status.success() {
            let key = stdout_str(&out);
            // Stash the zeroizing copy first so the long-lived storage
            // is the protected one; the LoginOutcome value is discarded
            // by every current caller.
            self.session_key = Some(Zeroizing::new(key.clone()));
            return LoginOutcome::Success(key);
        }
        // `bw` exits with an error when it needs an OTP but stdin is empty;
        // detect it via the prompt text on stdout/stderr.
        let combined = format!("{}\n{}", stdout_str(&out), stderr_str(&out));
        if combined_needs_otp(&combined) {
            return LoginOutcome::NeedsOtp;
        }
        LoginOutcome::Failed(stderr_str(&out))
    }

    fn login_with_otp(&mut self, email: &str, password: &str, otp: &str) -> Result<String, String> {
        // The OTP is fed via stdin instead of `--code <otp>` so it
        // never appears in `argv` (and therefore never in `ps`). That
        // requires dropping `--nointeraction` for this single call so
        // bw's interactive prompt reads the code off stdin. We
        // intentionally **do not** prepend the global `--nointeraction`
        // here.
        //
        // A trailing newline is required — `inquirer` (the prompt
        // library bw uses) treats it as the line terminator.
        let out = bw_run_with_password_and_stdin_timeout(
            &["login", email, "--passwordenv", BW_PASSWORD_ENV, "--raw"],
            password,
            &format!("{otp}\n"),
            AUTH_TIMEOUT,
        )?;
        if out.status.success() {
            let key = stdout_str(&out);
            self.session_key = Some(Zeroizing::new(key.clone()));
            Ok(key)
        } else {
            Err(stderr_str(&out))
        }
    }

    fn login_with_api_key(&mut self) -> Result<(), String> {
        // `bw login --apikey` consumes BW_CLIENTID and BW_CLIENTSECRET
        // from the parent environment. We do not need to forward them
        // explicitly — std::process::Command inherits the parent env
        // by default.
        let out = bw_run_timeout(&["login", "--apikey"], AUTH_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(stderr_str(&out))
        }
    }

    fn login_with_sso(&mut self) -> Result<(), String> {
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
            Err(stderr_str(&out))
        }
    }

    fn unlock(&mut self, password: &str) -> Result<String, String> {
        // Unlock is a local crypto operation — no network involved, no
        // timeout needed.
        let out = bw_run_with_password(
            &["unlock", "--passwordenv", BW_PASSWORD_ENV, "--raw"],
            password,
        )?;
        if out.status.success() {
            let key = stdout_str(&out);
            self.session_key = Some(Zeroizing::new(key.clone()));
            Ok(key)
        } else {
            Err(stderr_str(&out))
        }
    }

    fn lock(&mut self) {
        // Local-only: clears the cached symmetric key.
        let _ = bw_run(&["lock"]);
        self.session_key = None;
    }

    fn logout(&mut self) -> Result<(), String> {
        let out = bw_run_timeout(&["logout"], QUICK_NET_TIMEOUT)?;
        // Drop the session even if the CLI complained — we cannot use
        // a key whose account just got removed locally.
        self.session_key = None;
        if out.status.success() {
            Ok(())
        } else {
            Err(stderr_str(&out))
        }
    }

    fn session_key(&self) -> Option<&str> {
        self.session_key.as_ref().map(|z| z.as_str())
    }

    // ── Configuration ─────────────────────────────────────────────────────

    fn set_server(&mut self, url: &str) -> Result<(), String> {
        // Talks to the backend to validate the URL.
        let out = bw_run_timeout(&["config", "server", url], QUICK_NET_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(stderr_str(&out))
        }
    }

    // ── Vault data ────────────────────────────────────────────────────────

    fn list_items(&mut self) -> Result<Vec<Item>, String> {
        // Local-only — reads the cached vault populated by the last sync.
        let session = self.session()?.to_string();
        let out = bw_run(&["list", "items", "--session", &session])?;
        if out.status.success() {
            serde_json::from_str::<Vec<Item>>(&stdout_str(&out))
                .map_err(|e| format!("Error parsing items JSON: {e}"))
        } else {
            Err(stderr_str(&out))
        }
    }

    fn list_trash(&mut self) -> Result<Vec<Item>, String> {
        // Local-only.
        let session = self.session()?.to_string();
        let out = bw_run(&["list", "items", "--trash", "--session", &session])?;
        if out.status.success() {
            serde_json::from_str::<Vec<Item>>(&stdout_str(&out))
                .map_err(|e| format!("Error parsing trash JSON: {e}"))
        } else {
            Err(stderr_str(&out))
        }
    }

    fn sync(&mut self) -> Result<(), String> {
        let session = self.session()?.to_string();
        let out = bw_run_timeout(&["sync", "--session", &session], SYNC_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(stderr_str(&out))
        }
    }

    // ── Single-field reads ────────────────────────────────────────────────

    fn get_totp(&mut self, item_id: &str) -> Result<String, String> {
        // TOTP is computed locally from the cached seed — no network.
        let session = self.session()?.to_string();
        let out = bw_run(&["get", "totp", item_id, "--session", &session])?;
        if out.status.success() {
            Ok(stdout_str(&out))
        } else {
            Err(stderr_str(&out))
        }
    }

    fn get_item_json(&mut self, item_id: &str) -> Result<String, String> {
        // Local-only — reads the cached item.
        let session = self.session()?.to_string();
        let out = bw_run(&["get", "item", item_id, "--session", &session])?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).to_string())
        } else {
            Err(stderr_str(&out))
        }
    }

    fn check_exposed(&mut self, item_id: &str) -> Result<u32, String> {
        // Network: bw queries HaveIBeenPwned (k-anonymity API).
        let session = self.session()?.to_string();
        let out = bw_run_timeout(
            &["get", "exposed", item_id, "--session", &session],
            ITEM_OP_TIMEOUT,
        )?;
        if !out.status.success() {
            return Err(stderr_str(&out));
        }
        // bw prints just an integer to stdout — parse it strictly so
        // any unexpected output bubbles up as an error rather than a
        // silent zero.
        let text = stdout_str(&out);
        text.parse::<u32>()
            .map_err(|_| format!("Unexpected `bw get exposed` output: {text}"))
    }

    // ── Item CRUD ─────────────────────────────────────────────────────────

    fn create_item(&mut self, item_json: &str) -> Result<Item, String> {
        let session = self.session()?.to_string();
        let encoded = base64_encode(item_json);
        let out = bw_run_timeout(
            &["create", "item", &encoded, "--session", &session],
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            serde_json::from_str::<Item>(&stdout_str(&out))
                .map_err(|e| format!("Error parsing created item: {e}"))
        } else {
            Err(stderr_str(&out))
        }
    }

    fn edit_item(&mut self, item_id: &str, item_json: &str) -> Result<Item, String> {
        let session = self.session()?.to_string();
        let encoded = base64_encode(item_json);
        let out = bw_run_timeout(
            &["edit", "item", item_id, &encoded, "--session", &session],
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            serde_json::from_str::<Item>(&stdout_str(&out))
                .map_err(|e| format!("Error parsing edited item: {e}"))
        } else {
            Err(stderr_str(&out))
        }
    }

    fn delete_item(&mut self, item_id: &str, permanent: bool) -> Result<(), String> {
        let session = self.session()?.to_string();
        let mut args: Vec<&str> = vec!["delete", "item", item_id, "--session", &session];
        if permanent {
            args.push("--permanent");
        }
        let out = bw_run_timeout(&args, ITEM_OP_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(stderr_str(&out))
        }
    }

    fn restore_item(&mut self, item_id: &str) -> Result<(), String> {
        let session = self.session()?.to_string();
        let out = bw_run_timeout(
            &["restore", "item", item_id, "--session", &session],
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(stderr_str(&out))
        }
    }

    // ── Folder CRUD ───────────────────────────────────────────────────────

    fn list_folders(&mut self) -> Result<Vec<Folder>, String> {
        // Local-only — reads the cached folder list.
        let session = self.session()?.to_string();
        let out = bw_run(&["list", "folders", "--session", &session])?;
        if out.status.success() {
            serde_json::from_str::<Vec<Folder>>(&stdout_str(&out))
                .map_err(|e| format!("Error parsing folders JSON: {e}"))
        } else {
            Err(stderr_str(&out))
        }
    }

    fn create_folder(&mut self, name: &str) -> Result<Folder, String> {
        let session = self.session()?.to_string();
        let payload = json!({ "name": name }).to_string();
        let encoded = base64_encode(&payload);
        let out = bw_run_timeout(
            &["create", "folder", &encoded, "--session", &session],
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            serde_json::from_str::<Folder>(&stdout_str(&out))
                .map_err(|e| format!("Error parsing created folder: {e}"))
        } else {
            Err(stderr_str(&out))
        }
    }

    fn edit_folder(&mut self, folder_id: &str, name: &str) -> Result<Folder, String> {
        let session = self.session()?.to_string();
        let payload = json!({ "name": name }).to_string();
        let encoded = base64_encode(&payload);
        let out = bw_run_timeout(
            &["edit", "folder", folder_id, &encoded, "--session", &session],
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            serde_json::from_str::<Folder>(&stdout_str(&out))
                .map_err(|e| format!("Error parsing edited folder: {e}"))
        } else {
            Err(stderr_str(&out))
        }
    }

    fn delete_folder(&mut self, folder_id: &str) -> Result<(), String> {
        let session = self.session()?.to_string();
        let out = bw_run_timeout(
            &["delete", "folder", folder_id, "--session", &session],
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(stderr_str(&out))
        }
    }

    fn export(&mut self, format: &str, output_path: &str) -> Result<(), String> {
        // Bulk: a full vault export can run for several seconds on
        // large accounts.
        let session = self.session()?.to_string();
        let out = bw_run_timeout(
            &[
                "export",
                "--format",
                format,
                "--output",
                output_path,
                "--session",
                &session,
            ],
            BULK_TIMEOUT,
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(stderr_str(&out))
        }
    }

    fn get_fingerprint(&mut self) -> Result<String, String> {
        // Local-only — derived from the cached public key.
        let session = self.session()?.to_string();
        let out = bw_run(&["get", "fingerprint", "me", "--session", &session])?;
        if out.status.success() {
            Ok(stdout_str(&out))
        } else {
            Err(stderr_str(&out))
        }
    }

    fn import(&mut self, format: &str, input_path: &str) -> Result<(), String> {
        // Bulk: import can upload thousands of items in one go.
        let session = self.session()?.to_string();
        let out = bw_run_timeout(
            &["import", format, input_path, "--session", &session],
            BULK_TIMEOUT,
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(stderr_str(&out))
        }
    }

    fn upload_attachment(&mut self, item_id: &str, file_path: &str) -> Result<Item, String> {
        // Bulk: file uploads can be megabytes.
        let session = self.session()?.to_string();
        let out = bw_run_timeout(
            &[
                "create",
                "attachment",
                "--file",
                file_path,
                "--itemid",
                item_id,
                "--session",
                &session,
            ],
            BULK_TIMEOUT,
        )?;
        if out.status.success() {
            serde_json::from_str::<Item>(&stdout_str(&out))
                .map_err(|e| format!("Error parsing item with new attachment: {e}"))
        } else {
            Err(stderr_str(&out))
        }
    }

    fn download_attachment(
        &mut self,
        item_id: &str,
        file_name: &str,
        output_path: &str,
    ) -> Result<(), String> {
        // Bulk: file downloads can be megabytes.
        let session = self.session()?.to_string();
        let out = bw_run_timeout(
            &[
                "get",
                "attachment",
                file_name,
                "--itemid",
                item_id,
                "--output",
                output_path,
                "--session",
                &session,
            ],
            BULK_TIMEOUT,
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(stderr_str(&out))
        }
    }

    fn delete_attachment(&mut self, item_id: &str, attachment_id: &str) -> Result<(), String> {
        let session = self.session()?.to_string();
        let out = bw_run_timeout(
            &[
                "delete",
                "attachment",
                attachment_id,
                "--itemid",
                item_id,
                "--session",
                &session,
            ],
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(stderr_str(&out))
        }
    }

    fn list_organizations(&mut self) -> Result<Vec<Organization>, String> {
        // Local-only — reads the cached membership list.
        let session = self.session()?.to_string();
        let out = bw_run(&["list", "organizations", "--session", &session])?;
        if out.status.success() {
            serde_json::from_str::<Vec<Organization>>(&stdout_str(&out))
                .map_err(|e| format!("Error parsing organizations JSON: {e}"))
        } else {
            Err(stderr_str(&out))
        }
    }

    fn list_collections(&mut self) -> Result<Vec<Collection>, String> {
        // Local-only.
        let session = self.session()?.to_string();
        let out = bw_run(&["list", "collections", "--session", &session])?;
        if out.status.success() {
            serde_json::from_str::<Vec<Collection>>(&stdout_str(&out))
                .map_err(|e| format!("Error parsing collections JSON: {e}"))
        } else {
            Err(stderr_str(&out))
        }
    }

    fn send_text(
        &mut self,
        name: &str,
        days_to_expire: u8,
        content: &str,
    ) -> Result<String, String> {
        let session = self.session()?.to_string();
        let days = days_to_expire.clamp(1, 31).to_string();
        // bw prints just the URL on stdout when --fullObject is omitted.
        let out = bw_run_timeout(
            &[
                "send",
                "-n",
                name,
                "-d",
                &days,
                "--session",
                &session,
                content,
            ],
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            let url = stdout_str(&out);
            if url.is_empty() {
                Err("bw send returned an empty URL".into())
            } else {
                Ok(url)
            }
        } else {
            Err(stderr_str(&out))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::VaultPort;

    #[test]
    fn detects_known_phrases() {
        assert!(combined_needs_otp("New device detected"));
        assert!(combined_needs_otp("Enter OTP:"));
        assert!(combined_needs_otp("Verification required to continue"));
        assert!(combined_needs_otp("Please enter the verification code"));
        assert!(combined_needs_otp("Two-step login"));
    }

    #[test]
    fn ignores_unrelated_errors() {
        assert!(!combined_needs_otp("Invalid email address"));
        assert!(!combined_needs_otp("Username or password is incorrect."));
        assert!(!combined_needs_otp(""));
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
            session_key: Some(Zeroizing::new("test-key-DO-NOT-USE".into())),
        };
        assert!(a.session_key().is_some());
        a.lock();
        assert!(a.session_key().is_none());
    }

    /// Compile-time guard that `session_key` is the zeroizing wrapper.
    /// If a future refactor swaps the field back to `Option<String>`
    /// this fails to compile, signalling that the protection on the
    /// most security-relevant in-memory secret got dropped.
    #[test]
    fn session_key_field_type_is_zeroizing() {
        fn assert_is_zeroizing(_: &Option<Zeroizing<String>>) {}
        let a = BwCliAdapter { session_key: None };
        assert_is_zeroizing(&a.session_key);
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
