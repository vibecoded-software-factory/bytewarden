//! Key handler for the confirm-logout popup.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::auth;
use crate::tui::screens::Screen;

/// Dispatches a single key event on the confirm-logout popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('n') => app.screen = Screen::Vault,
        KeyCode::Enter => auth::logout(app),
        _ => {}
    }
}
