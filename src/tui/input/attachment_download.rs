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
    crate::tui::input::common::route_line_editor(&mut state.path, key);
}
