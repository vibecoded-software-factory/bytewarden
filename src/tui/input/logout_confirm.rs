use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::auth;
use crate::tui::screens::Screen;

pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('n') => app.screen = Screen::Vault,
        KeyCode::Enter => auth::logout(app),
        _ => {}
    }
}
