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
    let (input, cursor): (&mut String, &mut usize) = match focus {
        SendFocus::Name => (&mut state.name, &mut state.name_cursor),
        SendFocus::Content => (&mut state.content, &mut state.content_cursor),
        SendFocus::Days => return, // handled above
    };

    match key.code {
        KeyCode::Left => {
            if *cursor > 0 {
                *cursor -= 1;
            }
        }
        KeyCode::Right => {
            if *cursor < input.chars().count() {
                *cursor += 1;
            }
        }
        KeyCode::Home => *cursor = 0,
        KeyCode::End => *cursor = input.chars().count(),
        KeyCode::Backspace => {
            if *cursor > 0 {
                let byte = input
                    .char_indices()
                    .nth(*cursor - 1)
                    .map(|(b, _)| b)
                    .unwrap_or(0);
                input.remove(byte);
                *cursor -= 1;
            }
        }
        KeyCode::Delete => {
            if *cursor < input.chars().count() {
                let byte = input
                    .char_indices()
                    .nth(*cursor)
                    .map(|(b, _)| b)
                    .unwrap_or(0);
                input.remove(byte);
            }
        }
        KeyCode::Char(c) => {
            let byte = input
                .char_indices()
                .nth(*cursor)
                .map(|(b, _)| b)
                .unwrap_or(input.len());
            input.insert(byte, c);
            *cursor += 1;
        }
        _ => {}
    }
}
