use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::assign_collections::{cancel as cancel_popup, commit as commit_popup};
use crate::tui::input::nav::nav_clamp;

const PAGE: usize = 5;

pub fn mouse(app: &mut App, col: u16, row: u16) {
    let Some(idx) = crate::tui::view::assign_collections::collection_row_at(col, row) else {
        return;
    };
    if let Some(state) = app.assign_collections.as_mut() {
        state.error = false;
        if idx < state.available.len() {
            state.cursor = idx;
            state.toggle_cursor();
        }
    }
}

pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel_popup(app),
        KeyCode::Enter => return commit_popup(app),
        _ => {}
    }

    let Some(state) = app.assign_collections.as_mut() else {
        return;
    };

    state.error = false;

    let n = state.available.len();
    let mut step = |dir: i8, times: usize| {
        for _ in 0..times {
            nav_clamp(&mut state.cursor, n, dir);
        }
    };
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => step(1, 1),
        KeyCode::Char('k') | KeyCode::Up => step(-1, 1),
        KeyCode::PageDown => step(1, PAGE),
        KeyCode::PageUp => step(-1, PAGE),
        KeyCode::Home => state.cursor = 0,
        KeyCode::End if n > 0 => {
            state.cursor = n - 1;
        }
        KeyCode::Char(' ') => state.toggle_cursor(),
        _ => {}
    }
}
