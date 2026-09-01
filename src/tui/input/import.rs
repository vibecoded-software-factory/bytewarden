use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::import::{cancel, commit, focus_step};
use crate::tui::import::ImportFocus;

pub fn mouse(app: &mut App, col: u16, row: u16) {
    let Some(idx) = crate::tui::view::widgets::field_hit_at(col, row) else {
        return;
    };
    if let Some(s) = app.import.as_mut() {
        s.focus = if idx == 0 {
            ImportFocus::Format
        } else {
            ImportFocus::Path
        };
    }
}

pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel(app),
        KeyCode::Enter => return commit(app),
        KeyCode::Tab | KeyCode::Down => return focus_step(app, 1),
        KeyCode::BackTab | KeyCode::Up => return focus_step(app, -1),
        _ => {}
    }

    let Some(state) = app.import.as_mut() else {
        return;
    };

    if state.focus == ImportFocus::Format {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => state.cycle_format(-1),
            KeyCode::Right | KeyCode::Char('l') => state.cycle_format(1),
            _ => {}
        }
        return;
    }

    crate::tui::input::common::route_line_editor(&mut state.path, key);
}
