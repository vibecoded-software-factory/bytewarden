//! Key handler for the login / unlock screen.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::auth::{api_key_login, attempt_login, commit_server_change, sso_login};
use crate::tui::input::is_alt;
use crate::tui::screens::LoginField;

/// Dispatches a single key event on the login screen.
pub fn handle(app: &mut App, key: KeyEvent) {
    // Alt+K — headless API-key login (reads BW_CLIENTID + BW_CLIENTSECRET
    // from the environment). Available as a global shortcut on the
    // login screen.
    if key.code == KeyCode::Char('k') && is_alt(&key) {
        return api_key_login(app);
    }
    // Alt+S — SSO login (opens the user's browser and blocks until
    // the federated callback arrives).
    if key.code == KeyCode::Char('s') && is_alt(&key) {
        return sso_login(app);
    }

    match key.code {
        KeyCode::Tab => {
            // Whenever focus leaves the Server field, persist any
            // change so the user does not have to remember a separate
            // commit key.
            let leaving_server = app.active_field == LoginField::Server;
            app.active_field = match app.active_field {
                LoginField::Server => LoginField::Email,
                LoginField::Email => LoginField::Password,
                LoginField::Password => {
                    if app.awaiting_code() {
                        LoginField::Otp
                    } else {
                        LoginField::SaveEmail
                    }
                }
                LoginField::Otp => LoginField::SaveEmail,
                LoginField::SaveEmail => LoginField::AutoLock,
                LoginField::AutoLock => LoginField::KeepSession,
                LoginField::KeepSession => LoginField::Server,
            };
            if leaving_server {
                commit_server_change(app);
            }
        }
        KeyCode::BackTab => {
            let leaving_server = app.active_field == LoginField::Server;
            app.active_field = match app.active_field {
                LoginField::KeepSession => LoginField::AutoLock,
                LoginField::AutoLock => LoginField::SaveEmail,
                LoginField::SaveEmail => {
                    if app.awaiting_code() {
                        LoginField::Otp
                    } else {
                        LoginField::Password
                    }
                }
                LoginField::Otp => LoginField::Password,
                LoginField::Password => LoginField::Email,
                LoginField::Email => LoginField::Server,
                LoginField::Server => LoginField::KeepSession,
            };
            if leaving_server {
                commit_server_change(app);
            }
        }
        KeyCode::Char(' ') if app.active_field == LoginField::SaveEmail => {
            app.toggle_save_email();
        }
        KeyCode::Char(' ') if app.active_field == LoginField::AutoLock => {
            app.auto_lock = !app.auto_lock;
            app.settings.write_auto_lock(app.auto_lock);
        }
        KeyCode::Char(' ') if app.active_field == LoginField::KeepSession => {
            app.toggle_keep_session();
        }
        KeyCode::Enter => {
            // On the Server field, Enter commits the URL change in
            // place instead of submitting the login form.
            if app.active_field == LoginField::Server {
                commit_server_change(app);
            } else {
                attempt_login(app);
            }
        }
        KeyCode::F(2) => app.login_password_visible = !app.login_password_visible,
        // ← → on the Otp field cycles the 2FA method when in 2FA
        // mode (Authenticator / Email / YubiKey). On any other text
        // field they keep their normal cursor-movement role.
        KeyCode::Left if app.two_factor_required && app.active_field == LoginField::Otp => {
            app.two_factor_method = app.two_factor_method.prev();
        }
        KeyCode::Right if app.two_factor_required && app.active_field == LoginField::Otp => {
            app.two_factor_method = app.two_factor_method.next();
        }
        // Everything else drives the focused field's `LineEditor` (the
        // shared input model). `login_editor_mut` returns `None` on the
        // checkbox fields, so typing there is a no-op. An edit clears a
        // stale login error and persists the e-mail when opted in.
        _ => {
            let changed = match app.login_editor_mut() {
                Some(ed) => crate::tui::input::common::route_line_editor(ed, key),
                None => false,
            };
            if changed {
                app.clear_login_error();
                app.persist_email_if_saving();
            }
        }
    }
}
