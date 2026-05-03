//! Key handler for the confirm-delete-attachment popup.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::items::{cancel_delete_attachment, queue_delete_attachment};

/// Dispatches a single key event on the confirm-delete-attachment popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('n') => cancel_delete_attachment(app),
        KeyCode::Enter => queue_delete_attachment(app),
        _ => {}
    }
}
