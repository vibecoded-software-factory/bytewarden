//! Key handler for the vault-export popup.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::export::ExportFocus;
use crate::tui::flows::export::{cancel, commit, cycle_format, focus_step};

/// Dispatches a single key event on the export popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel(app),
        KeyCode::Enter => return commit(app),
        KeyCode::Tab | KeyCode::Down => return focus_step(app, 1),
        KeyCode::BackTab | KeyCode::Up => return focus_step(app, -1),
        _ => {}
    }

    let Some(state) = app.export.as_mut() else {
        return;
    };

    match state.focus {
        ExportFocus::Format => match key.code {
            KeyCode::Char(' ') | KeyCode::Right | KeyCode::Left => cycle_format(app),
            _ => {}
        },
        ExportFocus::Path => {
            // Re-borrow because cycle_format above takes &mut App via
            // a flow function (not an issue here since we never call
            // it from this branch — keeping the comment as a hint).
            match key.code {
                KeyCode::Left => {
                    if state.path_cursor > 0 {
                        state.path_cursor -= 1;
                    }
                }
                KeyCode::Right => {
                    if state.path_cursor < state.path.chars().count() {
                        state.path_cursor += 1;
                    }
                }
                KeyCode::Home => state.path_cursor = 0,
                KeyCode::End => state.path_cursor = state.path.chars().count(),
                KeyCode::Backspace => {
                    if state.path_cursor > 0 {
                        let byte = state
                            .path
                            .char_indices()
                            .nth(state.path_cursor - 1)
                            .map(|(b, _)| b)
                            .unwrap_or(0);
                        state.path.remove(byte);
                        state.path_cursor -= 1;
                    }
                }
                KeyCode::Delete => {
                    if state.path_cursor < state.path.chars().count() {
                        let byte = state
                            .path
                            .char_indices()
                            .nth(state.path_cursor)
                            .map(|(b, _)| b)
                            .unwrap_or(0);
                        state.path.remove(byte);
                    }
                }
                KeyCode::Char(c) => {
                    let byte = state
                        .path
                        .char_indices()
                        .nth(state.path_cursor)
                        .map(|(b, _)| b)
                        .unwrap_or(state.path.len());
                    state.path.insert(byte, c);
                    state.path_cursor += 1;
                }
                _ => {}
            }
        }
    }
}
