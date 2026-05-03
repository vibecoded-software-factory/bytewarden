//! Key handler for the confirm-delete-folder popup.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::folders::{cancel_delete, confirm_delete};

/// Dispatches a single key event on the confirm-delete-folder popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('n') => cancel_delete(app),
        KeyCode::Enter => confirm_delete(app),
        _ => {}
    }
}
