//! Key handler for the folder-name input popup (Create / Rename).

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::folders::{cancel_name_popup, commit_name_popup};

/// Dispatches a single key event on the folder-name popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel_name_popup(app),
        KeyCode::Enter => return commit_name_popup(app),
        _ => {}
    }

    let Some(state) = app.folder_name.as_mut() else {
        return;
    };
    crate::tui::input::common::route_line_editor(&mut state.input, key);
}
