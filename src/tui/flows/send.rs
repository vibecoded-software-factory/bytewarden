//! Send-create popup flow.

use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::screens::Screen;
use crate::tui::send::{SEND_MAX_DAYS, SEND_MIN_DAYS, SendCreateState, SendFocus};

/// Opens the send-create popup with default values.
pub fn open(app: &mut App) {
    app.send_create = Some(SendCreateState::new());
    app.screen = Screen::SendCreate;
}

/// Closes the popup and returns to the vault list.
pub fn cancel(app: &mut App) {
    app.send_create = None;
    app.screen = Screen::Vault;
}

/// Cycles focus through Name → Days → Content → Name.
pub fn focus_step(app: &mut App, dir: i32) {
    let Some(state) = app.send_create.as_mut() else {
        return;
    };
    let order = [SendFocus::Name, SendFocus::Days, SendFocus::Content];
    let cur = order.iter().position(|f| *f == state.focus).unwrap_or(0);
    let next = if dir > 0 {
        (cur + 1) % order.len()
    } else {
        (cur + order.len() - 1) % order.len()
    };
    state.focus = order[next];
}

/// Bumps the days field up or down (clamped to [1, 31]).
pub fn adjust_days(app: &mut App, delta: i32) {
    let Some(state) = app.send_create.as_mut() else {
        return;
    };
    let new = (state.days as i32 + delta).clamp(SEND_MIN_DAYS as i32, SEND_MAX_DAYS as i32) as u8;
    state.days = new;
}

/// Calls `bw send` and copies the resulting URL to the clipboard so
/// the user can paste it wherever they need.
pub fn commit(app: &mut App) {
    let Some(state) = app.send_create.as_ref() else {
        return;
    };
    let name = state.name.trim().to_string();
    let content = state.content.clone();
    let days = state.days;
    if name.is_empty() {
        app.set_action(ActionState::Error("Name cannot be empty.".into()));
        return;
    }
    if content.is_empty() {
        app.set_action(ActionState::Error("Content cannot be empty.".into()));
        return;
    }

    let cmd = format!("bw send -n <name> -d {days} <content> --session ***");
    app.set_action(ActionState::Running("Creating Send…".into()));
    match app.vault.send_text(&name, days, &content) {
        Ok(url) => {
            app.push_cmd(&cmd, true, &format!("send url for \"{name}\""));
            // Try to drop the URL on the clipboard — the toast tells
            // the user whether that worked. Failure is non-fatal: the
            // URL is also visible in the toast text. The Send link is
            // a capability token (anyone with the URL can read the
            // content), so we honour the same clipboard auto-clear
            // window as for passwords.
            let ttl = app.clipboard_clear_secs;
            match app.clipboard.write_with_clear(&url, ttl) {
                Ok(()) => {
                    let clear_hint = if ttl == 0 {
                        String::new()
                    } else {
                        format!(", clears in {ttl}s")
                    };
                    app.set_action(ActionState::Done(format!(
                        "Send URL copied to clipboard ✓ (expires in {days}d{clear_hint})"
                    )));
                }
                Err(_) => {
                    app.set_action(ActionState::Done(format!("Send URL: {url}")));
                }
            }
            app.send_create = None;
            app.screen = Screen::Vault;
        }
        Err(e) => app.cmd_err(&cmd, &e, "Send failed"),
    }
}
