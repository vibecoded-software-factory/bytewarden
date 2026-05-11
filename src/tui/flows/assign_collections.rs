//! Flow for the multi-select "assign collections" popup.
//!
//! Open path filters the global collection list to the org of the
//! item currently in the edit form, snapshots the user's existing
//! membership and parks the cursor on row 0. Commit validates the
//! "≥1 selection" rule that bw enforces for org items and copies the
//! chosen UUIDs back into the matching `EditField`.

use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::assign_collections::{AssignCollectionsPurpose, AssignCollectionsState};
use crate::tui::screens::Screen;

/// Opens the popup for the focused "Collections" row.
///
/// Supports two callers:
/// * **Edit mode** (`app.edit_mode == true`): drives
///   `app.edit_fields[edit_field_idx]`. The org id comes from the
///   underlying item.
/// * **Create form** (`app.screen == Screen::Create`): drives
///   `app.create_fields[create_field_idx]`. The org id comes from
///   the sibling `Organization` row in the same form.
///
/// No-op + error toast when the focused row isn't a Collections row,
/// when there's no resolvable org id, or when the org has no visible
/// collections at all.
pub fn open(app: &mut App) {
    let in_create = matches!(app.screen, Screen::Create);
    let in_edit = app.edit_mode && matches!(app.screen, Screen::Detail);
    if !in_create && !in_edit {
        return;
    }

    let (target_idx, focused_field_is_collections, current_ids, org_id) = if in_create {
        let Some(field) = app.create_fields.get(app.create_field_idx) else {
            return;
        };
        // Resolve org id from the sibling Organization row.
        let org = app
            .create_fields
            .iter()
            .find(|f| f.is_organization())
            .and_then(|f| f.organization_id.clone());
        (
            app.create_field_idx,
            field.is_collections(),
            field.collection_ids.clone(),
            org,
        )
    } else {
        let Some(field) = app.edit_fields.get(app.edit_field_idx) else {
            return;
        };
        let org = app.selected_item().and_then(|i| i.organization_id.clone());
        (
            app.edit_field_idx,
            field.is_collections(),
            field.collection_ids.clone(),
            org,
        )
    };

    if !focused_field_is_collections {
        app.set_action(ActionState::Error(
            "Move to the Collections row first.".into(),
        ));
        return;
    }
    let Some(org_id) = org_id else {
        app.set_action(ActionState::Error(
            "Pick an organisation first (cycle the Organization row with ← →).".into(),
        ));
        return;
    };

    // Filter to the org and sort by display name.
    let mut available: Vec<crate::domain::Collection> = app
        .collections
        .iter()
        .filter(|c| c.organization_id.as_deref() == Some(org_id.as_str()))
        .cloned()
        .collect();
    available.sort_by_cached_key(|c| c.name.to_lowercase());

    if available.is_empty() {
        app.set_action(ActionState::Error(
            "No collections visible for this organisation.".into(),
        ));
        return;
    }

    let origin = if in_create {
        Screen::Create
    } else {
        Screen::Detail
    };
    app.assign_collections = Some(AssignCollectionsState::new(
        available,
        &current_ids,
        target_idx,
        origin,
        AssignCollectionsPurpose::UpdateField,
    ));
    app.screen = Screen::AssignCollections;
}

/// Opens the popup in **move-to-org** mode for the currently
/// selected detail-screen item.
///
/// Preconditions enforced inline (with friendly error toasts):
/// * The item must be personal (already in an org → use the
///   regular Collections row, not move).
/// * The user must have exactly one organisation membership —
///   the multi-org case requires picking the org first, which is
///   not yet implemented (see audit roadmap).
/// * That org must have at least one visible collection.
pub fn open_for_move(app: &mut App) {
    if !matches!(app.screen, Screen::Detail) || app.edit_mode {
        return;
    }
    let Some(item) = app.selected_item() else {
        return;
    };
    if item.organization_id.is_some() {
        app.set_action(ActionState::Error(
            "This item is already in an organisation — edit Collections instead.".into(),
        ));
        return;
    }
    if app.organizations.len() != 1 {
        let msg = if app.organizations.is_empty() {
            "You're not a member of any organisation — nothing to move into.".into()
        } else {
            format!(
                "Multiple orgs ({}) — pick one with `bw move <id> <org>` from shell for now.",
                app.organizations.len()
            )
        };
        app.set_action(ActionState::Error(msg));
        return;
    }
    let org_id = app.organizations[0].id.clone();
    let item_id = item.id.clone();

    let mut available: Vec<crate::domain::Collection> = app
        .collections
        .iter()
        .filter(|c| c.organization_id.as_deref() == Some(org_id.as_str()))
        .cloned()
        .collect();
    available.sort_by_cached_key(|c| c.name.to_lowercase());
    if available.is_empty() {
        app.set_action(ActionState::Error(
            "No collections visible for this organisation.".into(),
        ));
        return;
    }

    app.assign_collections = Some(AssignCollectionsState::new(
        available,
        &[],
        0,
        Screen::Detail,
        AssignCollectionsPurpose::MoveToOrg {
            item_id,
            organization_id: org_id,
        },
    ));
    app.screen = Screen::AssignCollections;
}

/// Discards the popup and returns to whichever screen the user came
/// from (edit-mode detail or the create form).
pub fn cancel(app: &mut App) {
    let origin = app
        .assign_collections
        .as_ref()
        .map(|s| s.origin.clone())
        .unwrap_or(Screen::Detail);
    app.assign_collections = None;
    app.screen = origin;
}

/// Validates and applies the popup selection. Bw requires org items
/// to be in **at least one** collection — empty selection is
/// rejected with an inline error strip so the user can fix it
/// without losing their progress.
///
/// Branches on [`AssignCollectionsPurpose`]:
/// * `UpdateField`: copy the chosen UUIDs into the matching
///   `EditField`. The actual `bw edit` / `bw create` happens later
///   via the regular Enter-to-save flow.
/// * `MoveToOrg`: call `bw move` directly — the move is the
///   commit. On success the in-memory item is dropped from
///   `app.items` (it now belongs to the org and would re-appear
///   from the next sync); a silent refresh re-fetches the vault
///   so the new state is visible immediately.
pub fn commit(app: &mut App) {
    let Some(state) = app.assign_collections.as_mut() else {
        return;
    };
    if state.selected.is_empty() {
        state.error = true;
        return;
    }
    let ids = state.collected_ids();
    let display: String = state
        .available
        .iter()
        .filter(|c| state.selected.contains(&c.id))
        .map(|c| c.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let target_idx = state.edit_field_idx;
    let origin = state.origin.clone();
    let purpose = state.purpose.clone();
    app.assign_collections = None;

    match purpose {
        AssignCollectionsPurpose::UpdateField => {
            let target_vec = match origin {
                Screen::Create => &mut app.create_fields,
                _ => &mut app.edit_fields,
            };
            if let Some(field) = target_vec.get_mut(target_idx) {
                field.collection_ids = ids;
                field.value = zeroize::Zeroizing::new(display);
                field.cursor = field.value.chars().count();
            }
            app.set_action(ActionState::Done("Collections updated ✓".into()));
            app.screen = origin;
        }
        AssignCollectionsPurpose::MoveToOrg {
            item_id,
            organization_id,
        } => {
            let cmd = format!("bw move {item_id} {organization_id} <ids>");
            app.set_action(ActionState::Running("Moving…".into()));
            match app.vault.move_item(&item_id, &organization_id, &ids) {
                Ok(()) => {
                    app.push_cmd(
                        &cmd,
                        true,
                        &format!("moved into {} collection(s)", ids.len()),
                    );
                    // The item changed organisation context — reload
                    // the vault silently so the indicator (`👥`) and
                    // the collection rows light up. The Done toast
                    // below is preserved by `refresh_items_silent`.
                    super::vault::refresh_items_silent(app);
                    app.set_action(ActionState::Done("Moved ✓".into()));
                    app.screen = Screen::Detail;
                }
                Err(e) => {
                    app.cmd_err(&cmd, &e, "Move failed");
                    app.screen = Screen::Detail;
                }
            }
        }
    }
}
