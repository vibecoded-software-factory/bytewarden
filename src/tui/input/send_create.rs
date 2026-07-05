//! Key handler for the send-create popup.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::send::{adjust_days, cancel, commit, focus_step};
use crate::tui::send::SendFocus;

/// Dispatches a single key event on the send-create popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel(app),
        KeyCode::Enter => return commit(app),
        KeyCode::Tab | KeyCode::Down => return focus_step(app, 1),
        KeyCode::BackTab | KeyCode::Up => return focus_step(app, -1),
        _ => {}
    }

    let focus = app
        .send_create
        .as_ref()
        .map(|s| s.focus)
        .unwrap_or(SendFocus::Name);

    if matches!(focus, SendFocus::Days) {
        match key.code {
            KeyCode::Right | KeyCode::Char('+') => return adjust_days(app, 1),
            KeyCode::Left | KeyCode::Char('-') => return adjust_days(app, -1),
            _ => return,
        }
    }

    let Some(state) = app.send_create.as_mut() else {
        return;
    };
    let editor = match focus {
        SendFocus::Name => &mut state.name,
        SendFocus::Content => &mut state.content,
        SendFocus::Days => return, // handled above
    };
    crate::tui::input::common::route_line_editor(editor, key);
}
