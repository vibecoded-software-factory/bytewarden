use crate::ports::BwError;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::screens::Screen;
use crate::tui::send::{SEND_MAX_DAYS, SEND_MIN_DAYS, SendCreateState, SendFocus};
use crate::tui::worker::{InFlight, WorkerRequest};

pub fn open(app: &mut App) {
    app.send_create = Some(SendCreateState::new());
    app.screen = Screen::SendCreate;
}

pub fn cancel(app: &mut App) {
    app.send_create = None;
    app.screen = Screen::Vault;
}

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

pub fn adjust_days(app: &mut App, delta: i32) {
    let Some(state) = app.send_create.as_mut() else {
        return;
    };
    let new = (state.days as i32 + delta).clamp(SEND_MIN_DAYS as i32, SEND_MAX_DAYS as i32) as u8;
    state.days = new;
}

pub fn commit(app: &mut App) {
    let Some(state) = app.send_create.as_ref() else {
        return;
    };
    let name = state.name.text().trim().to_string();
    let content = state.content.text().to_string();
    let days = state.days;
    if name.is_empty() {
        app.set_action(ActionState::Error("Name cannot be empty.".into()));
        return;
    }
    if content.is_empty() {
        app.set_action(ActionState::Error("Content cannot be empty.".into()));
        return;
    }

    app.submit(
        InFlight::SendText,
        "Creating Send…",
        WorkerRequest::SendText {
            name,
            days,
            content,
        },
    );
}

pub fn handle(app: &mut App, r: Result<String, BwError>) {
    let days = app.send_create.as_ref().map(|s| s.days).unwrap_or(0);
    let cmd = format!("bw send -n <name> -d {days} <content>");
    match r {
        Ok(url) => {
            app.push_cmd(&cmd, true, "send url created");

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
