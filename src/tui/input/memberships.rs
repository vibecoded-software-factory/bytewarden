use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::memberships::{close, move_cursor};

const PAGE: i8 = 5;

pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => close(app),
        KeyCode::Char('j') | KeyCode::Down => move_cursor(app, 1),
        KeyCode::Char('k') | KeyCode::Up => move_cursor(app, -1),
        KeyCode::PageDown => {
            for _ in 0..PAGE {
                move_cursor(app, 1);
            }
        }
        KeyCode::PageUp => {
            for _ in 0..PAGE {
                move_cursor(app, -1);
            }
        }
        KeyCode::Home => {
            if let Some(state) = app.memberships.as_mut() {
                state.cursor = 0;
            }
        }
        KeyCode::End => {
            if let Some(state) = app.memberships.as_mut() {
                state.cursor = state.selectable_len().saturating_sub(1);
            }
        }
        _ => {}
    }
}
