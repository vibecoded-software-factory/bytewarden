//! Vault-import popup flow.

use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::import::{ImportFocus, ImportState};
use crate::tui::screens::Screen;
use crate::tui::worker::{InFlight, WorkerRequest};

/// Opens the import popup, populating the format dropdown from the
/// cached `bw import --formats` list (loaded once at login).
pub fn open(app: &mut App) {
    app.import = Some(ImportState::new(&app.import_formats));
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
    let format = state.current_format().to_string();
    let path = state.path.trim().to_string();
    if path.is_empty() {
        app.set_action(ActionState::Error("Input path cannot be empty.".into()));
        return;
    }
    let pb = std::path::PathBuf::from(&path);
    if !pb.is_file() {
        app.set_action(ActionState::Error(format!("Import file not found: {path}")));
        return;
    }

    app.set_action(ActionState::Running("Importing…".into()));
    app.in_flight = Some(InFlight::Import);
    let _ = app.worker_tx.send(WorkerRequest::Import { format, path });
}

/// `bw import` response. On success, closes the popup and silently
/// reloads items then folders so the new content shows up.
pub fn handle(app: &mut App, r: Result<(), String>) {
    let cmd = "bw import".to_string();
    match r {
        Ok(()) => {
            app.push_cmd(&cmd, true, "import succeeded");
            app.set_action(ActionState::Done("Import succeeded ✓".into()));
            app.import = None;
            app.screen = Screen::Vault;
            // Fresh data — reload items then folders (both silent).
            app.in_flight = Some(InFlight::ImportReloadItems);
            let _ = app.worker_tx.send(WorkerRequest::ListItems);
        }
        Err(e) => app.cmd_err(&cmd, &e, "Import failed"),
    }
}

/// Silent post-import item reload → chains the folder reload.
pub fn handle_reload_items(app: &mut App, r: Result<Vec<crate::domain::Item>, String>) {
    match r {
        Ok(items) => super::vault::set_items(app, items),
        Err(e) => app.push_cmd("bw list items", false, &e),
    }
    app.in_flight = Some(InFlight::ImportReloadFolders);
    let _ = app.worker_tx.send(WorkerRequest::ListFolders);
}

/// Silent post-import folder reload.
pub fn handle_reload_folders(app: &mut App, r: Result<Vec<crate::domain::Folder>, String>) {
    super::folders::handle_reload(app, r);
}
