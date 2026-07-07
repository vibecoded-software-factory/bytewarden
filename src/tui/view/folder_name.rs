//! Folder-name input popup (used for both Create and Rename).

use ratatui::{Frame, layout::Rect};

use crate::tui::app::App;
use crate::tui::flows::folders::FolderNamePurpose;
use crate::tui::view::widgets::{InputFooter, InputPopup, draw_input_popup};

/// Renders the folder-name popup. No-op when no popup is in flight.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.folder_name else {
        return;
    };
    let (title, note) = match state.purpose {
        FolderNamePurpose::Create => (
            " New folder ",
            "A new folder will be created at the top level.",
        ),
        FolderNamePurpose::Rename { .. } => {
            (" Rename folder ", "Items inside the folder are unaffected.")
        }
    };
    draw_input_popup(
        frame,
        area,
        &app.theme,
        InputPopup {
            title,
            width_pct: 60,
            context: vec![],
            label: "Folder name",
            label_hint: "Enter to apply, Esc to cancel",
            editor: &state.input,
            placeholder: "type a folder name",
            footer: InputFooter::Note(note),
        },
    );
}
