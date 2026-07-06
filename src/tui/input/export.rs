//! Key handler for the vault-export popup.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::export::ExportFocus;
use crate::tui::flows::export::{cancel, commit, cycle_format, focus_step};

/// Click: focus the field under the pointer.
pub fn mouse(app: &mut App, col: u16, row: u16) {
    let Some(idx) = crate::tui::view::widgets::field_hit_at(col, row) else {
        return;
    };
    if let Some(s) = app.export.as_mut() {
        s.focus = if idx == 0 {
            ExportFocus::Format
        } else {
            ExportFocus::Path
        };
    }
}

/// Dispatches a single key event on the export popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel(app),
        KeyCode::Enter => return commit(app),
        KeyCode::Tab | KeyCode::Down => return focus_step(app, 1),
        KeyCode::BackTab | KeyCode::Up => return focus_step(app, -1),
        _ => {}
    }

    let Some(state) = app.export.as_mut() else {
        return;
    };

    match state.focus {
        ExportFocus::Format => match key.code {
            KeyCode::Char(' ') | KeyCode::Right | KeyCode::Left => cycle_format(app),
            _ => {}
        },
        ExportFocus::Path => {
            crate::tui::input::common::route_line_editor(&mut state.path, key);
        }
    }
}
