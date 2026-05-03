//! Key handler for the rename-custom-field popup.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::items::{cancel_rename_field, commit_rename_field};

/// Dispatches a single key event on the rename popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel_rename_field(app),
        KeyCode::Enter => return commit_rename_field(app),
        _ => {}
    }

    let Some(state) = app.rename_field.as_mut() else {
        return;
    };
    match key.code {
        KeyCode::Left => {
            if state.cursor > 0 {
                state.cursor -= 1;
            }
        }
        KeyCode::Right => {
            if state.cursor < state.input.chars().count() {
                state.cursor += 1;
            }
        }
        KeyCode::Home => state.cursor = 0,
        KeyCode::End => state.cursor = state.input.chars().count(),
        KeyCode::Backspace => {
            if state.cursor > 0 {
                let byte = state
                    .input
                    .char_indices()
                    .nth(state.cursor - 1)
                    .map(|(b, _)| b)
                    .unwrap_or(0);
                state.input.remove(byte);
                state.cursor -= 1;
            }
        }
        KeyCode::Delete => {
            if state.cursor < state.input.chars().count() {
                let byte = state
                    .input
                    .char_indices()
                    .nth(state.cursor)
                    .map(|(b, _)| b)
                    .unwrap_or(0);
                state.input.remove(byte);
            }
        }
        KeyCode::Char(c) => {
            let byte = state
                .input
                .char_indices()
                .nth(state.cursor)
                .map(|(b, _)| b)
                .unwrap_or(state.input.len());
            state.input.insert(byte, c);
            state.cursor += 1;
        }
        _ => {}
    }
}
