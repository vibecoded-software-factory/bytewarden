//! Key + mouse handler for the per-item action menu
//! (`Screen::ItemActions`).
//!
//! `j`/`k` (or arrows) move the highlight, `Enter`/`l` run the action,
//! `Esc` closes back to the vault. A left-click on a row runs that action
//! straight away (a context menu — one click acts).

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::item_actions;

/// Dispatches a single key event on the action menu.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => item_actions::close(app),
        KeyCode::Enter | KeyCode::Char('l') => item_actions::run_selected(app),
        KeyCode::Char('j') | KeyCode::Down => item_actions::move_cursor(app, 1),
        KeyCode::Char('k') | KeyCode::Up => item_actions::move_cursor(app, -1),
        _ => {}
    }
}

/// A click inside the menu runs the action under the pointer — the mouse
/// twin of highlighting it and pressing Enter. A click outside is handled
/// upstream as click-outside-to-dismiss.
pub fn mouse(app: &mut App, col: u16, row: u16) {
    if let Some(idx) = crate::tui::view::item_actions::item_action_at(col, row) {
        if let Some(state) = app.item_actions.as_mut() {
            state.cursor = idx;
        }
        item_actions::run_selected(app);
    }
}
