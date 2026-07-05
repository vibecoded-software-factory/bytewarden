//! Authentication, lock and session-resume flows.
//!
//! Every `bw` call runs on the worker thread: a `request_*` builder
//! sends a [`WorkerRequest`] and stashes an [`InFlight`] ticket; the
//! matching `handle_*` runs when the response arrives and chains the
//! next step (status → resume/login → load items → session data → vault).

use crate::ports::BwError;
use zeroize::Zeroizing;

use crate::domain::item::Item;
use crate::domain::vault_info::{LoginOutcome, VaultInfo, VaultStatus};
use crate::ports::ParallelSessionData;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::screens::{LoginField, Screen};
use crate::tui::session_file;
use crate::tui::worker::{InFlight, WorkerRequest};

// ── Boot / resume ─────────────────────────────────────────────────────────

/// Kicks off the boot sequence: `bw status` on the worker. The response
/// handler routes to a resume, the locked login form, or fresh login.
pub fn request_resume(app: &mut App) {
    app.in_flight = Some(InFlight::BootStatus);
    let _ = app.worker_tx.send(WorkerRequest::Status);
}

/// Boot `bw status` response.
pub fn handle_boot_status(app: &mut App, r: Result<VaultInfo, BwError>) {
    let info = match r {
        Ok(i) => i,
        Err(e) => {
            app.push_cmd("bw status", false, &e);
            app.screen = Screen::Login;
            app.set_action(ActionState::Idle);
            return;
        }
    };
    app.push_cmd("bw status", true, &format!("{:?}", info.status));

    // Seed the Server field from the CLI's reported URL so the user can
    // see (and edit) which backend they're hitting.
    let server = info
        .server_url
        .clone()
        .unwrap_or_else(|| "https://bitwarden.com".to_string());
    app.server_cursor = server.chars().count();
    app.server_input = server.clone();
    app.server_committed = server;

    match info.status {
        VaultStatus::Unlocked => {
            // The CLI reports an unlocked vault — that means a usable
            // session key is present (the adapter sets `BW_SESSION` from
            // it before spawning `bw status`). Resume by listing items;
            // if the backend rejects the key we fall back to the login
            // form in `handle_resume_items`.
            app.authenticated = true;
            if let Some(email) = info.user_email.clone()
                && app.email_input.is_empty()
            {
                app.email_cursor = email.chars().count();
                app.email_input = email;
            }
            app.in_flight = Some(InFlight::ResumeItems);
            app.set_action(ActionState::Running("Resuming session…".into()));
            let _ = app.worker_tx.send(WorkerRequest::ListItems);
        }
        VaultStatus::Locked => {
            app.authenticated = true;
            apply_locked_state(app, info.user_email);
            app.screen = Screen::Login;
            app.set_action(ActionState::Idle);
        }
        VaultStatus::Unauthenticated => {
            app.authenticated = false;
            app.screen = Screen::Login;
            app.set_action(ActionState::Idle);
        }
    }
}

/// Resume: items listed with the seeded session key.
pub fn handle_resume_items(app: &mut App, r: Result<Vec<Item>, BwError>) {
    match r {
        Ok(items) => {
            let count = items.len();
            app.items = items;
            app.sort_items();
            app.push_cmd("bw list items", true, &format!("{count} items loaded"));
            app.in_flight = Some(InFlight::ResumeSessionData);
            let _ = app.worker_tx.send(WorkerRequest::ParallelSessionData);
        }
        Err(e) => {
            // The session key was present but the backend rejected it
            // (stale key, server changed, logged out elsewhere…). Fall
            // back to the login form with a hint.
            app.push_cmd("bw list items", false, &e);
            apply_locked_state(app, None);
            app.screen = Screen::Login;
            app.set_action(ActionState::Error(
                "Saved session is no longer valid. Please log in again.".into(),
            ));
        }
    }
}

/// Resume: post-list secondary session data → vault.
pub fn handle_resume_session_data(app: &mut App, data: ParallelSessionData) {
    apply_parallel_session_data(app, data);
    app.go_to_vault();
    app.set_action(ActionState::Idle);
}

/// Applies the four secondary post-auth reads (folders, orgs,
/// collections, import-formats) fetched in one trip by
/// [`crate::ports::VaultPort::parallel_session_data`]. Partial failures
/// are surfaced through the same `cmd_log` lines the individual silent
/// refreshes used to write.
fn apply_parallel_session_data(app: &mut App, data: ParallelSessionData) {
    match data.folders {
        Ok(folders) => {
            let count = folders.len();
            let mut sorted = folders;
            sorted.sort_by_key(|f| f.name.to_lowercase());
            app.folders = sorted;
            app.folder_selected = crate::tui::folders::row_for_filter(
                &app.active_folder,
                &app.folders,
                &app.collections,
            );
            app.push_cmd("bw list folders", true, &format!("{count} folders loaded"));
        }
        Err(e) => app.cmd_err("bw list folders", &e, "Load folders failed"),
    }

    match data.organizations {
        Ok(orgs) => {
            let count = orgs.len();
            app.organizations = orgs;
            app.push_cmd(
                "bw list organizations",
                true,
                &format!("{count} organisations loaded"),
            );
        }
        Err(e) => {
            app.push_cmd("bw list organizations", false, &e);
            app.organizations.clear();
        }
    }

    match data.collections {
        Ok(mut cs) => {
            cs.sort_by(|a, b| {
                a.organization_id
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.organization_id.as_deref().unwrap_or(""))
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
            let count = cs.len();
            app.collections = cs;
            app.push_cmd(
                "bw list collections",
                true,
                &format!("{count} collections loaded"),
            );
        }
        Err(e) => {
            app.push_cmd("bw list collections", false, &e);
            app.collections.clear();
        }
    }

    if let Ok(formats) = data.import_formats {
        app.import_formats = formats;
    }
}

/// Applies a "locked-but-known-account" UI state.
fn apply_locked_state(app: &mut App, user_email: Option<String>) {
    if let Some(email) = user_email
        && !email.is_empty()
        && app.email_input.is_empty()
    {
        app.email_cursor = email.chars().count();
        app.email_input = email;
    }
    app.active_field = LoginField::Password;
}

// ── Login / unlock ────────────────────────────────────────────────────────

/// Validates the login form and dispatches the right worker request
/// (unlock, fresh login, or resume an OTP / 2FA challenge).
pub fn attempt_login(app: &mut App) {
    if app.password_input.is_empty() {
        app.login_error = true;
        return;
    }
    if let Err(msg) = crate::domain::validation::validate_email(&app.email_input) {
        app.set_action(ActionState::Error(msg.into()));
        app.active_field = LoginField::Email;
        return;
    }
    request_login(app);
}

/// Sends the appropriate login/unlock request to the worker.
fn request_login(app: &mut App) {
    let email = app.email_input.clone();
    let password = app.password_input.clone();
    app.set_action(ActionState::Running("Logging in…".into()));

    if app.otp_required || app.two_factor_required {
        let code = Zeroizing::new(app.otp_input.trim().to_string());
        if app.two_factor_required {
            app.in_flight = Some(InFlight::LoginTwoFactor);
            let _ = app.worker_tx.send(WorkerRequest::LoginTwoFactor {
                email,
                password,
                code,
                method: app.two_factor_method,
            });
        } else {
            app.in_flight = Some(InFlight::LoginOtp);
            let _ = app.worker_tx.send(WorkerRequest::LoginOtp {
                email,
                password,
                otp: code,
            });
        }
    } else if app.authenticated {
        app.in_flight = Some(InFlight::Unlock);
        let _ = app.worker_tx.send(WorkerRequest::Unlock { password });
    } else {
        app.in_flight = Some(InFlight::Login);
        let _ = app.worker_tx.send(WorkerRequest::Login { email, password });
    }
}

/// Fresh `bw login` outcome.
pub fn handle_login(app: &mut App, outcome: LoginOutcome) {
    match outcome {
        LoginOutcome::Success(key) => {
            app.push_cmd("bw login *** --raw", true, "logged in");
            on_login_success(app, &key);
        }
        LoginOutcome::NeedsDeviceVerification => {
            app.push_cmd(
                "bw login *** --raw",
                true,
                "device verification required — OTP sent",
            );
            app.set_action(ActionState::Idle);
            app.otp_required = true;
            app.two_factor_required = false;
            app.otp_input.clear();
            app.otp_cursor = 0;
            app.active_field = LoginField::Otp;
        }
        LoginOutcome::NeedsTwoFactor => {
            app.push_cmd(
                "bw login *** --raw",
                true,
                "two-factor required — pick method and enter code",
            );
            app.set_action(ActionState::Idle);
            app.two_factor_required = true;
            app.two_factor_method = crate::domain::TwoFactorMethod::Authenticator;
            app.otp_required = false;
            app.otp_input.clear();
            app.otp_cursor = 0;
            app.active_field = LoginField::Otp;
        }
        LoginOutcome::Failed(err) => {
            app.push_cmd("bw login *** --raw", false, &err);
            app.set_action(ActionState::Idle);
            app.set_login_error();
        }
    }
}

/// `bw unlock` response.
pub fn handle_unlock(app: &mut App, r: Result<String, BwError>) {
    match r {
        Ok(key) => on_login_success(app, &key),
        Err(_) => {
            app.push_cmd("bw unlock ***", false, "invalid credentials");
            app.set_action(ActionState::Idle);
            app.set_login_error();
        }
    }
}

/// `bw login` resuming a new-device verification OTP.
pub fn handle_login_otp(app: &mut App, r: Result<String, BwError>) {
    let cmd = "bw login *** --raw  (otp via stdin)";
    match r {
        Ok(key) => {
            app.otp_input.clear();
            app.otp_cursor = 0;
            app.otp_required = false;
            app.two_factor_required = false;
            app.push_cmd(cmd, true, "verified");
            on_login_success(app, &key);
        }
        Err(_) => fail_code(app, cmd, "Invalid verification code"),
    }
}

/// `bw login --method N` resuming a permanent-2FA challenge.
pub fn handle_login_two_factor(app: &mut App, r: Result<String, BwError>) {
    let cmd = format!(
        "bw login *** --method {} --raw  (code via stdin)",
        app.two_factor_method.as_u8()
    );
    match r {
        Ok(key) => {
            app.otp_input.clear();
            app.otp_cursor = 0;
            app.otp_required = false;
            app.two_factor_required = false;
            app.push_cmd(&cmd, true, "verified");
            on_login_success(app, &key);
        }
        Err(_) => fail_code(app, &cmd, "Invalid 2FA code"),
    }
}

/// Shared failure tail for an OTP / 2FA code rejection.
fn fail_code(app: &mut App, cmd: &str, label: &str) {
    app.push_cmd(cmd, false, label);
    app.set_action(ActionState::Idle);
    app.otp_input.clear();
    app.otp_cursor = 0;
    app.active_field = LoginField::Otp;
    app.login_error = true;
}

/// Common tail of every successful login / unlock path: caches the
/// session key, persists it if opted in, then chains the vault load.
fn on_login_success(app: &mut App, session_key: &str) {
    app.authenticated = true;
    app.session_marker = Some(Zeroizing::new(session_key.to_string()));
    if app.save_email {
        let email = app.email_input.clone();
        app.settings.write(true, Some(&email));
    }
    if app.keep_session && !session_key.is_empty() {
        session_file::save(session_key);
    }
    app.password_input.clear();
    app.password_cursor = 0;
    app.in_flight = Some(InFlight::PostLoginItems);
    app.set_action(ActionState::Running("Loading vault…".into()));
    let _ = app.worker_tx.send(WorkerRequest::ListItems);
}

/// Post-login: items loaded → fetch the secondary session data.
pub fn handle_post_login_items(app: &mut App, r: Result<Vec<Item>, BwError>) {
    match r {
        Ok(items) => {
            let count = items.len();
            app.items = items;
            app.sort_items();
            app.push_cmd("bw list items", true, &format!("{count} items loaded"));
            app.in_flight = Some(InFlight::PostLoginSessionData);
            let _ = app.worker_tx.send(WorkerRequest::ParallelSessionData);
        }
        Err(e) => {
            // Login succeeded but the first list failed — land the user
            // on the (empty) vault with the error surfaced rather than
            // stranding them on the login screen.
            app.cmd_err("bw list items", &e, "Load failed");
            app.go_to_vault();
        }
    }
}

/// Post-login: secondary session data → vault.
pub fn handle_post_login_session_data(app: &mut App, data: ParallelSessionData) {
    apply_parallel_session_data(app, data);
    app.set_action(ActionState::Done("Loaded ✓".into()));
    app.go_to_vault();
}

// ── API-key / SSO login (leave the vault Locked) ──────────────────────────

/// Attempts a headless login using `BW_CLIENTID` / `BW_CLIENTSECRET`.
pub fn api_key_login(app: &mut App) {
    if std::env::var("BW_CLIENTID").is_err() || std::env::var("BW_CLIENTSECRET").is_err() {
        app.set_action(ActionState::Error(
            "BW_CLIENTID and BW_CLIENTSECRET must be set in the environment.".into(),
        ));
        return;
    }
    app.set_action(ActionState::Running("API-key login…".into()));
    app.in_flight = Some(InFlight::LoginApiKey);
    let _ = app.worker_tx.send(WorkerRequest::LoginApiKey);
}

/// `bw login --apikey` response (vault left Locked).
pub fn handle_api_key(app: &mut App, r: Result<(), BwError>) {
    match r {
        Ok(()) => {
            app.push_cmd("bw login --apikey", true, "logged in via API key");
            app.authenticated = true;
            app.set_action(ActionState::Done(
                "Logged in via API key — enter master password to unlock.".into(),
            ));
            app.active_field = LoginField::Password;
        }
        Err(e) => app.cmd_err("bw login --apikey", &e, "API-key login failed"),
    }
}

/// SSO login via `bw login --sso` (opens the browser on the worker).
pub fn sso_login(app: &mut App) {
    app.set_action(ActionState::Running(
        "SSO login (check your browser)…".into(),
    ));
    app.in_flight = Some(InFlight::LoginSso);
    let _ = app.worker_tx.send(WorkerRequest::LoginSso);
}

/// `bw login --sso` response (vault left Locked).
pub fn handle_sso(app: &mut App, r: Result<(), BwError>) {
    match r {
        Ok(()) => {
            app.push_cmd("bw login --sso", true, "logged in via SSO");
            app.authenticated = true;
            app.set_action(ActionState::Done(
                "Logged in via SSO — enter master password to unlock.".into(),
            ));
            app.active_field = LoginField::Password;
        }
        Err(e) => app.cmd_err("bw login --sso", &e, "SSO login failed"),
    }
}

// ── Lock / logout / server / fingerprint ──────────────────────────────────

/// Locks the vault and returns to the login screen. The UI state is
/// reset immediately (secrets vanish at once) and `bw lock` is sent
/// fire-and-forget so the worker drops its key in the background.
pub fn lock_vault(app: &mut App) {
    let _ = app.worker_tx.send(WorkerRequest::Lock);
    // Drop any in-flight ticket so a late response can't repopulate the
    // list after we've cleared it.
    app.in_flight = None;
    session_file::clear();
    app.session_marker = None;
    app.screen = Screen::Login;
    app.items.clear();
    app.trashed_items.clear();
    app.collections.clear();
    app.organizations.clear();
    app.rebuild_caches();
    app.password_input.clear();
    app.password_cursor = 0;
    app.active_field = LoginField::Password;
    app.push_cmd("bw lock", true, "vault locked");
    app.set_action(ActionState::Done("Locked ✓".into()));
}

/// Opens the confirm-logout popup over the vault.
pub fn open_confirm_logout(app: &mut App) {
    app.screen = Screen::ConfirmLogout;
}

/// Persists a new server URL via `bw config server <url>` when the Server
/// field differs from the last committed value.
pub fn commit_server_change(app: &mut App) {
    let url = app.server_input.trim().to_string();
    if url.is_empty() || url == app.server_committed {
        return;
    }
    if let Err(msg) = crate::domain::validation::validate_server_url(&url) {
        app.set_action(ActionState::Error(msg.into()));
        app.active_field = LoginField::Server;
        return;
    }
    app.set_action(ActionState::Running("Setting server…".into()));
    app.in_flight = Some(InFlight::SetServer);
    let _ = app.worker_tx.send(WorkerRequest::SetServer { url });
}

/// `bw config server` response.
pub fn handle_set_server(app: &mut App, r: Result<(), BwError>) {
    let url = app.server_input.trim().to_string();
    match r {
        Ok(()) => {
            app.push_cmd(
                &format!("bw config server {url}"),
                true,
                "server URL updated",
            );
            app.server_committed = url;
            app.set_action(ActionState::Done("Server updated ✓".into()));
        }
        Err(e) => {
            app.cmd_err(&format!("bw config server {url}"), &e, "Set server failed");
            let cmt = app.server_committed.clone();
            app.server_cursor = cmt.chars().count();
            app.server_input = cmt;
        }
    }
}

/// Logs out of the current account.
pub fn logout(app: &mut App) {
    app.set_action(ActionState::Running("Logging out…".into()));
    app.in_flight = Some(InFlight::Logout);
    let _ = app.worker_tx.send(WorkerRequest::Logout);
}

/// `bw logout` response.
pub fn handle_logout(app: &mut App, r: Result<(), BwError>) {
    match r {
        Ok(()) => {
            session_file::clear();
            app.push_cmd("bw logout", true, "account removed from local CLI");
            app.authenticated = false;
            app.session_marker = None;
            app.items.clear();
            app.trashed_items.clear();
            app.folders.clear();
            app.collections.clear();
            app.organizations.clear();
            app.rebuild_caches();
            app.password_input.clear();
            app.password_cursor = 0;
            app.otp_input.clear();
            app.otp_cursor = 0;
            app.otp_required = false;
            app.two_factor_required = false;
            app.email_input.clear();
            app.email_cursor = 0;
            app.search_query.clear();
            app.selected_index = 0;
            app.scroll_offset = 0;
            // Wipe the command log: it can carry item names, folder ids,
            // export/import paths and the user's e-mail.
            app.cmd_log.clear();
            app.cmd_log_scroll = 0;
            app.screen = Screen::Login;
            app.active_field = LoginField::Email;
            app.set_action(ActionState::Done("Logged out ✓".into()));
        }
        Err(e) => app.cmd_err("bw logout", &e, "Logout failed"),
    }
}

/// Fetches the current user's fingerprint phrase.
pub fn show_fingerprint(app: &mut App) {
    app.set_action(ActionState::Running("Fetching fingerprint…".into()));
    app.in_flight = Some(InFlight::Fingerprint);
    let _ = app.worker_tx.send(WorkerRequest::GetFingerprint);
}

/// `bw get fingerprint me` response.
pub fn handle_fingerprint(app: &mut App, r: Result<String, BwError>) {
    match r {
        Ok(phrase) => {
            app.push_cmd("bw get fingerprint me", true, &phrase);
            app.set_action(ActionState::Done(format!("🔑 {phrase}")));
        }
        Err(e) => app.cmd_err("bw get fingerprint me", &e, "Fingerprint failed"),
    }
}

/// Locks the vault if the inactivity timer has elapsed. No-op while a
/// request is in flight so an auto-lock can't race a pending response.
pub fn check_auto_lock(app: &mut App) {
    if !app.auto_lock || app.is_busy() {
        return;
    }
    if app.screen != Screen::Vault && app.screen != Screen::Detail {
        return;
    }
    if app.last_activity.elapsed().as_secs() >= app.lock_after_secs {
        lock_vault(app);
    }
}
