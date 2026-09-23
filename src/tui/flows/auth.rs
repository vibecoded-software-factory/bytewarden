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

pub fn request_resume(app: &mut App) {
    if app.begin(InFlight::BootStatus) {
        let _ = app.worker_tx.send(WorkerRequest::Status);
    }
}

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

    let server = info
        .server_url
        .clone()
        .unwrap_or_else(|| "https://bitwarden.com".to_string());
    app.login.server_input.set(server.clone());
    app.login.server_committed = server;

    match info.status {
        VaultStatus::Unlocked => {
            app.authenticated = true;
            if let Some(email) = info.user_email.clone()
                && app.login.email_input.is_empty()
            {
                app.login.email_input.set(email);
            }
            app.submit(
                InFlight::ResumeItems,
                "Resuming session…",
                WorkerRequest::ListItems,
            );
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

pub fn handle_resume_items(app: &mut App, r: Result<Vec<Item>, BwError>) {
    match r {
        Ok(items) => {
            let count = items.len();
            app.vault.items = items;
            app.vault.sort_items();
            app.push_cmd("bw list items", true, &format!("{count} items loaded"));

            if app.begin(InFlight::ResumeSessionData) {
                let _ = app.worker_tx.send(WorkerRequest::ParallelSessionData);
            }
        }
        Err(e) => {
            app.push_cmd("bw list items", false, &e);
            apply_locked_state(app, None);
            app.screen = Screen::Login;
            app.set_action(ActionState::Error(
                "Saved session is no longer valid. Please log in again.".into(),
            ));
        }
    }
}

pub fn handle_resume_session_data(app: &mut App, data: ParallelSessionData) {
    apply_parallel_session_data(app, data);
    app.go_to_vault();
    app.set_action(ActionState::Idle);
}

fn apply_parallel_session_data(app: &mut App, data: ParallelSessionData) {
    match data.folders {
        Ok(folders) => {
            let count = folders.len();
            let mut sorted = folders;
            sorted.sort_by_key(|f| f.name.to_lowercase());
            app.folders = sorted;
            app.vault.folder_selected = crate::tui::folders::row_for_filter(
                &app.vault.active_folder,
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

fn apply_locked_state(app: &mut App, user_email: Option<String>) {
    if let Some(email) = user_email
        && !email.is_empty()
        && app.login.email_input.is_empty()
    {
        app.login.email_input.set(email);
    }
    app.login.active_field = LoginField::Password;
}

pub fn attempt_login(app: &mut App) {
    if app.login.password_input.is_empty() {
        app.login.login_error = true;
        return;
    }
    if let Err(msg) = crate::domain::validation::validate_email(app.login.email_input.text()) {
        app.set_action(ActionState::Error(msg.into()));
        app.login.active_field = LoginField::Email;
        return;
    }
    request_login(app);
}

fn request_login(app: &mut App) {
    let email = app.login.email_input.text().to_string();
    let password = Zeroizing::new(app.login.password_input.text().to_string());

    if app.login.otp_required || app.login.two_factor_required {
        let code = Zeroizing::new(app.login.otp_input.text().trim().to_string());
        if app.login.two_factor_required {
            let method = app.login.two_factor_method;
            app.submit(
                InFlight::LoginTwoFactor,
                "Logging in…",
                WorkerRequest::LoginTwoFactor {
                    email,
                    password,
                    code,
                    method,
                },
            );
        } else {
            app.submit(
                InFlight::LoginOtp,
                "Logging in…",
                WorkerRequest::LoginOtp {
                    email,
                    password,
                    otp: code,
                },
            );
        }
    } else if app.authenticated {
        app.submit(
            InFlight::Unlock,
            "Logging in…",
            WorkerRequest::Unlock { password },
        );
    } else {
        app.submit(
            InFlight::Login,
            "Logging in…",
            WorkerRequest::Login { email, password },
        );
    }
}

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
            app.login.otp_required = true;
            app.login.two_factor_required = false;
            app.login.otp_input.clear();
            app.login.active_field = LoginField::Otp;
        }
        LoginOutcome::NeedsTwoFactor => {
            app.push_cmd(
                "bw login *** --raw",
                true,
                "two-factor required — pick method and enter code",
            );
            app.set_action(ActionState::Idle);
            app.login.two_factor_required = true;
            app.login.two_factor_method = crate::domain::TwoFactorMethod::Authenticator;
            app.login.otp_required = false;
            app.login.otp_input.clear();
            app.login.active_field = LoginField::Otp;
        }
        LoginOutcome::Failed(err) => {
            app.push_cmd("bw login *** --raw", false, &err);
            app.set_action(ActionState::Idle);
            app.login.set_error();
        }
    }
}

pub fn handle_unlock(app: &mut App, r: Result<String, BwError>) {
    match r {
        Ok(key) => on_login_success(app, &key),
        Err(_) => {
            app.push_cmd("bw unlock ***", false, "invalid credentials");
            app.set_action(ActionState::Idle);
            app.login.set_error();
        }
    }
}

pub fn handle_login_otp(app: &mut App, r: Result<String, BwError>) {
    let cmd = "bw login *** --raw  (otp via stdin)";
    match r {
        Ok(key) => {
            app.login.otp_input.clear();
            app.login.otp_required = false;
            app.login.two_factor_required = false;
            app.push_cmd(cmd, true, "verified");
            on_login_success(app, &key);
        }
        Err(_) => fail_code(app, cmd, "Invalid verification code"),
    }
}

pub fn handle_login_two_factor(app: &mut App, r: Result<String, BwError>) {
    let cmd = format!(
        "bw login *** --method {} --raw  (code via stdin)",
        app.login.two_factor_method.as_u8()
    );
    match r {
        Ok(key) => {
            app.login.otp_input.clear();
            app.login.otp_required = false;
            app.login.two_factor_required = false;
            app.push_cmd(&cmd, true, "verified");
            on_login_success(app, &key);
        }
        Err(_) => fail_code(app, &cmd, "Invalid 2FA code"),
    }
}

fn fail_code(app: &mut App, cmd: &str, label: &str) {
    app.push_cmd(cmd, false, label);
    app.set_action(ActionState::Idle);
    app.login.otp_input.clear();
    app.login.active_field = LoginField::Otp;
    app.login.login_error = true;
}

fn on_login_success(app: &mut App, session_key: &str) {
    app.authenticated = true;
    app.session_marker = Some(Zeroizing::new(session_key.to_string()));
    if app.login.save_email {
        let email = app.login.email_input.text().to_string();
        app.settings.write(true, Some(&email));
    }
    if app.login.keep_session && !session_key.is_empty() {
        session_file::save(session_key);
    }
    app.login.password_input.clear();
    app.submit(
        InFlight::PostLoginItems,
        "Loading vault…",
        WorkerRequest::ListItems,
    );
}

pub fn handle_post_login_items(app: &mut App, r: Result<Vec<Item>, BwError>) {
    match r {
        Ok(items) => {
            let count = items.len();
            app.vault.items = items;
            app.vault.sort_items();
            app.push_cmd("bw list items", true, &format!("{count} items loaded"));

            if app.begin(InFlight::PostLoginSessionData) {
                let _ = app.worker_tx.send(WorkerRequest::ParallelSessionData);
            }
        }
        Err(e) => {
            app.cmd_err("bw list items", &e, "Load failed");
            app.go_to_vault();
        }
    }
}

pub fn handle_post_login_session_data(app: &mut App, data: ParallelSessionData) {
    apply_parallel_session_data(app, data);
    app.set_action(ActionState::Done("Loaded ✓".into()));
    app.go_to_vault();
}

pub fn api_key_login(app: &mut App) {
    if std::env::var("BW_CLIENTID").is_err() || std::env::var("BW_CLIENTSECRET").is_err() {
        app.set_action(ActionState::Error(
            "BW_CLIENTID and BW_CLIENTSECRET must be set in the environment.".into(),
        ));
        return;
    }
    app.submit(
        InFlight::LoginApiKey,
        "API-key login…",
        WorkerRequest::LoginApiKey,
    );
}

pub fn handle_api_key(app: &mut App, r: Result<(), BwError>) {
    match r {
        Ok(()) => {
            app.push_cmd("bw login --apikey", true, "logged in via API key");
            app.authenticated = true;
            app.set_action(ActionState::Done(
                "Logged in via API key — enter master password to unlock.".into(),
            ));
            app.login.active_field = LoginField::Password;
        }
        Err(e) => app.cmd_err("bw login --apikey", &e, "API-key login failed"),
    }
}

pub fn sso_login(app: &mut App) {
    app.submit(
        InFlight::LoginSso,
        "SSO login (check your browser)…",
        WorkerRequest::LoginSso,
    );
}

pub fn handle_sso(app: &mut App, r: Result<(), BwError>) {
    match r {
        Ok(()) => {
            app.push_cmd("bw login --sso", true, "logged in via SSO");
            app.authenticated = true;
            app.set_action(ActionState::Done(
                "Logged in via SSO — enter master password to unlock.".into(),
            ));
            app.login.active_field = LoginField::Password;
        }
        Err(e) => app.cmd_err("bw login --sso", &e, "SSO login failed"),
    }
}

pub fn lock_vault(app: &mut App) {
    let _ = app.worker_tx.send(WorkerRequest::Lock);

    app.in_flight = None;
    app.request_started = None;
    session_file::clear();
    app.session_marker = None;
    app.screen = Screen::Login;
    app.vault.items.clear();
    app.vault.trashed_items.clear();
    app.collections.clear();
    app.organizations.clear();
    app.vault.rebuild_caches();
    app.login.password_input.clear();
    app.login.active_field = LoginField::Password;
    app.push_cmd("bw lock", true, "vault locked");
    app.set_action(ActionState::Done("Locked ✓".into()));
}

pub fn open_confirm_logout(app: &mut App) {
    app.screen = Screen::ConfirmLogout;
}

pub fn commit_server_change(app: &mut App) {
    let url = app.login.server_input.text().trim().to_string();
    if url.is_empty() || url == app.login.server_committed {
        return;
    }
    if let Err(msg) = crate::domain::validation::validate_server_url(&url) {
        app.set_action(ActionState::Error(msg.into()));
        app.login.active_field = LoginField::Server;
        return;
    }
    app.submit(
        InFlight::SetServer,
        "Setting server…",
        WorkerRequest::SetServer { url },
    );
}

pub fn handle_set_server(app: &mut App, r: Result<(), BwError>) {
    let url = app.login.server_input.text().trim().to_string();
    match r {
        Ok(()) => {
            app.push_cmd(
                &format!("bw config server {url}"),
                true,
                "server URL updated",
            );
            app.login.server_committed = url;
            app.set_action(ActionState::Done("Server updated ✓".into()));
        }
        Err(e) => {
            app.cmd_err(&format!("bw config server {url}"), &e, "Set server failed");
            let cmt = app.login.server_committed.clone();
            app.login.server_input.set(cmt);
        }
    }
}

pub fn logout(app: &mut App) {
    app.submit(InFlight::Logout, "Logging out…", WorkerRequest::Logout);
}

pub fn handle_logout(app: &mut App, r: Result<(), BwError>) {
    match r {
        Ok(()) => {
            session_file::clear();
            app.push_cmd("bw logout", true, "account removed from local CLI");
            app.authenticated = false;
            app.session_marker = None;
            app.vault.items.clear();
            app.vault.trashed_items.clear();
            app.folders.clear();
            app.collections.clear();
            app.organizations.clear();
            app.vault.rebuild_caches();
            app.login.password_input.clear();
            app.login.otp_input.clear();
            app.login.otp_required = false;
            app.login.two_factor_required = false;
            app.login.email_input.clear();
            app.vault.search_query.clear();
            app.vault.selected_index = 0;
            app.vault.scroll_offset = 0;

            app.cmd_log.clear();
            app.screen = Screen::Login;
            app.login.active_field = LoginField::Email;
            app.set_action(ActionState::Done("Logged out ✓".into()));
        }
        Err(e) => app.cmd_err("bw logout", &e, "Logout failed"),
    }
}

pub fn show_fingerprint(app: &mut App) {
    app.submit(
        InFlight::Fingerprint,
        "Fetching fingerprint…",
        WorkerRequest::GetFingerprint,
    );
}

pub fn handle_fingerprint(app: &mut App, r: Result<String, BwError>) {
    match r {
        Ok(phrase) => {
            app.push_cmd("bw get fingerprint me", true, &phrase);
            app.set_action(ActionState::Done(format!("Fingerprint: {phrase}")));
        }
        Err(e) => app.cmd_err("bw get fingerprint me", &e, "Fingerprint failed"),
    }
}

pub fn check_auto_lock(app: &mut App) {
    if !app.auto_lock.enabled || app.is_busy() {
        return;
    }
    if app.screen != Screen::Vault && app.screen != Screen::Detail {
        return;
    }
    if app.auto_lock.is_expired() {
        lock_vault(app);
    }
}
