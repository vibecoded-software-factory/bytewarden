//! Folder list management + CRUD flows.
//!
//! The folder list is fetched on-demand (typically right after the
//! vault list at boot) and again after every CRUD mutation so the
//! sidebar stays in sync without the user having to hit refresh.

use crate::domain::Folder;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::folders::{filter_for_row, row_count, row_for_filter};

// ── Loading ───────────────────────────────────────────────────────────────

/// Loads the folder list with a "Loading…" spinner — used for an
/// explicit user refresh.
pub fn load_folders(app: &mut App) {
    app.set_action(ActionState::Running("Loading folders…".into()));
    if refresh_folders_silent(app) {
        app.set_action(ActionState::Idle);
    }
}

/// Refreshes `app.folders` from the backend without touching
/// `action_state`. Returns `true` on success.
///
/// Sorts the result alphabetically (case-insensitive) so the sidebar
/// has a stable, predictable order. Snaps the highlight back to the
/// row matching the previously-active filter.
pub fn refresh_folders_silent(app: &mut App) -> bool {
    let cmd = "bw list folders".to_string();
    match app.vault.list_folders() {
        Ok(folders) => {
            let count = folders.len();
            app.folders = sorted(folders);
            app.folder_selected =
                row_for_filter(&app.active_folder, &app.folders, &app.collections);
            app.push_cmd(&cmd, true, &format!("{count} folders loaded"));
            true
        }
        Err(e) => {
            app.cmd_err(&cmd, &e, "Load folders failed");
            false
        }
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

/// Commits the popup's input via `bw create folder` or `bw edit
/// folder` depending on the purpose. Validates non-empty after trim;
/// on backend error rolls the popup state back so the user can retry.
pub fn commit_name_popup(app: &mut App) {
    let Some(state) = app.folder_name.take() else {
        return;
    };
    let name = state.input.trim().to_string();
    if name.is_empty() {
        app.folder_name = Some(state);
        app.set_action(ActionState::Error("Folder name cannot be empty.".into()));
        return;
    }
    // Pre-flight unique check against the in-memory folder list.
    // For a rename, the folder being edited keeps its old name as the
    // exempt "current" so submitting the same value back is a no-op
    // rather than a duplicate-error.
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
        app.folder_name = Some(state);
        app.set_action(ActionState::Error(msg));
        return;
    }

    match &state.purpose {
        FolderNamePurpose::Create => {
            let cmd = "bw create folder".to_string();
            match app.vault.create_folder(&name) {
                Ok(folder) => {
                    app.push_cmd(&cmd, true, &format!("created: {}", folder.name));
                    app.set_action(ActionState::Done(format!(
                        "Folder \"{}\" created ✓",
                        folder.name
                    )));
                    refresh_folders_silent(app);
                    // Highlight the newly created folder so the next
                    // Enter applies its filter.
                    if let Some(idx) = app.folders.iter().position(|f| f.id == folder.id) {
                        app.folder_selected = idx + 2;
                    }
                    app.screen = crate::tui::screens::Screen::Vault;
                }
                Err(e) => {
                    app.cmd_err(&cmd, &e, "Create folder failed");
                    // Restore popup with the typed text so the user can fix.
                    app.folder_name = Some(state);
                }
            }
        }
        FolderNamePurpose::Rename { folder_id } => {
            let cmd = format!("bw edit folder {folder_id}");
            let id = folder_id.clone();
            match app.vault.edit_folder(&id, &name) {
                Ok(folder) => {
                    app.push_cmd(&cmd, true, &format!("renamed to: {}", folder.name));
                    app.set_action(ActionState::Done(format!(
                        "Renamed to \"{}\" ✓",
                        folder.name
                    )));
                    refresh_folders_silent(app);
                    app.screen = crate::tui::screens::Screen::Vault;
                }
                Err(e) => {
                    app.cmd_err(&cmd, &e, "Rename folder failed");
                    app.folder_name = Some(state);
                }
            }
        }
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

/// Performs the deletion and refreshes the sidebar. Items previously
/// assigned to the folder are not deleted — bw clears their
/// `folder_id` to null, which would show under "(No folder)" after
/// the next item-list refresh.
pub fn confirm_delete(app: &mut App) {
    let Some(folder) = focused_folder(app) else {
        app.screen = crate::tui::screens::Screen::Vault;
        return;
    };
    let id = folder.id.clone();
    let name = folder.name.clone();
    let cmd = format!("bw delete folder {id}");
    match app.vault.delete_folder(&id) {
        Ok(()) => {
            app.push_cmd(&cmd, true, &format!("deleted: {name}"));
            app.set_action(ActionState::Done(format!("Folder \"{name}\" deleted ✓")));
            // If the active filter was pointing at the now-gone folder,
            // fall back to "All folders" so the list isn't empty for a
            // confusing reason.
            if matches!(&app.active_folder, super::super::folders::FolderFilter::Folder(fid) if fid == &id)
            {
                app.active_folder = super::super::folders::FolderFilter::All;
                app.folder_selected = 0;
                app.rebuild_filtered_cache();
            }
            refresh_folders_silent(app);
            // Refresh items too — their folder_id pointers have changed.
            super::vault::refresh_items_silent(app);
        }
        Err(e) => app.cmd_err(&cmd, &e, "Delete folder failed"),
    }
    app.screen = crate::tui::screens::Screen::Vault;
}
