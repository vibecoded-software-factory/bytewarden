use crate::domain::item::Item;
use crate::ports::BwError;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::worker::{InFlight, WorkerRequest};

pub(crate) fn set_items(app: &mut App, items: Vec<Item>) {
    app.vault.items = items;
    app.vault.sort_items();
}

pub(crate) fn set_items_keep_cursor(app: &mut App, items: Vec<Item>) {
    let prev = app.vault.selected_item_id();
    set_items(app, items);
    app.vault.reanchor_selection(prev.as_deref());
}

pub(crate) fn set_trash(app: &mut App, items: Vec<Item>) {
    let mut sorted = items;
    sorted.sort_by_cached_key(|i| i.name.to_lowercase());
    app.vault.trashed_items = sorted;
    app.vault.rebuild_caches();
}

pub fn request_load_items(app: &mut App) {
    app.submit(
        InFlight::LoadItems,
        "Loading vault…",
        WorkerRequest::ListItems,
    );
}

pub fn handle_load_items(app: &mut App, r: Result<Vec<Item>, BwError>) {
    match r {
        Ok(items) => {
            let n = items.len();
            set_items_keep_cursor(app, items);
            app.push_cmd("bw list items", true, &format!("{n} items loaded"));
            app.set_action(ActionState::Idle);
        }
        Err(e) => app.cmd_err("bw list items", &e, "Load failed"),
    }
}

pub fn request_load_trash(app: &mut App) {
    app.submit(
        InFlight::LoadTrash,
        "Loading trash…",
        WorkerRequest::ListTrash,
    );
}

pub fn handle_load_trash(app: &mut App, r: Result<Vec<Item>, BwError>) {
    match r {
        Ok(items) => {
            let n = items.len();
            set_trash(app, items);
            app.push_cmd(
                "bw list items --trash",
                true,
                &format!("{n} trashed items loaded"),
            );
            app.set_action(ActionState::Idle);
        }
        Err(e) => app.cmd_err("bw list items --trash", &e, "Load trash failed"),
    }
}

pub fn request_reload_items_silent(app: &mut App) {
    if app.begin(InFlight::ReloadItemsSilent) {
        let _ = app.worker_tx.send(WorkerRequest::ListItems);
    }
}

pub fn handle_reload_items_silent(app: &mut App, r: Result<Vec<Item>, BwError>) {
    match r {
        Ok(items) => {
            let n = items.len();
            set_items_keep_cursor(app, items);
            app.push_cmd("bw list items", true, &format!("{n} items loaded"));
        }
        Err(e) => app.cmd_err("bw list items", &e, "Load failed"),
    }
}

pub fn request_sync(app: &mut App) {
    app.submit(InFlight::Sync, "Syncing…", WorkerRequest::Sync);
}

pub fn handle_sync(app: &mut App, r: Result<(), BwError>) {
    match r {
        Ok(()) => {
            app.push_cmd("bw sync", true, "vault synced");
            app.set_action(ActionState::Done("Synced ✓".into()));

            if app.begin(InFlight::SyncReload) {
                let _ = app.worker_tx.send(WorkerRequest::ListItems);
            }
        }
        Err(e) => app.cmd_err("bw sync", &e, "Sync failed"),
    }
}

pub fn handle_sync_reload(app: &mut App, r: Result<Vec<Item>, BwError>) {
    match r {
        Ok(items) => {
            let n = items.len();
            set_items_keep_cursor(app, items);
            app.push_cmd("bw list items", true, &format!("{n} items loaded"));
        }
        Err(e) => app.cmd_err("bw list items", &e, "Load failed"),
    }
}
