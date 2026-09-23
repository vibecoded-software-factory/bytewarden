use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::auth::{api_key_login, attempt_login, commit_server_change, sso_login};
use crate::tui::input::is_alt;
use crate::tui::screens::LoginField;

pub fn handle(app: &mut App, key: KeyEvent) {
    if key.code == KeyCode::Char('k') && is_alt(&key) {
        return api_key_login(app);
    }

    if key.code == KeyCode::Char('s') && is_alt(&key) {
        return sso_login(app);
    }

    match key.code {
        KeyCode::Tab => {
            let leaving_server = app.login.active_field == LoginField::Server;
            app.login.active_field = match app.login.active_field {
                LoginField::Server => LoginField::Email,
                LoginField::Email => LoginField::Password,
                LoginField::Password => {
                    if app.login.awaiting_code() {
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
            let leaving_server = app.login.active_field == LoginField::Server;
            app.login.active_field = match app.login.active_field {
                LoginField::KeepSession => LoginField::AutoLock,
                LoginField::AutoLock => LoginField::SaveEmail,
                LoginField::SaveEmail => {
                    if app.login.awaiting_code() {
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
        KeyCode::Char(' ') if app.login.active_field == LoginField::SaveEmail => {
            app.toggle_save_email();
        }
        KeyCode::Char(' ') if app.login.active_field == LoginField::AutoLock => {
            app.auto_lock.enabled = !app.auto_lock.enabled;
            app.settings.write_auto_lock(app.auto_lock.enabled);
        }
        KeyCode::Char(' ') if app.login.active_field == LoginField::KeepSession => {
            app.toggle_keep_session();
        }
        KeyCode::Enter => {
            if app.login.active_field == LoginField::Server {
                commit_server_change(app);
            } else {
                attempt_login(app);
            }
        }
        KeyCode::F(2) => app.login.password_visible = !app.login.password_visible,

        KeyCode::Left
            if app.login.two_factor_required && app.login.active_field == LoginField::Otp =>
        {
            app.login.two_factor_method = app.login.two_factor_method.prev();
        }
        KeyCode::Right
            if app.login.two_factor_required && app.login.active_field == LoginField::Otp =>
        {
            app.login.two_factor_method = app.login.two_factor_method.next();
        }

        _ => {
            let changed = match app.login.editor_mut() {
                Some(ed) => crate::tui::input::common::route_line_editor(ed, key),
                None => false,
            };
            if changed {
                app.login.clear_error();
                app.persist_email_if_saving();
            }
        }
    }
}
