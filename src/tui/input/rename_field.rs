//! Key handler for the rename-custom-field popup.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::items::{cancel_rename_field, commit_rename_field};

/// Dispatches a single key event on the rename popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel_rename_field(app),
        KeyCode::Enter => return commit_rename_field(app),
        _ => {}
    }

    let Some(state) = app.rename_field.as_mut() else {
        return;
    };
    crate::tui::input::common::route_line_editor(&mut state.input, key);
}
