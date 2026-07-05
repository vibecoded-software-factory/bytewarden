//! Vault-list management flows (load / silent reload / sync).
//!
//! Each loader has two flavours:
//!
//! * `request_load_*` — user-initiated; shows a spinner and settles to
//!   Idle (or Error) when the response arrives.
//! * `request_reload_*_silent` — chained after another action (delete,
//!   restore, sync, import, …). The response handler updates the cache
//!   without touching `action_state`, so the primary success toast
//!   (Deleted ✓, Synced ✓, …) survives instead of being clobbered.

use crate::domain::item::Item;
use crate::ports::BwError;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::worker::{InFlight, WorkerRequest};

/// Replaces the in-memory item list and rebuilds the search/filter
/// caches (via `sort_items`). Does **not** touch the cursor — for the
/// paths that set their own selection afterwards (create, restore).
pub(crate) fn set_items(app: &mut App, items: Vec<Item>) {
    app.items = items;
    app.sort_items();
}

/// Like [`set_items`] but **preserves the user's cursor on the same item
/// by id** across the reload — the invalidation contract for every
/// background / post-mutation refresh where the list changes underneath
/// the user (silent reload, sync, import, move, folder-delete, manual
/// F5). Capture the id first, replace, then re-anchor.
pub(crate) fn set_items_keep_cursor(app: &mut App, items: Vec<Item>) {
    let prev = app.selected_item_id();
    set_items(app, items);
    app.reanchor_selection(prev.as_deref());
}

/// Replaces the trash list (sorted) and rebuilds caches.
pub(crate) fn set_trash(app: &mut App, items: Vec<Item>) {
    let mut sorted = items;
    sorted.sort_by_cached_key(|i| i.name.to_lowercase());
    app.trashed_items = sorted;
    app.rebuild_caches();
}

// ── User-initiated loaders ────────────────────────────────────────────────

/// Loads the full vault list with a "Loading…" spinner.
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

/// Loads trashed items with a "Loading…" spinner — used when the user
/// picks the Trash filter.
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

// ── Silent reloads (preserve any prior Done toast) ────────────────────────

/// Queues a silent vault-list refresh. The caller has already set the
/// toast it wants the user to keep seeing.
pub fn request_reload_items_silent(app: &mut App) {
    // Silent — the caller's prior toast must survive, so claim the slot
    // through `begin` (no `Running` override).
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

// ── Sync ──────────────────────────────────────────────────────────────────

/// Queues a vault sync.
pub fn request_sync(app: &mut App) {
    app.submit(InFlight::Sync, "Syncing…", WorkerRequest::Sync);
}

pub fn handle_sync(app: &mut App, r: Result<(), BwError>) {
    match r {
        Ok(()) => {
            app.push_cmd("bw sync", true, "vault synced");
            app.set_action(ActionState::Done("Synced ✓".into()));
            // Refresh silently so the "Synced ✓" toast survives.
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
