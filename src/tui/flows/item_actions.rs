//! Dispatch for the per-item action menu (`Screen::ItemActions`).
//!
//! The menu is opened by right-clicking a vault row (the mouse layer
//! seats the list cursor there first, so every action operates on the
//! right item through the ordinary `vault.selected_item()` path). Each
//! arm delegates to the existing per-action flow, so the secret-exposing
//! actions (copy password) keep the reprompt gate they already carry —
//! the mouse can't bypass the master-password re-check.

use crate::tui::app::App;
use crate::tui::flows::{assign_collections, copy, items};
use crate::tui::item_actions::{ItemAction, ItemActionsState, actions_for};
use crate::tui::screens::Screen;

/// Opens the action menu for the currently-selected vault item. No-op
/// when the list is empty (nothing to act on).
pub fn open(app: &mut App) {
    let Some(item) = app.vault.selected_item() else {
        return;
    };
    let is_trash = app.vault.is_trash_view();
    let can_move = assign_collections::can_move_selected(app);
    let actions = actions_for(item, is_trash, can_move);
    if actions.is_empty() {
        return;
    }
    let item_id = item.id.clone();
    app.item_actions = Some(ItemActionsState {
        item_id,
        actions,
        cursor: 0,
    });
    app.screen = Screen::ItemActions;
}

/// Closes the menu back to the vault, discarding its state.
pub fn close(app: &mut App) {
    app.item_actions = None;
    app.screen = Screen::Vault;
}

/// Moves the menu cursor by `delta`, clamped to the action list.
pub fn move_cursor(app: &mut App, delta: isize) {
    if let Some(state) = app.item_actions.as_mut() {
        let n = state.actions.len();
        if n == 0 {
            return;
        }
        let cur = state.cursor as isize + delta;
        state.cursor = cur.clamp(0, n as isize - 1) as usize;
    }
}

/// Runs the highlighted action. Closes the menu first so any screen the
/// action opens (detail, the confirm popup) — and any reprompt popup it
/// triggers — is anchored on the vault, not on the transient menu.
pub fn run_selected(app: &mut App) {
    let Some(state) = app.item_actions.as_ref() else {
        return;
    };
    let Some(&action) = state.actions.get(state.cursor) else {
        return close(app);
    };
    run(app, action);
}

/// Runs a specific action (the click path selects by index, then calls
/// here). Closes the menu first, then delegates to the owning flow.
pub fn run(app: &mut App, action: ItemAction) {
    close(app);
    match action {
        ItemAction::Open => app.go_to_detail(),
        ItemAction::CopyUsername => copy::copy_username_to_clipboard(app),
        ItemAction::CopyPassword => copy::copy_password_to_clipboard(app),
        ItemAction::CopyTotp => copy::copy_totp_to_clipboard(app),
        ItemAction::Edit => {
            app.go_to_detail();
            items::enter_edit_mode(app);
        }
        ItemAction::Move => assign_collections::open_for_move_from(app, Screen::Vault),
        ItemAction::ToggleFavorite => items::toggle_favorite(app),
        ItemAction::Restore => items::queue_restore_item(app),
        ItemAction::Delete => items::open_confirm_delete(app),
    }
}
