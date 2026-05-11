//! Authentication, lock and session-resume flows.

use zeroize::Zeroizing;

use crate::domain::vault_info::{LoginOutcome, VaultStatus};
use crate::tui::action::{ActionState, PendingAction};
use crate::tui::app::App;
use crate::tui::screens::{LoginField, Screen};
use crate::tui::session_file;

/// Reads `bw status` once at boot and either:
///
/// * Restores an existing session from `$BW_SESSION` if available,
/// * Pre-fills the email field for an existing locked account, or
/// * Falls through to the login screen.
pub fn resume_from_status(app: &mut App) {
    let info = match app.vault.status() {
        Ok(i) => i,
        Err(e) => {
            app.push_cmd("bw status", false, &e);
            return;
        }
    };
    app.push_cmd("bw status", true, &format!("{:?}", info.status));

    // Seed the Server field from the CLI's reported URL so the user
    // can see (and edit) which backend they're hitting.
    let server = info
        .server_url
        .clone()
        .unwrap_or_else(|| "https://bitwarden.com".to_string());
    app.server_cursor = server.chars().count();
    app.server_input = server.clone();
    app.server_committed = server;

    match info.status {
        VaultStatus::Unlocked => {
            // The CLI reports an unlocked vault. If the adapter picked
            // up a session key (typically from `$BW_SESSION`) we try to
            // use it directly; otherwise we fall through to the login
            // form like the locked case.
            if app.vault.session_key().is_some() {
                let email_for_form = info.user_email.clone();
                if try_resume_session(app, info.user_email) {
                    return;
                }
                // The session key was syntactically present but the
                // backend rejected it (stale key, server changed,
                // logged out elsewhere…). Fall back to the login form
                // and surface a hint so the user knows what happened.
                apply_locked_state(app, email_for_form);
                app.set_action(ActionState::Error(
                    "Saved session is no longer valid. Please log in again.".into(),
                ));
                return;
            }
            apply_locked_state(app, info.user_email);
        }
        VaultStatus::Locked => apply_locked_state(app, info.user_email),
        VaultStatus::Unauthenticated => {}
    }
}

/// Attempts to resume a previously-exported session key by listing
/// items with it. Returns `true` when the key works and the vault has
/// been loaded; `false` when the backend rejected the key.
fn try_resume_session(app: &mut App, user_email: Option<String>) -> bool {
    if app.email_input.is_empty()
        && let Some(email) = user_email
    {
        app.email_cursor = email.chars().count();
        app.email_input = email;
    }
    match app.vault.list_items() {
        Ok(items) => {
            let count = items.len();
            app.items = items;
            app.sort_items();
            app.push_cmd("bw status", true, "session resumed");
            app.push_cmd("bw list items", true, &format!("{count} items loaded"));
            apply_parallel_session_data(app);
            app.go_to_vault();
            true
        }
        Err(e) => {
            app.push_cmd("bw list items", false, &e);
            false
        }
    }
}

/// Fires the four secondary post-auth reads (folders, orgs,
/// collections, import-formats) in one trip via
/// [`crate::ports::VaultPort::parallel_session_data`] and applies the
/// results to `App`. The `BwCliAdapter` impl spawns one worker thread
/// per call so the dominant cost (Node cold-start, ~500 ms each) is
/// paid concurrently — login gets its sidebar populated in roughly
/// the time of a single bw invocation instead of four back-to-back.
///
/// Failures are surfaced through the same `cmd_log` lines the
/// individual silent refreshes used to write, so the operator gets
/// the same diagnostic trail. We keep the post-login folder
/// highlight in sync with `active_folder` even when the folder fetch
/// fails, since the sidebar still has to render.
fn apply_parallel_session_data(app: &mut App) {
    let data = app.vault.parallel_session_data();

    match data.folders {
        Ok(folders) => {
            let count = folders.len();
            // Mirror the alphabetical sort the silent-refresh helper
            // applied — the renderer relies on it.
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
        Err(e) => {
            app.cmd_err("bw list folders", &e, "Load folders failed");
        }
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
            // Same sort key as `refresh_memberships_silent` so the
            // sidebar order stays deterministic.
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

// ── Triggered flows ───────────────────────────────────────────────────────

/// Validates the login form and queues a [`PendingAction::Login`].
pub fn attempt_login(app: &mut App) {
    if app.password_input.is_empty() {
        app.login_error = true;
        return;
    }
    // Email shape check: catches missing-`@` / missing-TLD typos
    // before paying for a full network round-trip.
    if let Err(msg) = crate::domain::validation::validate_email(&app.email_input) {
        app.set_action(ActionState::Error(msg.into()));
        app.active_field = LoginField::Email;
        return;
    }
    app.set_action(ActionState::Running("Logging in…".into()));
    app.pending_action = PendingAction::Login;
}

/// Executes the login pending action — branches between unlock, fresh
/// login, and login-with-OTP.
pub fn do_login(app: &mut App) {
    let email = app.email_input.clone();
    let password = app.password_input.clone();

    let already_auth = matches!(
        app.vault.status().map(|s| s.status),
        Ok(VaultStatus::Locked) | Ok(VaultStatus::Unlocked)
    );

    // Vault is locked but authenticated — only need to unlock.
    if already_auth {
        match app.vault.unlock(&password) {
            Ok(_) => on_login_success(app, &email),
            Err(_) => {
                app.push_cmd("bw unlock ***", false, "invalid credentials");
                app.set_action(ActionState::Idle);
                app.set_login_error();
            }
        }
        return;
    }

    // Resume of an interactive challenge — the previous login attempt
    // returned NeedsDeviceVerification or NeedsTwoFactor and the user
    // has now typed their code. The two paths reuse the same buffer
    // (`otp_input`) but call into different port methods — bw needs
    // `--method N` only for permanent 2FA.
    if app.otp_required || app.two_factor_required {
        // Wrap the trimmed copy so even this short-lived intermediate
        // is wiped from the heap once the call returns.
        let code = Zeroizing::new(app.otp_input.trim().to_string());
        let result = if app.two_factor_required {
            app.vault
                .login_with_two_factor(&email, &password, &code, app.two_factor_method)
        } else {
            app.vault.login_with_otp(&email, &password, &code)
        };
        let cmd_label_owned;
        let (cmd_label, error_label): (&str, &str) = if app.two_factor_required {
            cmd_label_owned = format!(
                "bw login *** --method {} --raw  (code via stdin)",
                app.two_factor_method.as_u8()
            );
            (cmd_label_owned.as_str(), "Invalid 2FA code")
        } else {
            (
                "bw login *** --raw  (otp via stdin)",
                "Invalid verification code",
            )
        };
        match result {
            Ok(_) => {
                app.otp_input.clear();
                app.otp_cursor = 0;
                app.otp_required = false;
                app.two_factor_required = false;
                on_login_success(app, &email);
            }
            Err(_) => {
                // The redacted log reflects that the code never reached
                // argv (env var for password, stdin for code).
                app.push_cmd(cmd_label, false, error_label);
                app.set_action(ActionState::Idle);
                app.otp_input.clear();
                app.otp_cursor = 0;
                app.active_field = LoginField::Otp;
                app.login_error = true;
            }
        }
        return;
    }

    match app.vault.login(&email, &password) {
        LoginOutcome::Success(_) => on_login_success(app, &email),
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
            // Default to Authenticator — the most common method.
            // The user can cycle to Email / YubiKey from the form.
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

/// Common tail of every successful login path.
fn on_login_success(app: &mut App, email: &str) {
    if app.save_email {
        app.settings.write(true, Some(email));
    }
    // Persist the freshly-issued session key for the lifetime of the
    // parent shell, if the user opted in. Best-effort — failure here
    // just means they'll have to re-enter the master password next
    // launch.
    if app.keep_session
        && let Some(key) = app.vault.session_key()
    {
        session_file::save(key);
    }
    app.password_input.clear();
    app.password_cursor = 0;
    super::vault::load_items(app);
    // Folders, organisations, collections and import-formats run in
    // parallel — see `apply_parallel_session_data` for the rationale
    // (Node cold-start dominates and parallelising drops post-login
    // latency from ~4× to ~1× a single bw call).
    apply_parallel_session_data(app);
    app.set_action(ActionState::Done("Loaded ✓".into()));
    app.go_to_vault();
}

/// Attempts a headless login using `BW_CLIENTID` / `BW_CLIENTSECRET`
/// from the environment via `bw login --apikey`.
///
/// On success the bw CLI is logged in but the vault is locked — the
/// user still needs to type their master password and hit Enter to
/// unlock. We surface that explicitly via a Done toast so they know
/// what to do next.
pub fn api_key_login(app: &mut App) {
    if std::env::var("BW_CLIENTID").is_err() || std::env::var("BW_CLIENTSECRET").is_err() {
        app.set_action(ActionState::Error(
            "BW_CLIENTID and BW_CLIENTSECRET must be set in the environment.".into(),
        ));
        return;
    }
    app.set_action(ActionState::Running("API-key login…".into()));
    match app.vault.login_with_api_key() {
        Ok(()) => {
            app.push_cmd("bw login --apikey", true, "logged in via API key");
            app.set_action(ActionState::Done(
                "Logged in via API key — enter master password to unlock.".into(),
            ));
            app.active_field = LoginField::Password;
        }
        Err(e) => app.cmd_err("bw login --apikey", &e, "API-key login failed"),
    }
}

/// SSO login. Opens the user's browser via `bw login --sso` and
/// blocks until the federated callback arrives.
///
/// Like API-key login, on success the vault is Locked — the user
/// still needs to type their master password and hit Enter to
/// unlock. We surface that explicitly so the screen-frozen-during-
/// browser-flow surprise is at least followed by a clear next step.
pub fn sso_login(app: &mut App) {
    app.set_action(ActionState::Running(
        "SSO login (check your browser)…".into(),
    ));
    match app.vault.login_with_sso() {
        Ok(()) => {
            app.push_cmd("bw login --sso", true, "logged in via SSO");
            app.set_action(ActionState::Done(
                "Logged in via SSO — enter master password to unlock.".into(),
            ));
            app.active_field = LoginField::Password;
        }
        Err(e) => app.cmd_err("bw login --sso", &e, "SSO login failed"),
    }
}

/// Locks the vault and returns the user to the login screen.
pub fn lock_vault(app: &mut App) {
    app.vault.lock();
    // The persisted session key would let a subsequent launch skip
    // unlock — exactly what locking is meant to prevent.
    session_file::clear();
    app.screen = Screen::Login;
    app.items.clear();
    // Trashed items also carry full plaintext (the user may have
    // opened the trash view at some point in this session). Drop
    // them too so the heap-dump exposure window closes the moment
    // the vault is locked, not just on logout.
    app.trashed_items.clear();
    // Wipe organisation memberships too: the sidebar should not show
    // collection rows from the previous session while the vault is
    // locked. Folders are kept because they're a personal-vault
    // concept that the next unlock will refresh anyway.
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

/// Persists a new server URL via `bw config server <url>` if the
/// Server field on the login form differs from the last committed
/// value. No-op when the value is unchanged or empty.
///
/// `bw` rejects this when there is an active session, so the caller
/// should make sure the user is unauthenticated (the login screen
/// always is — locked or not, the master key is required to proceed).
pub fn commit_server_change(app: &mut App) {
    let url = app.server_input.trim().to_string();
    if url.is_empty() || url == app.server_committed {
        return;
    }
    if let Err(msg) = crate::domain::validation::validate_server_url(&url) {
        app.set_action(ActionState::Error(msg.into()));
        // Keep the user on the Server field so they can fix it.
        app.active_field = LoginField::Server;
        return;
    }
    app.set_action(ActionState::Running("Setting server…".into()));
    match app.vault.set_server(&url) {
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
            // Roll the field back to the committed value so the user
            // is not misled by what they typed.
            let cmt = app.server_committed.clone();
            app.server_cursor = cmt.chars().count();
            app.server_input = cmt;
        }
    }
}

/// Logs out of the current account, returning the user to a clean
/// login screen.
///
/// Distinct from [`lock_vault`]: lock keeps the account configured (so
/// the next launch only needs the master password); logout removes the
/// account from the local CLI entirely, so the next launch starts at
/// email entry.
pub fn logout(app: &mut App) {
    app.set_action(ActionState::Running("Logging out…".into()));
    match app.vault.logout() {
        Ok(()) => {
            session_file::clear();
            // Record the successful logout line first — every other
            // flow follows the same `push_cmd` pattern and dropping it
            // would leave a hole if the cmd_log clear below is ever
            // removed. The log is wiped a few lines down anyway.
            app.push_cmd("bw logout", true, "account removed from local CLI");
            // Reset every piece of session-bound UI state so nothing
            // survives the logout.
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
            // Wipe the command log: even with session-key redaction in
            // place, the log can still carry item names, folder ids,
            // export/import paths, and the user's e-mail — none of
            // which the next user (or the same user re-logging into a
            // different account) needs to see. Distinct from `lock`,
            // where preserving the log helps debugging an unlock cycle.
            app.cmd_log.clear();
            app.cmd_log_scroll = 0;
            app.screen = Screen::Login;
            app.active_field = LoginField::Email;
            app.set_action(ActionState::Done("Logged out ✓".into()));
        }
        Err(e) => app.cmd_err("bw logout", &e, "Logout failed"),
    }
}

/// Fetches the current user's fingerprint phrase via `bw get
/// fingerprint me` and surfaces it as a Done toast so the user can
/// read (and remember to share with admins for verification) without
/// leaving the vault screen.
///
/// The phrase has no privacy concerns — it is a public identifier
/// derived from the user's public key.
pub fn show_fingerprint(app: &mut App) {
    app.set_action(ActionState::Running("Fetching fingerprint…".into()));
    match app.vault.get_fingerprint() {
        Ok(phrase) => {
            app.push_cmd("bw get fingerprint me", true, &phrase);
            app.set_action(ActionState::Done(format!("🔑 {phrase}")));
        }
        Err(e) => app.cmd_err("bw get fingerprint me", &e, "Fingerprint failed"),
    }
}

/// Locks the vault if the inactivity timer has elapsed.
pub fn check_auto_lock(app: &mut App) {
    if !app.auto_lock {
        return;
    }
    if app.screen != Screen::Vault && app.screen != Screen::Detail {
        return;
    }
    if app.last_activity.elapsed().as_secs() >= app.lock_after_secs {
        lock_vault(app);
    }
}
