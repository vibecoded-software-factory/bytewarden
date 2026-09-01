use crate::ports::BwError;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::assign_collections::{AssignCollectionsPurpose, AssignCollectionsState};
use crate::tui::screens::Screen;
use crate::tui::worker::{InFlight, WorkerRequest};

pub fn open(app: &mut App) {
    let in_create = matches!(app.screen, Screen::Create);
    let in_edit = app.edit.active && matches!(app.screen, Screen::Detail);
    if !in_create && !in_edit {
        return;
    }

    let (target_idx, focused_field_is_collections, current_ids, org_id) = if in_create {
        let Some(field) = app.create.fields.get(app.create.field_idx) else {
            return;
        };

        let org = app
            .create
            .fields
            .iter()
            .find(|f| f.is_organization())
            .and_then(|f| f.organization_id.clone());
        (
            app.create.field_idx,
            field.is_collections(),
            field.collection_ids.clone(),
            org,
        )
    } else {
        let Some(field) = app.edit.fields.get(app.edit.field_idx) else {
            return;
        };
        let org = app
            .vault
            .selected_item()
            .and_then(|i| i.organization_id.clone());
        (
            app.edit.field_idx,
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

pub fn open_for_move(app: &mut App) {
    if !matches!(app.screen, Screen::Detail) || app.edit.active {
        return;
    }
    open_for_move_from(app, Screen::Detail);
}

pub fn can_move_selected(app: &App) -> bool {
    let Some(item) = app.vault.selected_item() else {
        return false;
    };
    if item.organization_id.is_some() || app.organizations.len() != 1 {
        return false;
    }
    let org_id = app.organizations[0].id.as_str();
    app.collections
        .iter()
        .any(|c| c.organization_id.as_deref() == Some(org_id))
}

pub fn open_for_move_from(app: &mut App, origin: Screen) {
    let Some(item) = app.vault.selected_item() else {
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
        origin,
        AssignCollectionsPurpose::MoveToOrg {
            item_id,
            organization_id: org_id,
        },
    ));
    app.screen = Screen::AssignCollections;
}

pub fn cancel(app: &mut App) {
    let origin = app
        .assign_collections
        .as_ref()
        .map(|s| s.origin.clone())
        .unwrap_or(Screen::Detail);
    app.assign_collections = None;
    app.screen = origin;
}

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
                Screen::Create => &mut app.create.fields,
                _ => &mut app.edit.fields,
            };
            if let Some(field) = target_vec.get_mut(target_idx) {
                field.collection_ids = ids;
                field.editor.set(display);
            }
            app.set_action(ActionState::Done("Collections updated ✓".into()));
            app.screen = origin;
        }
        AssignCollectionsPurpose::MoveToOrg {
            item_id,
            organization_id,
        } => {
            app.submit(
                InFlight::MoveItem,
                "Moving…",
                WorkerRequest::MoveItem {
                    item_id,
                    organization_id,
                    collection_ids: ids,
                },
            );
        }
    }
}

pub fn handle_move(app: &mut App, r: Result<(), BwError>) {
    match r {
        Ok(()) => {
            app.push_cmd("bw move", true, "moved into organisation");
            app.set_action(ActionState::Done("Moved ✓".into()));
            app.screen = Screen::Detail;
            if app.begin(InFlight::MoveReloadItems) {
                let _ = app.worker_tx.send(WorkerRequest::ListItems);
            }
        }
        Err(e) => {
            app.cmd_err("bw move", &e, "Move failed");
            app.screen = Screen::Detail;
        }
    }
}

pub fn handle_move_reload(app: &mut App, r: Result<Vec<crate::domain::Item>, BwError>) {
    match r {
        Ok(items) => super::vault::set_items_keep_cursor(app, items),
        Err(e) => app.push_cmd("bw list items", false, &e),
    }
}
