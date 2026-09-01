use crate::ports::BwError;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::import::{ImportFocus, ImportState};
use crate::tui::screens::Screen;
use crate::tui::worker::{InFlight, WorkerRequest};

pub fn open(app: &mut App) {
    app.import = Some(ImportState::new(&app.import_formats));
    app.screen = Screen::Import;
}

pub fn cancel(app: &mut App) {
    app.import = None;
    app.screen = Screen::Vault;
}

pub fn focus_step(app: &mut App, _dir: i32) {
    let Some(state) = app.import.as_mut() else {
        return;
    };
    state.focus = match state.focus {
        ImportFocus::Format => ImportFocus::Path,
        ImportFocus::Path => ImportFocus::Format,
    };
}

pub fn commit(app: &mut App) {
    let Some(state) = app.import.as_ref() else {
        return;
    };
    let format = state.current_format().to_string();
    let path = state.path.text().trim().to_string();
    if path.is_empty() {
        app.set_action(ActionState::Error("Input path cannot be empty.".into()));
        return;
    }
    let pb = std::path::PathBuf::from(&path);
    if !pb.is_file() {
        app.set_action(ActionState::Error(format!("Import file not found: {path}")));
        return;
    }

    app.submit(
        InFlight::Import,
        "Importing…",
        WorkerRequest::Import { format, path },
    );
}

pub fn handle(app: &mut App, r: Result<(), BwError>) {
    let cmd = "bw import".to_string();
    match r {
        Ok(()) => {
            app.push_cmd(&cmd, true, "import succeeded");
            app.set_action(ActionState::Done("Import succeeded ✓".into()));
            app.import = None;
            app.screen = Screen::Vault;

            if app.begin(InFlight::ImportReloadItems) {
                let _ = app.worker_tx.send(WorkerRequest::ListItems);
            }
        }
        Err(e) => app.cmd_err(&cmd, &e, "Import failed"),
    }
}

pub fn handle_reload_items(app: &mut App, r: Result<Vec<crate::domain::Item>, BwError>) {
    match r {
        Ok(items) => super::vault::set_items_keep_cursor(app, items),
        Err(e) => app.push_cmd("bw list items", false, &e),
    }
    if app.begin(InFlight::ImportReloadFolders) {
        let _ = app.worker_tx.send(WorkerRequest::ListFolders);
    }
}

pub fn handle_reload_folders(app: &mut App, r: Result<Vec<crate::domain::Folder>, BwError>) {
    super::folders::handle_reload(app, r);
}
