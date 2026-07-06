//! Key handler for the command palette (`Ctrl+P`).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::App;
use crate::tui::flows::palette;
use crate::tui::input::common::route_line_editor;

/// Runs the command row under the pointer, if any.
pub fn mouse(app: &mut App, col: u16, row: u16) {
    if let Some(fi) = crate::tui::view::palette::palette_row_at(col, row) {
        if let Some(state) = app.palette.as_mut() {
            state.selected = fi;
        }
        palette::run_selected(app);
    }
}

/// Dispatches a single key event on the command palette.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return palette::cancel(app),
        KeyCode::Enter => return palette::run_selected(app),
        KeyCode::Up => return palette::move_selection(app, -1),
        KeyCode::Down => return palette::move_selection(app, 1),
        // fzf-style selection moves that work while the query is typing.
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return palette::move_selection(app, 1);
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return palette::move_selection(app, -1);
        }
        _ => {}
    }
    // Otherwise the key edits the fuzzy query.
    if let Some(state) = app.palette.as_mut()
        && route_line_editor(&mut state.query, key)
    {
        palette::rebuild_filter(app);
    }
}
