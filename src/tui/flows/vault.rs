//! Vault-list management flows.
//!
//! Each loader has two flavours:
//!
//! * `load_*` — user-initiated; shows a "Loading…" spinner and
//!   transitions to Idle (or Error) on completion.
//! * `refresh_*_silent` — used as a side-effect of another action
//!   (delete, restore, sync). Updates the in-memory cache without
//!   touching `action_state`, so the primary success toast (Deleted ✓,
//!   Synced ✓, …) is preserved instead of being clobbered by a
//!   follow-up "Loading…" frame.

use crate::tui::action::{ActionState, PendingAction};
use crate::tui::app::App;

// ── User-initiated loaders ────────────────────────────────────────────────

/// Loads the full vault list with a "Loading…" spinner.
pub fn load_items(app: &mut App) {
    app.set_action(ActionState::Running("Loading vault…".into()));
    if refresh_items_silent(app) {
        app.set_action(ActionState::Idle);
    }
}

/// Loads trashed items with a "Loading…" spinner — used when the user
/// picks the Trash filter.
pub fn load_trash(app: &mut App) {
    app.set_action(ActionState::Running("Loading trash…".into()));
    if refresh_trash_silent(app) {
        app.set_action(ActionState::Idle);
    }
}

// ── Silent refreshes (preserve any prior Done toast) ──────────────────────

/// Refreshes `app.items` from the backend without touching
/// `action_state`. Returns `true` on success.
///
/// Caller is expected to have set whichever toast they want the user to
/// see. On *failure* the function does set an Error state, since
/// silently swallowing an error would leave the cache stale without
/// notifying the user.
pub fn refresh_items_silent(app: &mut App) -> bool {
    let cmd = format!("bw list items --session {}", app.session_key_display());
    match app.vault.list_items() {
        Ok(items) => {
            let count = items.len();
            app.items = items;
            // sort_items rebuilds the search/filter caches at the end,
            // so we do not need a separate rebuild_caches call here.
            app.sort_items();
            app.push_cmd(&cmd, true, &format!("{count} items loaded"));
            true
        }
        Err(e) => {
            app.cmd_err(&cmd, &e, "Load failed");
            false
        }
    }
}

/// Refreshes `app.trashed_items` without touching `action_state`.
/// Returns `true` on success.
pub fn refresh_trash_silent(app: &mut App) -> bool {
    let cmd = format!(
        "bw list items --trash --session {}",
        app.session_key_display()
    );
    match app.vault.list_trash() {
        Ok(items) => {
            let count = items.len();
            let mut sorted = items;
            sorted.sort_by_cached_key(|i| i.name.to_lowercase());
            app.trashed_items = sorted;
            app.rebuild_caches();
            app.push_cmd(&cmd, true, &format!("{count} trashed items loaded"));
            true
        }
        Err(e) => {
            app.cmd_err(&cmd, &e, "Load trash failed");
            false
        }
    }
}

// ── Sync ──────────────────────────────────────────────────────────────────

/// Queues a vault sync.
pub fn sync_vault(app: &mut App) {
    app.set_action(ActionState::Running("Syncing…".into()));
    app.pending_action = PendingAction::SyncVault;
}

/// Pending-action executor for [`PendingAction::SyncVault`].
pub fn do_sync_vault(app: &mut App) {
    let cmd = format!("bw sync --session {}", app.session_key_display());
    match app.vault.sync() {
        Ok(()) => {
            app.push_cmd(&cmd, true, "vault synced");
            // Refresh silently so the "Synced ✓" toast survives.
            refresh_items_silent(app);
            app.set_action(ActionState::Done("Synced ✓".into()));
        }
        Err(e) => app.cmd_err(&cmd, &e, "Sync failed"),
    }
}
