//! Vault-import popup flow.

use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::import::{ImportFocus, ImportState};
use crate::tui::screens::Screen;

/// Opens the import popup with default values.
pub fn open(app: &mut App) {
    app.import = Some(ImportState::new());
    app.screen = Screen::Import;
}

/// Closes the popup, returns to the vault list.
pub fn cancel(app: &mut App) {
    app.import = None;
    app.screen = Screen::Vault;
}

/// Cycles focus between the format and path fields.
pub fn focus_step(app: &mut App, _dir: i32) {
    let Some(state) = app.import.as_mut() else {
        return;
    };
    state.focus = match state.focus {
        ImportFocus::Format => ImportFocus::Path,
        ImportFocus::Path => ImportFocus::Format,
    };
}

/// Runs `bw import` and reports the outcome. Closes the popup on
/// success; keeps it open on failure (typo in format / wrong path).
///
/// After a successful import we silently refresh both the items and
/// the folders lists so the new content shows up immediately.
pub fn commit(app: &mut App) {
    let Some(state) = app.import.as_ref() else {
        return;
    };
    let format = state.format.trim().to_string();
    let path = state.path.trim().to_string();
    if format.is_empty() {
        app.set_action(ActionState::Error("Format cannot be empty.".into()));
        return;
    }
    if path.is_empty() {
        app.set_action(ActionState::Error("Input path cannot be empty.".into()));
        return;
    }
    let pb = std::path::PathBuf::from(&path);
    if !pb.is_file() {
        app.set_action(ActionState::Error(format!("Import file not found: {path}")));
        return;
    }

    let cmd = format!("bw import {format} <path> --session ***");
    app.set_action(ActionState::Running("Importing…".into()));
    match app.vault.import(&format, &path) {
        Ok(()) => {
            app.push_cmd(&cmd, true, "import succeeded");
            // Fresh data — a successful import added new items / folders.
            super::vault::refresh_items_silent(app);
            super::folders::refresh_folders_silent(app);
            app.set_action(ActionState::Done("Import succeeded ✓".into()));
            app.import = None;
            app.screen = Screen::Vault;
        }
        Err(e) => app.cmd_err(&cmd, &e, "Import failed"),
    }
}
