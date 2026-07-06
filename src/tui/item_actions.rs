//! Per-item action menu model — the list of secondary actions offered
//! when a vault row is right-clicked (`Screen::ItemActions`).
//!
//! The enum + [`actions_for`] builder are pure (no I/O, no `App`) so the
//! applicable-action logic is unit-testable; dispatch lives in
//! [`crate::tui::flows::item_actions`] and reuses the existing per-action
//! flows, so each secret-exposing path keeps its reprompt gate.

use crate::domain::item::Item;

/// A secondary action that can be run against a single vault item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemAction {
    /// Open the item's detail view.
    Open,
    /// Copy the login username to the clipboard.
    CopyUsername,
    /// Copy the login password to the clipboard (reprompt-gated).
    CopyPassword,
    /// Enter edit mode on the item.
    Edit,
    /// Toggle the item's favorite flag.
    ToggleFavorite,
    /// Restore a trashed item.
    Restore,
    /// Delete the item (soft-delete, or permanent from the trash view).
    Delete,
}

impl ItemAction {
    /// The menu label shown for this action.
    pub fn label(self) -> &'static str {
        match self {
            ItemAction::Open => "Open",
            ItemAction::CopyUsername => "Copy username",
            ItemAction::CopyPassword => "Copy password",
            ItemAction::Edit => "Edit",
            ItemAction::ToggleFavorite => "Toggle favorite",
            ItemAction::Restore => "Restore",
            ItemAction::Delete => "Delete",
        }
    }
}

/// Live state for the open item-action menu.
pub struct ItemActionsState {
    /// The id of the item the menu targets — captured at open time so the
    /// menu stays anchored even if the list reorders underneath it.
    pub item_id: String,
    /// The applicable actions, in display order.
    pub actions: Vec<ItemAction>,
    /// The highlighted row.
    pub cursor: usize,
}

/// Builds the applicable actions for `item`, given whether the current
/// view is the trash. Trashed items can only be opened, restored or
/// permanently deleted; copy/edit/favorite don't apply there. Copy
/// actions appear only when the login actually carries that field.
pub fn actions_for(item: &Item, is_trash: bool) -> Vec<ItemAction> {
    if is_trash {
        return vec![ItemAction::Open, ItemAction::Restore, ItemAction::Delete];
    }
    let mut v = vec![ItemAction::Open];
    if let Some(login) = &item.login {
        if login.username.as_deref().is_some_and(|s| !s.is_empty()) {
            v.push(ItemAction::CopyUsername);
        }
        if login.password.as_deref().is_some_and(|s| !s.is_empty()) {
            v.push(ItemAction::CopyPassword);
        }
    }
    v.push(ItemAction::Edit);
    v.push(ItemAction::ToggleFavorite);
    v.push(ItemAction::Delete);
    v
}
