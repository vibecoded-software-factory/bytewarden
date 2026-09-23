use crate::domain::Folder;
use crate::domain::LineEditor;
use crate::ports::BwError;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::folders::{filter_for_row, row_count, row_for_filter};
use crate::tui::input::nav::nav_clamp;
use crate::tui::worker::{InFlight, WorkerRequest};

pub fn request_reload_folders_silent(app: &mut App) {
    if app.begin(InFlight::FolderReload) {
        let _ = app.worker_tx.send(WorkerRequest::ListFolders);
    }
}

pub fn handle_reload(app: &mut App, r: Result<Vec<Folder>, BwError>) {
    match r {
        Ok(folders) => {
            let count = folders.len();
            app.folders = sorted(folders);
            app.vault.folder_selected =
                row_for_filter(&app.vault.active_folder, &app.folders, &app.collections);
            app.push_cmd("bw list folders", true, &format!("{count} folders loaded"));
        }
        Err(e) => app.cmd_err("bw list folders", &e, "Load folders failed"),
    }
}

fn sorted(mut folders: Vec<Folder>) -> Vec<Folder> {
    folders.sort_by_key(|f| f.name.to_lowercase());
    folders
}

pub fn move_down(app: &mut App) {
    move_by(app, 1);
}

pub fn move_up(app: &mut App) {
    move_by(app, -1);
}

fn move_by(app: &mut App, delta: i8) {
    let n = row_count(&app.folders, &app.collections);
    nav_clamp(&mut app.vault.folder_selected, n, delta);
}

pub fn apply_filter(app: &mut App) {
    app.vault.active_folder =
        filter_for_row(app.vault.folder_selected, &app.folders, &app.collections);
    app.vault.selected_index = 0;
    app.vault.scroll_offset = 0;
    app.vault.rebuild_filtered_cache();
}

pub fn focused_folder(app: &App) -> Option<&Folder> {
    if app.vault.folder_selected < 2 {
        None
    } else {
        app.folders.get(app.vault.folder_selected - 2)
    }
}

pub fn id_by_name(folders: &[Folder], name: &str) -> Option<String> {
    let lower = name.trim().to_lowercase();
    folders
        .iter()
        .find(|f| f.name.to_lowercase() == lower)
        .map(|f| f.id.clone())
}

pub fn name_by_id(folders: &[Folder], id: &str) -> Option<String> {
    folders.iter().find(|f| f.id == id).map(|f| f.name.clone())
}

#[derive(Debug, Clone)]
pub enum FolderNamePurpose {
    Create,

    Rename { folder_id: String },
}

#[derive(Debug, Clone)]
pub struct FolderNameState {
    pub input: LineEditor,

    pub purpose: FolderNamePurpose,
}

impl FolderNameState {
    fn fresh(purpose: FolderNamePurpose, prefill: &str) -> Self {
        Self {
            input: LineEditor::with_text(prefill),
            purpose,
        }
    }
}

pub fn open_create(app: &mut App) {
    app.folder_name = Some(FolderNameState::fresh(FolderNamePurpose::Create, ""));
    app.screen = crate::tui::screens::Screen::FolderName;
}

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

pub fn cancel_name_popup(app: &mut App) {
    app.folder_name = None;
    app.screen = crate::tui::screens::Screen::Vault;
}

pub fn commit_name_popup(app: &mut App) {
    let Some(state) = app.folder_name.as_ref() else {
        return;
    };
    let name = state.input.text().trim().to_string();
    if name.is_empty() {
        app.set_action(ActionState::Error("Folder name cannot be empty.".into()));
        return;
    }

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
            app.submit(
                InFlight::CreateFolder,
                "Creating folder…",
                WorkerRequest::CreateFolder { name },
            );
        }
        FolderNamePurpose::Rename { folder_id } => {
            app.submit(
                InFlight::EditFolder,
                "Renaming folder…",
                WorkerRequest::EditFolder { folder_id, name },
            );
        }
    }
}

pub fn handle_create(app: &mut App, r: Result<Folder, BwError>) {
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

        Err(e) => app.cmd_err("bw create folder", &e, "Create folder failed"),
    }
}

pub fn handle_edit(app: &mut App, r: Result<Folder, BwError>) {
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

pub fn open_confirm_delete(app: &mut App) {
    if focused_folder(app).is_none() {
        app.set_action(ActionState::Error(
            "Pick a folder to delete (not a meta-row).".into(),
        ));
        return;
    }
    app.screen = crate::tui::screens::Screen::ConfirmDeleteFolder;
}

pub fn cancel_delete(app: &mut App) {
    app.screen = crate::tui::screens::Screen::Vault;
}

pub fn confirm_delete(app: &mut App) {
    let Some(folder) = focused_folder(app) else {
        app.screen = crate::tui::screens::Screen::Vault;
        return;
    };
    let id = folder.id.clone();
    let name = folder.name.clone();
    if matches!(&app.vault.active_folder, super::super::folders::FolderFilter::Folder(fid) if fid == &id)
    {
        app.vault.active_folder = super::super::folders::FolderFilter::All;
        app.vault.folder_selected = 0;
        app.vault.rebuild_filtered_cache();
    }
    app.screen = crate::tui::screens::Screen::Vault;
    app.submit(
        InFlight::DeleteFolder { name },
        "Deleting folder…",
        WorkerRequest::DeleteFolder { folder_id: id },
    );
}

pub fn handle_delete(app: &mut App, name: String, r: Result<(), BwError>) {
    match r {
        Ok(()) => {
            app.push_cmd("bw delete folder", true, &format!("deleted: {name}"));
            app.set_action(ActionState::Done(format!("Folder \"{name}\" deleted ✓")));

            if app.begin(InFlight::FolderDeleteReloadItems) {
                let _ = app.worker_tx.send(WorkerRequest::ListItems);
            }
        }
        Err(e) => app.cmd_err("bw delete folder", &e, "Delete folder failed"),
    }
}

pub fn handle_delete_reload_items(app: &mut App, r: Result<Vec<crate::domain::Item>, BwError>) {
    match r {
        Ok(items) => super::vault::set_items_keep_cursor(app, items),
        Err(e) => app.push_cmd("bw list items", false, &e),
    }
    request_reload_folders_silent(app);
}
