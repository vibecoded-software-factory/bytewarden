//! Key handler for the multi-select collections-assignment popup.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::assign_collections::{cancel as cancel_popup, commit as commit_popup};

/// Dispatches a single key event on the assign-collections popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel_popup(app),
        KeyCode::Enter => return commit_popup(app),
        _ => {}
    }

    let Some(state) = app.assign_collections.as_mut() else {
        return;
    };
    // Any keystroke clears the error strip so the message doesn't
    // outlive the situation that produced it.
    state.error = false;

    let n = state.available.len();
    match key.code {
        KeyCode::Char('j') | KeyCode::Down if n > 0 && state.cursor + 1 < n => {
            state.cursor += 1;
        }
        KeyCode::Char('k') | KeyCode::Up if state.cursor > 0 => {
            state.cursor -= 1;
        }
        KeyCode::PageDown if n > 0 => {
            state.cursor = (state.cursor + 5).min(n - 1);
        }
        KeyCode::PageUp => {
            state.cursor = state.cursor.saturating_sub(5);
        }
        KeyCode::Home => state.cursor = 0,
        KeyCode::End if n > 0 => {
            state.cursor = n - 1;
        }
        KeyCode::Char(' ') => state.toggle_cursor(),
        _ => {}
    }
}
