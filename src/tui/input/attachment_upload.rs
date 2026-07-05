//! Key handler for the attachment-upload popup.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::items::{cancel_attachment_upload, commit_attachment_upload};

/// Dispatches a single key event on the attachment-upload popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel_attachment_upload(app),
        KeyCode::Enter => return commit_attachment_upload(app),
        _ => {}
    }

    let Some(state) = app.attachment_upload.as_mut() else {
        return;
    };
    crate::tui::input::common::route_line_editor(&mut state.path, key);
}
