use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::items::{cancel_delete_attachment, queue_delete_attachment};

pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('n') => cancel_delete_attachment(app),
        KeyCode::Enter => queue_delete_attachment(app),
        _ => {}
    }
}
