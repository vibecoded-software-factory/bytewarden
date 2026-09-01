use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::item_actions;

pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => item_actions::close(app),
        KeyCode::Enter | KeyCode::Char('l') => item_actions::run_selected(app),
        KeyCode::Char('j') | KeyCode::Down => item_actions::move_cursor(app, 1),
        KeyCode::Char('k') | KeyCode::Up => item_actions::move_cursor(app, -1),
        _ => {}
    }
}

pub fn mouse(app: &mut App, col: u16, row: u16) {
    if let Some(idx) = crate::tui::view::item_actions::item_action_at(col, row) {
        if let Some(state) = app.item_actions.as_mut() {
            state.cursor = idx;
        }
        item_actions::run_selected(app);
    }
}
