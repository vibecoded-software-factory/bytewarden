//! Folder list management + CRUD flows.
//!
//! The folder list is fetched on-demand (typically right after the
//! vault list at boot) and again after every CRUD mutation so the
//! sidebar stays in sync without the user having to hit refresh.

use crate::domain::Folder;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::folders::{filter_for_row, row_count, row_for_filter};
use crate::tui::worker::{InFlight, WorkerRequest};

// ── Loading ───────────────────────────────────────────────────────────────

/// Queues a silent folder reload (worker). The caller has already set the
/// toast it wants the user to keep seeing.
pub fn request_reload_folders_silent(app: &mut App) {
    app.in_flight = Some(InFlight::FolderReload);
    let _ = app.worker_tx.send(WorkerRequest::ListFolders);
}

/// `bw list folders` response — applies the list silently (preserves the
/// prior toast). Sorts alphabetically and snaps the highlight back to the
/// row matching the active filter.
pub fn handle_reload(app: &mut App, r: Result<Vec<Folder>, String>) {
    match r {
        Ok(folders) => {
            let count = folders.len();
            app.folders = sorted(folders);
            app.folder_selected =
                row_for_filter(&app.active_folder, &app.folders, &app.collections);
            app.push_cmd("bw list folders", true, &format!("{count} folders loaded"));
        }
        Err(e) => app.cmd_err("bw list folders", &e, "Load folders failed"),
    }
}

fn sorted(mut folders: Vec<Folder>) -> Vec<Folder> {
    folders.sort_by_key(|f| f.name.to_lowercase());
    folders
}

// ── Sidebar navigation ────────────────────────────────────────────────────

/// Moves the folder-sidebar highlight down by one (clamped, skips the
/// separator row at logical index 2).
pub fn move_down(app: &mut App) {
    let n = row_count(&app.folders, &app.collections);
    if n == 0 {
        return;
    }
    if app.folder_selected + 1 < n {
        app.folder_selected += 1;
    }
}

/// Moves the highlight up by one (clamped at 0).
pub fn move_up(app: &mut App) {
    if app.folder_selected > 0 {
        app.folder_selected -= 1;
    }
}

/// Activates the highlighted folder filter and resets the item-list
/// selection / scroll so the user lands at the top of the new view.
pub fn apply_filter(app: &mut App) {
    app.active_folder = filter_for_row(app.folder_selected, &app.folders, &app.collections);
    app.selected_index = 0;
    app.scroll_offset = 0;
    app.rebuild_filtered_cache();
}

// ── Lookup helpers ────────────────────────────────────────────────────────

/// Returns the folder currently highlighted in the sidebar, or `None`
/// when the highlight is on a meta-row ("All folders" / "(No folder)").
pub fn focused_folder(app: &App) -> Option<&Folder> {
    if app.folder_selected < 2 {
        None
    } else {
        app.folders.get(app.folder_selected - 2)
    }
}

/// Resolves a folder name (case-insensitive) to its id, used by the
/// edit form's "Folder" field.
pub fn id_by_name(folders: &[Folder], name: &str) -> Option<String> {
    let lower = name.trim().to_lowercase();
    folders
        .iter()
        .find(|f| f.name.to_lowercase() == lower)
        .map(|f| f.id.clone())
}

/// Resolves a folder id to its name, used to populate the "Folder"
/// field in the edit form.
pub fn name_by_id(folders: &[Folder], id: &str) -> Option<String> {
    folders.iter().find(|f| f.id == id).map(|f| f.name.clone())
}

// ── Name-prompt popup state (Create / Rename) ─────────────────────────────

/// What the in-flight folder-name popup will do on commit.
#[derive(Debug, Clone)]
pub enum FolderNamePurpose {
    /// Create a brand-new folder with the typed name.
    Create,
    /// Rename the folder with this id to the typed name.
    Rename { folder_id: String },
}

/// Buffer for the in-flight folder-name popup.
#[derive(Debug, Clone)]
pub struct FolderNameState {
    /// Text being typed.
    pub input: String,
    /// Cursor position (character index).
    pub cursor: usize,
    /// What to do when the user hits Enter.
    pub purpose: FolderNamePurpose,
}

impl FolderNameState {
    fn fresh(purpose: FolderNamePurpose, prefill: &str) -> Self {
        Self {
            input: prefill.to_string(),
            cursor: prefill.chars().count(),
            purpose,
        }
    }
}

/// Opens the popup in Create mode (empty input).
pub fn open_create(app: &mut App) {
    app.folder_name = Some(FolderNameState::fresh(FolderNamePurpose::Create, ""));
    app.screen = crate::tui::screens::Screen::FolderName;
}

/// Opens the popup in Rename mode for the currently focused folder.
/// No-op + error toast when the highlight is on a meta-row.
pub fn open_rename(app: &mut App) {
    let Some(folder) = focused_folder(app) else {
        app.set_action(ActionState::Error(
            "Pick a folder to rename (not 'All folders' or '(No folder)').".into(),
        ));
        return;
    };
    let folder_id = folder.id.clone();
    let prefill = folder.name.clone();
    app.folder_name = Some(FolderNameState::fresh(
        FolderNamePurpose::Rename { folder_id },
        &prefill,
    ));
    app.screen = crate::tui::screens::Screen::FolderName;
}

/// Cancels the popup and returns to the vault screen.
pub fn cancel_name_popup(app: &mut App) {
    app.folder_name = None;
    app.screen = crate::tui::screens::Screen::Vault;
}

/// Validates and dispatches the folder create / rename to the worker.
/// The popup state (`folder_name`) is kept so the response handler can
/// restore it on error; it's cleared on success.
pub fn commit_name_popup(app: &mut App) {
    let Some(state) = app.folder_name.as_ref() else {
        return;
    };
    let name = state.input.trim().to_string();
    if name.is_empty() {
        app.set_action(ActionState::Error("Folder name cannot be empty.".into()));
        return;
    }
    // Pre-flight unique check against the in-memory folder list. For a
    // rename, the folder being edited keeps its old name as the exempt
    // "current" so submitting the same value back is a no-op.
    let existing: Vec<&str> = app.folders.iter().map(|f| f.name.as_str()).collect();
    let current = match &state.purpose {
        FolderNamePurpose::Rename { folder_id } => app
            .folders
            .iter()
            .find(|f| &f.id == folder_id)
            .map(|f| f.name.as_str()),
        FolderNamePurpose::Create => None,
    };
    if let Err(msg) = crate::domain::validation::check_name_unique(&name, &existing, current) {
        app.set_action(ActionState::Error(msg));
        return;
    }

    match state.purpose.clone() {
        FolderNamePurpose::Create => {
            app.set_action(ActionState::Running("Creating folder…".into()));
            app.in_flight = Some(InFlight::CreateFolder);
            let _ = app.worker_tx.send(WorkerRequest::CreateFolder { name });
        }
        FolderNamePurpose::Rename { folder_id } => {
            app.set_action(ActionState::Running("Renaming folder…".into()));
            app.in_flight = Some(InFlight::EditFolder);
            let _ = app
                .worker_tx
                .send(WorkerRequest::EditFolder { folder_id, name });
        }
    }
}

/// `bw create folder` response.
pub fn handle_create(app: &mut App, r: Result<Folder, String>) {
    match r {
        Ok(folder) => {
            app.push_cmd(
                "bw create folder",
                true,
                &format!("created: {}", folder.name),
            );
            app.set_action(ActionState::Done(format!(
                "Folder \"{}\" created ✓",
                folder.name
            )));
            app.folder_name = None;
            app.screen = crate::tui::screens::Screen::Vault;
            request_reload_folders_silent(app);
        }
        // Keep the popup open (state untouched) so the user can fix it.
        Err(e) => app.cmd_err("bw create folder", &e, "Create folder failed"),
    }
}

/// `bw edit folder` response.
pub fn handle_edit(app: &mut App, r: Result<Folder, String>) {
    match r {
        Ok(folder) => {
            app.push_cmd(
                "bw edit folder",
                true,
                &format!("renamed to: {}", folder.name),
            );
            app.set_action(ActionState::Done(format!(
                "Renamed to \"{}\" ✓",
                folder.name
            )));
            app.folder_name = None;
            app.screen = crate::tui::screens::Screen::Vault;
            request_reload_folders_silent(app);
        }
        Err(e) => app.cmd_err("bw edit folder", &e, "Rename folder failed"),
    }
}

// ── Delete confirm popup ──────────────────────────────────────────────────

/// Opens the confirm-delete popup for the focused folder. No-op on
/// meta-rows.
pub fn open_confirm_delete(app: &mut App) {
    if focused_folder(app).is_none() {
        app.set_action(ActionState::Error(
            "Pick a folder to delete (not a meta-row).".into(),
        ));
        return;
    }
    app.screen = crate::tui::screens::Screen::ConfirmDeleteFolder;
}

/// Returns to the vault list, leaving the folder intact.
pub fn cancel_delete(app: &mut App) {
    app.screen = crate::tui::screens::Screen::Vault;
}

/// Dispatches the folder deletion to the worker. If the active filter
/// points at the folder being deleted, it's reset to "All" up front so
/// the list isn't confusingly empty.
pub fn confirm_delete(app: &mut App) {
    let Some(folder) = focused_folder(app) else {
        app.screen = crate::tui::screens::Screen::Vault;
        return;
    };
    let id = folder.id.clone();
    let name = folder.name.clone();
    if matches!(&app.active_folder, super::super::folders::FolderFilter::Folder(fid) if fid == &id)
    {
        app.active_folder = super::super::folders::FolderFilter::All;
        app.folder_selected = 0;
        app.rebuild_filtered_cache();
    }
    app.set_action(ActionState::Running("Deleting folder…".into()));
    app.screen = crate::tui::screens::Screen::Vault;
    app.in_flight = Some(InFlight::DeleteFolder { name });
    let _ = app
        .worker_tx
        .send(WorkerRequest::DeleteFolder { folder_id: id });
}

/// `bw delete folder` response. Items previously in the folder aren't
/// deleted — bw clears their `folder_id`, so both lists are reloaded.
pub fn handle_delete(app: &mut App, name: String, r: Result<(), String>) {
    match r {
        Ok(()) => {
            app.push_cmd("bw delete folder", true, &format!("deleted: {name}"));
            app.set_action(ActionState::Done(format!("Folder \"{name}\" deleted ✓")));
            // Items' folder_id pointers changed → reload items, then
            // folders. Both silent so the toast survives.
            app.in_flight = Some(InFlight::FolderDeleteReloadItems);
            let _ = app.worker_tx.send(WorkerRequest::ListItems);
        }
        Err(e) => app.cmd_err("bw delete folder", &e, "Delete folder failed"),
    }
}

/// Silent post-folder-delete item reload → chains the folder reload.
pub fn handle_delete_reload_items(app: &mut App, r: Result<Vec<crate::domain::Item>, String>) {
    match r {
        Ok(items) => super::vault::set_items(app, items),
        Err(e) => app.push_cmd("bw list items", false, &e),
    }
    request_reload_folders_silent(app);
}
