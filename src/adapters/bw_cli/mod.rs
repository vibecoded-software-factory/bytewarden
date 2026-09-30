pub mod codec;
pub mod json;
pub mod process;

use crate::domain::{
    Collection, Folder, Item, LoginOutcome, Organization, TwoFactorMethod, VaultInfo, VaultStatus,
};
use crate::ports::{BwError, ParallelSessionData, VaultPort};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use zeroize::Zeroizing;

use codec::base64_encode;
use json::opt_str;
use process::{
    BW_PASSWORD_ENV, PromptWait, bw_run, bw_run_timeout, bw_run_with_password,
    bw_run_with_password_and_stdin_timeout, bw_run_with_session,
    bw_run_with_session_and_stdin_timeout, bw_run_with_session_timeout, spawn_interactive,
    stderr_str, stdout_str,
};

fn redact_secret(text: &str, secret: &str) -> String {
    if secret.is_empty() {
        return text.to_string();
    }
    text.replace(secret, "***")
}

enum PayloadOp<'a> {
    CreateItem,
    EditItem {
        item_id: &'a str,
    },
    CreateFolder,
    EditFolder {
        folder_id: &'a str,
    },
    Move {
        item_id: &'a str,
        organization_id: &'a str,
    },
    SendCreate,
}

fn payload_invocation<'a>(op: PayloadOp<'a>, json: &str) -> (Vec<&'a str>, Zeroizing<String>) {
    let args = match op {
        PayloadOp::CreateItem => vec!["create", "item"],
        PayloadOp::EditItem { item_id } => vec!["edit", "item", item_id],
        PayloadOp::CreateFolder => vec!["create", "folder"],
        PayloadOp::EditFolder { folder_id } => vec!["edit", "folder", folder_id],
        PayloadOp::Move {
            item_id,
            organization_id,
        } => vec!["move", item_id, organization_id],
        PayloadOp::SendCreate => vec!["send", "create"],
    };
    (args, Zeroizing::new(base64_encode(json)))
}

fn bw_exit(out: &std::process::Output) -> BwError {
    BwError::exit(stderr_str(out), out.status.code())
}

fn parse_list_tolerant<T: serde::de::DeserializeOwned>(
    json: &str,
    what: &str,
) -> Result<Vec<T>, BwError> {
    let rows: Vec<serde_json::Value> = serde_json::from_str(json)
        .map_err(|e| BwError::InvalidJson(format!("Error parsing {what} JSON: {e}")))?;
    Ok(rows
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}

const STATUS_TIMEOUT: u64 = 4;

const QUICK_NET_TIMEOUT: u64 = 10;

const AUTH_TIMEOUT: u64 = 30;

const SSO_TIMEOUT: u64 = 180;

const ITEM_OP_TIMEOUT: u64 = 15;

const SYNC_TIMEOUT: u64 = 30;

const BULK_TIMEOUT: u64 = 60;

const DEFAULT_LIST_ITEMS_TIMEOUT: u64 = 60;

const TWO_FACTOR_PROMPT_PATTERNS: &[&str] = &[
    "two-step login",
    "two-step token",
    "authenticator app",
    "additional authentication",
    "no provider selected",
    "no providers available",
];

const DEVICE_VERIFICATION_PROMPT_PATTERNS: &[&str] = &[
    "new device",
    "device verification",
    "verification required",
    "verification code",
    "enter otp",
    "code is required",
];

const PROMPT_DEVICE_VERIFICATION: &[&str] = &["new device verification required"];

const PROMPT_TWO_FACTOR_CODE: &[&str] = &["two-step login code"];

const PROMPT_TWO_FACTOR_METHOD: &[&str] = &["two-step login method"];

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

#[derive(Clone)]
pub struct BwCliAdapter {
    session_key: Option<Arc<Zeroizing<String>>>,

    list_items_timeout: Arc<AtomicU64>,

    pending_login: Arc<std::sync::Mutex<Option<process::InteractiveChild>>>,
}

impl BwCliAdapter {
    pub fn new() -> Self {
        Self::new_with(None)
    }

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
            list_items_timeout: Arc::new(AtomicU64::new(DEFAULT_LIST_ITEMS_TIMEOUT)),
            pending_login: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn with_list_items_timeout(self, secs: u64) -> Self {
        if secs > 0 {
            self.list_items_timeout.store(secs, Ordering::Relaxed);
        }
        self
    }

    pub fn list_items_timeout_handle(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.list_items_timeout)
    }

    fn park_pending_login(&mut self, child: process::InteractiveChild) {
        if let Ok(mut slot) = self.pending_login.lock() {
            *slot = Some(child);
        }
    }

    fn take_pending_login(&mut self) -> Option<process::InteractiveChild> {
        self.pending_login.lock().ok().and_then(|mut s| s.take())
    }

    fn clear_pending_login(&mut self) {
        drop(self.take_pending_login());
    }

    fn session(&self) -> Result<Zeroizing<String>, BwError> {
        self.session_key
            .as_ref()
            .map(|z| Zeroizing::new(z.as_str().to_string()))
            .ok_or_else(|| BwError::Internal("Vault is locked".to_string()))
    }

    fn run_payload(
        &self,
        op: PayloadOp<'_>,
        json: &str,
        secs: u64,
    ) -> Result<std::process::Output, BwError> {
        let session = self.session()?;
        let (args, stdin) = payload_invocation(op, json);
        bw_run_with_session_and_stdin_timeout(&args, &session, &stdin, secs)
    }
}

impl Default for BwCliAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl VaultPort for BwCliAdapter {
    fn status(&mut self) -> Result<VaultInfo, BwError> {
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
        self.clear_pending_login();

        let mut child = match spawn_interactive(
            &["login", email, "--passwordenv", BW_PASSWORD_ENV, "--raw"],
            password,
        ) {
            Ok(c) => c,
            Err(e) => return LoginOutcome::Failed(e.to_string()),
        };

        let markers: Vec<&str> = PROMPT_DEVICE_VERIFICATION
            .iter()
            .chain(PROMPT_TWO_FACTOR_CODE)
            .chain(PROMPT_TWO_FACTOR_METHOD)
            .copied()
            .collect();

        match child.wait_for_prompt(&markers, AUTH_TIMEOUT) {
            Err(e) => LoginOutcome::Failed(e.to_string()),
            Ok(PromptWait::TimedOut) => {
                LoginOutcome::Failed(format!("bw login did not respond within {AUTH_TIMEOUT}s"))
            }
            Ok(PromptWait::Reached) => {
                let seen = child.stderr_so_far().to_lowercase();
                if PROMPT_TWO_FACTOR_METHOD.iter().any(|m| seen.contains(m)) {
                    return LoginOutcome::NeedsTwoFactor;
                }
                let outcome = if PROMPT_TWO_FACTOR_CODE.iter().any(|m| seen.contains(m)) {
                    LoginOutcome::NeedsTwoFactor
                } else {
                    LoginOutcome::NeedsDeviceVerification
                };

                self.park_pending_login(child);
                outcome
            }
            Ok(PromptWait::Exited(out)) => {
                if out.status.success() {
                    let key = stdout_str(&out);

                    self.session_key = Some(Arc::new(Zeroizing::new(key.clone())));
                    return LoginOutcome::Success(key);
                }

                let combined = format!("{}\n{}", stdout_str(&out), stderr_str(&out));
                match combined_outcome(&combined) {
                    Some(o) => o,
                    None => LoginOutcome::Failed(stderr_str(&out)),
                }
            }
        }
    }

    fn login_with_otp(
        &mut self,
        _email: &str,
        _password: &str,
        otp: &str,
    ) -> Result<String, BwError> {
        let mut child = self.take_pending_login().ok_or_else(|| {
            BwError::Internal(
                "the login attempt that requested this code is gone — start the login again".into(),
            )
        })?;

        let payload = Zeroizing::new(format!("{otp}\n"));
        child.submit_line(&payload)?;
        let out = child.finish(AUTH_TIMEOUT, "bw login")?;

        if out.status.success() {
            let key = stdout_str(&out);
            self.session_key = Some(Arc::new(Zeroizing::new(key.clone())));
            Ok(key)
        } else {
            Err(BwError::exit(
                redact_secret(&stderr_str(&out), otp),
                out.status.code(),
            ))
        }
    }

    fn login_with_two_factor(
        &mut self,
        email: &str,
        password: &str,
        code: &str,
        method: TwoFactorMethod,
    ) -> Result<String, BwError> {
        if let Some(mut child) = self.take_pending_login() {
            let payload = Zeroizing::new(format!("{code}\n"));
            child.submit_line(&payload)?;
            let out = child.finish(AUTH_TIMEOUT, "bw login")?;
            return if out.status.success() {
                let key = stdout_str(&out);
                self.session_key = Some(Arc::new(Zeroizing::new(key.clone())));
                Ok(key)
            } else {
                Err(BwError::exit(
                    redact_secret(&stderr_str(&out), code),
                    out.status.code(),
                ))
            };
        }

        let method_str = method.as_u8().to_string();

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
        let out = bw_run_timeout(&["login", "--apikey"], AUTH_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn login_with_sso(&mut self) -> Result<(), BwError> {
        let out = bw_run_timeout(&["login", "--sso"], SSO_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn unlock(&mut self, password: &str) -> Result<String, BwError> {
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
        let _ = bw_run(&["lock"]);
        self.session_key = None;
    }

    fn logout(&mut self) -> Result<(), BwError> {
        let out = bw_run_timeout(&["logout"], QUICK_NET_TIMEOUT)?;

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

    fn set_server(&mut self, url: &str) -> Result<(), BwError> {
        let out = bw_run_timeout(&["config", "server", url], QUICK_NET_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn list_items(&mut self) -> Result<Vec<Item>, BwError> {
        let session = self.session()?;
        let timeout = self.list_items_timeout.load(Ordering::Relaxed);
        let out = bw_run_with_session_timeout(&["list", "items"], &session, timeout)?;
        if out.status.success() {
            parse_list_tolerant::<Item>(&stdout_str(&out), "items")
        } else {
            Err(bw_exit(&out))
        }
    }

    fn list_trash(&mut self) -> Result<Vec<Item>, BwError> {
        let session = self.session()?;
        let timeout = self.list_items_timeout.load(Ordering::Relaxed);
        let out = bw_run_with_session_timeout(&["list", "items", "--trash"], &session, timeout)?;
        if out.status.success() {
            parse_list_tolerant::<Item>(&stdout_str(&out), "trash")
        } else {
            Err(bw_exit(&out))
        }
    }

    fn sync(&mut self) -> Result<(), BwError> {
        let session = self.session()?;
        let out = bw_run_with_session_timeout(&["sync"], &session, SYNC_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn get_totp(&mut self, item_id: &str) -> Result<String, BwError> {
        let session = self.session()?;
        let out = bw_run_with_session(&["get", "totp", item_id], &session)?;
        if out.status.success() {
            Ok(stdout_str(&out))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn get_item_json(&mut self, item_id: &str) -> Result<Zeroizing<String>, BwError> {
        let session = self.session()?;
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
        let session = self.session()?;
        let out =
            bw_run_with_session_timeout(&["get", "exposed", item_id], &session, ITEM_OP_TIMEOUT)?;
        if !out.status.success() {
            return Err(bw_exit(&out));
        }

        let text = stdout_str(&out);
        text.parse::<u32>()
            .map_err(|_| BwError::Shape(format!("Unexpected `bw get exposed` output: {text}")))
    }

    fn create_item(&mut self, item_json: &str) -> Result<Item, BwError> {
        let out = self.run_payload(PayloadOp::CreateItem, item_json, ITEM_OP_TIMEOUT)?;
        if out.status.success() {
            serde_json::from_str::<Item>(&stdout_str(&out))
                .map_err(|e| BwError::InvalidJson(format!("Error parsing created item: {e}")))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn edit_item(&mut self, item_id: &str, item_json: &str) -> Result<Item, BwError> {
        let out = self.run_payload(PayloadOp::EditItem { item_id }, item_json, ITEM_OP_TIMEOUT)?;
        if out.status.success() {
            serde_json::from_str::<Item>(&stdout_str(&out))
                .map_err(|e| BwError::InvalidJson(format!("Error parsing edited item: {e}")))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn delete_item(&mut self, item_id: &str, permanent: bool) -> Result<(), BwError> {
        let session = self.session()?;
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
        let session = self.session()?;
        let out =
            bw_run_with_session_timeout(&["restore", "item", item_id], &session, ITEM_OP_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn list_folders(&mut self) -> Result<Vec<Folder>, BwError> {
        let session = self.session()?;
        let out = bw_run_with_session(&["list", "folders"], &session)?;
        if out.status.success() {
            parse_list_tolerant::<Folder>(&stdout_str(&out), "folders")
        } else {
            Err(bw_exit(&out))
        }
    }

    fn create_folder(&mut self, name: &str) -> Result<Folder, BwError> {
        let payload = json!({ "name": name }).to_string();
        let out = self.run_payload(PayloadOp::CreateFolder, &payload, ITEM_OP_TIMEOUT)?;
        if out.status.success() {
            serde_json::from_str::<Folder>(&stdout_str(&out))
                .map_err(|e| BwError::InvalidJson(format!("Error parsing created folder: {e}")))
        } else {
            Err(bw_exit(&out))
        }
    }

    fn edit_folder(&mut self, folder_id: &str, name: &str) -> Result<Folder, BwError> {
        let payload = json!({ "name": name }).to_string();
        let out = self.run_payload(
            PayloadOp::EditFolder { folder_id },
            &payload,
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
        let session = self.session()?;
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
        let session = self.session()?;
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
        let session = self.session()?;
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
        let json = serde_json::to_string(collection_ids).map_err(|e| {
            BwError::InvalidJson(format!("Could not serialize collection ids: {e}"))
        })?;
        let out = self.run_payload(
            PayloadOp::Move {
                item_id,
                organization_id,
            },
            &json,
            ITEM_OP_TIMEOUT,
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn list_import_formats(&mut self) -> Result<Vec<String>, BwError> {
        let out = bw_run(&["import", "--formats"])?;
        if !out.status.success() {
            return Err(bw_exit(&out));
        }
        let stdout = stdout_str(&out);

        let mut seen = std::collections::HashSet::new();
        let mut out: Vec<String> = Vec::new();
        for line in stdout.lines() {
            let token: String = line
                .trim()
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();

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
        let session = self.session()?;
        let out =
            bw_run_with_session_timeout(&["import", format, input_path], &session, BULK_TIMEOUT)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(bw_exit(&out))
        }
    }

    fn upload_attachment(&mut self, item_id: &str, file_path: &str) -> Result<Item, BwError> {
        let session = self.session()?;
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
        let session = self.session()?;
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
        let session = self.session()?;
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
        let session = self.session()?;
        let out = bw_run_with_session(&["list", "organizations"], &session)?;
        if out.status.success() {
            parse_list_tolerant::<Organization>(&stdout_str(&out), "organizations")
        } else {
            Err(bw_exit(&out))
        }
    }

    fn list_collections(&mut self) -> Result<Vec<Collection>, BwError> {
        let session = self.session()?;
        let out = bw_run_with_session(&["list", "collections"], &session)?;
        if out.status.success() {
            parse_list_tolerant::<Collection>(&stdout_str(&out), "collections")
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
        let days = days_to_expire.clamp(1, 31) as i64;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let deletion = crate::domain::timefmt::unix_to_iso_utc(now + days * 86_400);

        let payload = json!({
            "name": name,
            "type": 0,
            "text": { "text": content, "hidden": false },
            "deletionDate": deletion,
        });
        let out = self.run_payload(PayloadOp::SendCreate, &payload.to_string(), ITEM_OP_TIMEOUT)?;
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
    fn tolerant_parse_skips_bad_rows_and_keeps_the_rest() {
        let json = r#"[
            {"id":"1","name":"a","type":1},
            {"broken":true},
            {"id":"2","name":"b","type":1}
        ]"#;
        let items: Vec<Item> = parse_list_tolerant(json, "items").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "1");
        assert_eq!(items[1].id, "2");
    }

    #[test]
    fn tolerant_parse_errors_only_when_the_top_level_is_not_an_array() {
        assert!(parse_list_tolerant::<Item>("not json", "items").is_err());
        assert!(parse_list_tolerant::<Item>("{}", "items").is_err());

        assert!(
            parse_list_tolerant::<Item>("[]", "items")
                .unwrap()
                .is_empty()
        );
    }

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

    #[test]
    fn non_interactive_code_request_is_a_challenge_not_a_credential_failure() {
        for text in [
            "Code is required.",
            "code is required",
            "\nCode is required.",
        ] {
            assert!(
                matches!(
                    combined_outcome(text),
                    Some(LoginOutcome::NeedsDeviceVerification)
                ),
                "{text:?} must be treated as a code challenge"
            );
        }
    }

    #[test]
    fn non_interactive_multi_provider_two_factor_routes_to_the_method_picker() {
        for text in [
            "Login failed. No provider selected.",
            "No providers available for this client.",
        ] {
            assert!(
                matches!(combined_outcome(text), Some(LoginOutcome::NeedsTwoFactor)),
                "{text:?} must ask the user to pick a 2FA method"
            );
        }
    }

    #[test]
    fn a_named_factor_outranks_the_generic_code_request() {
        assert!(matches!(
            combined_outcome("Two-step login code:\nCode is required."),
            Some(LoginOutcome::NeedsTwoFactor)
        ));
    }

    #[test]
    fn payload_writes_keep_the_encoded_json_out_of_argv() {
        let json = r#"{"login":{"password":"hunter2-DO-NOT-USE"}}"#;
        let encoded = base64_encode(json);
        let cases: Vec<(PayloadOp<'_>, Vec<&str>)> = vec![
            (PayloadOp::CreateItem, vec!["create", "item"]),
            (
                PayloadOp::EditItem { item_id: "item-1" },
                vec!["edit", "item", "item-1"],
            ),
            (PayloadOp::CreateFolder, vec!["create", "folder"]),
            (
                PayloadOp::EditFolder {
                    folder_id: "folder-1",
                },
                vec!["edit", "folder", "folder-1"],
            ),
            (
                PayloadOp::Move {
                    item_id: "item-1",
                    organization_id: "org-1",
                },
                vec!["move", "item-1", "org-1"],
            ),
            (PayloadOp::SendCreate, vec!["send", "create"]),
        ];
        for (op, expected) in cases {
            let (args, stdin) = payload_invocation(op, json);
            assert_eq!(args, expected);
            assert!(
                args.iter()
                    .all(|a| !a.contains(&encoded) && !a.contains("hunter2")),
                "payload leaked into argv: {args:?}"
            );
            assert_eq!(stdin.as_str(), encoded, "the payload goes over stdin");
        }
    }

    #[test]
    fn lock_clears_cached_session_key() {
        let mut a = BwCliAdapter {
            session_key: Some(Arc::new(Zeroizing::new("test-key-DO-NOT-USE".into()))),
            list_items_timeout: Arc::new(AtomicU64::new(DEFAULT_LIST_ITEMS_TIMEOUT)),
            pending_login: Arc::new(std::sync::Mutex::new(None)),
        };
        assert!(a.session_key().is_some());
        a.lock();
        assert!(a.session_key().is_none());
    }

    #[test]
    fn session_key_field_type_is_arc_zeroizing() {
        fn assert_is_arc_zeroizing(_: &Option<Arc<Zeroizing<String>>>) {}
        let a = BwCliAdapter {
            session_key: None,
            list_items_timeout: Arc::new(AtomicU64::new(DEFAULT_LIST_ITEMS_TIMEOUT)),
            pending_login: Arc::new(std::sync::Mutex::new(None)),
        };
        assert_is_arc_zeroizing(&a.session_key);
    }

    #[test]
    fn per_call_session_copy_is_zeroizing() {
        fn assert_is_zeroizing(_: &Zeroizing<String>) {}
        let a = BwCliAdapter {
            session_key: Some(Arc::new(Zeroizing::new("SESSION".to_string()))),
            list_items_timeout: Arc::new(AtomicU64::new(DEFAULT_LIST_ITEMS_TIMEOUT)),
            pending_login: Arc::new(std::sync::Mutex::new(None)),
        };
        let session = a.session().expect("unlocked adapter yields a key");
        assert_is_zeroizing(&session);
        assert_eq!(&*session, "SESSION");

        let as_str: &str = &session;
        assert_eq!(as_str, "SESSION");
    }

    #[test]
    fn session_errors_when_locked() {
        let a = BwCliAdapter {
            session_key: None,
            list_items_timeout: Arc::new(AtomicU64::new(DEFAULT_LIST_ITEMS_TIMEOUT)),
            pending_login: Arc::new(std::sync::Mutex::new(None)),
        };
        assert!(matches!(a.session(), Err(BwError::Internal(_))));
    }

    #[test]
    fn clone_shares_session_key_allocation() {
        let a = BwCliAdapter {
            session_key: Some(Arc::new(Zeroizing::new("shared-key".into()))),
            list_items_timeout: Arc::new(AtomicU64::new(DEFAULT_LIST_ITEMS_TIMEOUT)),
            pending_login: Arc::new(std::sync::Mutex::new(None)),
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
        assert_eq!(a.list_items_timeout.load(Ordering::Relaxed), 42);
    }

    #[test]
    fn with_list_items_timeout_ignores_zero() {
        let a = BwCliAdapter::new_with(None).with_list_items_timeout(0);
        assert_eq!(
            a.list_items_timeout.load(Ordering::Relaxed),
            DEFAULT_LIST_ITEMS_TIMEOUT
        );
    }

    #[test]
    fn list_items_timeout_handle_shares_the_atomic() {
        let a = BwCliAdapter::new_with(None);
        let handle = a.list_items_timeout_handle();

        handle.store(240, Ordering::Relaxed);
        assert_eq!(a.list_items_timeout.load(Ordering::Relaxed), 240);
    }

    #[test]
    fn new_with_adopts_provided_seed() {
        let seed = Zeroizing::new("seed-key-XYZ".to_string());
        let a = BwCliAdapter::new_with(Some(seed));
        assert_eq!(a.session_key(), Some("seed-key-XYZ"));
    }

    #[test]
    fn new_with_drops_empty_seed() {
        let seed = Zeroizing::new(String::new());

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
