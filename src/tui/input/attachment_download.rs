//! Key handler for the attachment-download popup.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::items::{cancel_attachment_download, queue_attachment_download};

/// Dispatches a single key event on the attachment-download popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel_attachment_download(app),
        KeyCode::Enter => return queue_attachment_download(app),
        _ => {}
    }

    let Some(state) = app.attachment_download.as_mut() else {
        return;
    };
    match key.code {
        KeyCode::Left if state.path_cursor > 0 => {
            state.path_cursor -= 1;
        }
        KeyCode::Right if state.path_cursor < state.path.chars().count() => {
            state.path_cursor += 1;
        }
        KeyCode::Home => state.path_cursor = 0,
        KeyCode::End => state.path_cursor = state.path.chars().count(),
        KeyCode::Backspace if state.path_cursor > 0 => {
            let byte = state
                .path
                .char_indices()
                .nth(state.path_cursor - 1)
                .map(|(b, _)| b)
                .unwrap_or(0);
            state.path.remove(byte);
            state.path_cursor -= 1;
        }
        KeyCode::Delete if state.path_cursor < state.path.chars().count() => {
            let byte = state
                .path
                .char_indices()
                .nth(state.path_cursor)
                .map(|(b, _)| b)
                .unwrap_or(0);
            state.path.remove(byte);
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
