//! Key handler for the memberships popup. Read-only — any key
//! (other than the implicit Ctrl+C global) closes it.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::memberships::close;

/// Dispatches a single key event on the memberships popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => close(app),
        _ => {}
    }
}
