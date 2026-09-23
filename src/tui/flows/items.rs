use crate::domain::LineEditor;
use crate::ports::BwError;
use serde_json::Value;
use zeroize::Zeroizing;

use crate::domain::filter::{CREATE_ITEM_TYPES, CreateItemType, ItemFilter};
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::detail_fields::build_detail_fields;
use crate::tui::edit_field::{build_create_fields_with_orgs, build_edit_fields_with_folders};
use crate::tui::flows::item_json::{build_create_payload, patch_edit_payload};
use crate::tui::flows::vault;
use crate::tui::screens::{Focus, Screen};
use crate::tui::worker::{InFlight, WorkerRequest};

pub fn open_create(app: &mut App) {
    app.create.type_idx = 0;
    app.create.item_type = CreateItemType::Login;
    app.create.choosing_type = true;
    app.create.fields = Vec::new();
    app.create.field_idx = 0;
    app.screen = Screen::Create;
}

pub fn create_select_type(app: &mut App) {
    app.create.item_type = CREATE_ITEM_TYPES[app.create.type_idx].clone();
    app.create.fields = build_create_fields_with_orgs(&app.create.item_type, &app.organizations);
    app.create.field_idx = 0;
    app.create.choosing_type = false;
}

pub fn cycle_create_org(app: &mut App, dir: i32) {
    if app.organizations.is_empty() {
        return;
    }
    let Some(field) = app.create.fields.get(app.create.field_idx) else {
        return;
    };
    if !field.is_organization() {
        return;
    }
    let current_id = field.organization_id.clone();

    let mut ids: Vec<Option<String>> = vec![None];
    ids.extend(app.organizations.iter().map(|o| Some(o.id.clone())));
    let cur_pos = ids.iter().position(|i| i == &current_id).unwrap_or(0);
    let len = ids.len() as i32;
    let new_pos = (((cur_pos as i32 + dir) % len) + len) % len;
    let new_id = ids[new_pos as usize].clone();
    let new_display = match &new_id {
        None => "Personal".to_string(),
        Some(id) => app
            .organizations
            .iter()
            .find(|o| &o.id == id)
            .map(|o| o.name.clone())
            .unwrap_or_else(|| "Personal".into()),
    };
    if let Some(f) = app.create.fields.get_mut(app.create.field_idx) {
        f.editor.set(new_display);
        f.organization_id = new_id.clone();
    }

    let org_idx = app.create.field_idx;
    let coll_pos = app.create.fields.iter().position(|f| f.is_collections());
    match (coll_pos, new_id.as_ref()) {
        (Some(pos), None) => {
            app.create.fields.remove(pos);
        }
        (Some(pos), Some(_)) => {
            if let Some(f) = app.create.fields.get_mut(pos) {
                f.editor.clear();
                f.collection_ids = Vec::new();
            }
        }
        (None, Some(_)) => {
            app.create.fields.insert(
                org_idx + 1,
                crate::tui::edit_field::EditField::collections("", Vec::new()),
            );
        }
        (None, None) => {}
    }
}

pub fn queue_create_item(app: &mut App) {
    let name = app
        .create
        .fields
        .first()
        .map(|f| f.value().trim().to_string())
        .unwrap_or_default();
    if name.is_empty() {
        app.set_action(ActionState::Error("Name is required".into()));
        return;
    }

    let org_set = app
        .create
        .fields
        .iter()
        .find(|f| f.is_organization())
        .and_then(|f| f.organization_id.clone());
    if org_set.is_some() {
        let coll_count = app
            .create
            .fields
            .iter()
            .find(|f| f.is_collections())
            .map(|f| f.collection_ids.len())
            .unwrap_or(0);
        if coll_count == 0 {
            app.set_action(ActionState::Error(
                "Pick at least one collection (Alt+L on the Collections row).".into(),
            ));
            return;
        }
    }
    let json = Zeroizing::new(build_create_payload(
        &app.create.item_type,
        &app.create.fields,
    ));
    app.submit(
        InFlight::CreateItem,
        "Creating…",
        WorkerRequest::CreateItem { json },
    );
}

pub fn handle_create(app: &mut App, r: Result<crate::domain::Item, BwError>) {
    let cmd = "bw create item".to_string();
    match r {
        Ok(item) => {
            let new_id = item.id.clone();
            let name = item.name.clone();
            app.vault.items.push(item);
            app.vault.sort_items();

            let new_idx = app
                .vault
                .filtered_items()
                .iter()
                .position(|i| i.id == new_id);
            match new_idx {
                Some(idx) => {
                    app.vault.selected_index = idx;
                    app.vault.scroll_offset = idx.saturating_sub(5);
                }
                None => {
                    app.vault.selected_index = 0;
                    app.vault.scroll_offset = 0;
                }
            }
            app.push_cmd(&cmd, true, &format!("created: {name}"));
            app.set_action(ActionState::Done("Created ✓".into()));
            app.screen = Screen::Vault;
        }
        Err(e) => app.cmd_err(&cmd, &e, "Create failed"),
    }
}

pub fn enter_edit_mode(app: &mut App) {
    let Some(item) = app.vault.selected_item() else {
        return;
    };
    let item = item.clone();

    let detail_label = build_detail_fields(&item, false, 0)
        .into_iter()
        .nth(app.detail_field)
        .map(|f| f.label);

    let fields = build_edit_fields_with_folders(&item, &app.folders, &app.collections);
    let initial_idx = detail_label
        .and_then(|lbl| fields.iter().position(|f| f.label == lbl))
        .unwrap_or(0)
        .min(fields.len().saturating_sub(1));

    app.edit.item_id = item.id.clone();
    app.edit.fields = fields;
    app.edit.field_idx = initial_idx;
    app.edit.active = true;
}

#[derive(Debug, Clone)]
pub struct AttachmentUploadState {
    pub path: LineEditor,

    pub item_id: String,

    pub item_name: String,
}

pub fn open_attachment_upload(app: &mut App) {
    let Some(item) = app.vault.selected_item() else {
        app.set_action(ActionState::Error(
            "Pick an item to attach a file to.".into(),
        ));
        return;
    };
    app.attachment_upload = Some(AttachmentUploadState {
        path: LineEditor::new(),
        item_id: item.id.clone(),
        item_name: item.name.clone(),
    });
    app.screen = Screen::AttachmentUpload;
}

pub fn cancel_attachment_upload(app: &mut App) {
    app.attachment_upload = None;
    app.screen = Screen::Detail;
}

#[derive(Debug, Clone)]
pub struct AttachmentDownloadState {
    pub path: LineEditor,

    pub item_id: String,

    pub item_name: String,

    pub file_name: String,
}

pub fn default_download_path(file_name: &str) -> String {
    let downloads = std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join("Downloads"))
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    unique_path(&downloads, file_name)
}

fn unique_path(dir: &std::path::Path, file_name: &str) -> String {
    let candidate = dir.join(file_name);
    if !candidate.exists() {
        return candidate.to_string_lossy().into_owned();
    }
    let (stem, ext) = split_name(file_name);
    for n in 1..1000 {
        let suffixed = if ext.is_empty() {
            format!("{stem}_{n}")
        } else {
            format!("{stem}_{n}.{ext}")
        };
        let p = dir.join(suffixed);
        if !p.exists() {
            return p.to_string_lossy().into_owned();
        }
    }
    candidate.to_string_lossy().into_owned()
}

fn split_name(file_name: &str) -> (String, String) {
    if let Some(idx) = file_name.rfind('.')
        && idx > 0
    {
        let (s, e) = file_name.split_at(idx);
        (s.to_string(), e.trim_start_matches('.').to_string())
    } else {
        (file_name.to_string(), String::new())
    }
}

pub fn open_attachment_download(app: &mut App) {
    let Some(item) = app.vault.selected_item() else {
        return;
    };
    let Some(att) = crate::tui::detail_fields::attachment_at(item, app.detail_field) else {
        app.set_action(ActionState::Error(
            "Move to an attachment row first.".into(),
        ));
        return;
    };
    let path = default_download_path(&att.file_name);
    app.attachment_download = Some(AttachmentDownloadState {
        path: LineEditor::with_text(path),
        item_id: item.id.clone(),
        item_name: item.name.clone(),
        file_name: att.file_name.clone(),
    });
    app.screen = Screen::AttachmentDownload;
}

pub fn cancel_attachment_download(app: &mut App) {
    app.attachment_download = None;
    app.screen = Screen::Detail;
}

pub fn queue_attachment_download(app: &mut App) {
    let Some(state) = app.attachment_download.as_ref() else {
        return;
    };
    if state.path.text().trim().is_empty() {
        app.set_action(ActionState::Error("Output path cannot be empty.".into()));
        return;
    }
    let item_id = state.item_id.clone();
    let file_name = state.file_name.clone();
    let output_path = state.path.text().trim().to_string();
    app.submit(
        InFlight::DownloadAttachment,
        "Downloading…",
        WorkerRequest::DownloadAttachment {
            item_id,
            file_name,
            output_path,
        },
    );
}

pub fn handle_download_attachment(app: &mut App, r: Result<(), BwError>) {
    let Some(state) = app.attachment_download.as_ref() else {
        return;
    };
    let item_id = state.item_id.clone();
    let item_name = state.item_name.clone();
    let file_name = state.file_name.clone();
    let path = state.path.text().trim().to_string();
    let cmd = format!("bw get attachment {file_name} --itemid {item_id} --output <path>");
    match r {
        Ok(()) => {
            app.push_cmd(&cmd, true, &format!("saved to {path}"));
            app.set_action(ActionState::Done(format!(
                "Downloaded \"{file_name}\" from \"{item_name}\" ✓"
            )));
            app.attachment_download = None;
            app.screen = Screen::Detail;
        }
        Err(e) => app.cmd_err(&cmd, &e, "Download failed"),
    }
}

#[derive(Debug, Clone)]
pub struct AttachmentDeleteState {
    pub item_id: String,
    pub item_name: String,

    pub attachment_id: String,

    pub file_name: String,
}

pub fn open_confirm_delete_attachment(app: &mut App) {
    let Some(item) = app.vault.selected_item() else {
        return;
    };
    let Some(att) = crate::tui::detail_fields::attachment_at(item, app.detail_field) else {
        app.set_action(ActionState::Error(
            "Move to an attachment row first.".into(),
        ));
        return;
    };
    app.attachment_delete = Some(AttachmentDeleteState {
        item_id: item.id.clone(),
        item_name: item.name.clone(),
        attachment_id: att.id.clone(),
        file_name: att.file_name.clone(),
    });
    app.screen = Screen::ConfirmDeleteAttachment;
}

pub fn cancel_delete_attachment(app: &mut App) {
    app.attachment_delete = None;
    app.screen = Screen::Detail;
}

pub fn queue_delete_attachment(app: &mut App) {
    let Some(state) = app.attachment_delete.as_ref() else {
        return;
    };
    let item_id = state.item_id.clone();
    let attachment_id = state.attachment_id.clone();
    app.submit(
        InFlight::DeleteAttachment,
        "Deleting attachment…",
        WorkerRequest::DeleteAttachment {
            item_id,
            attachment_id,
        },
    );
}

pub fn handle_delete_attachment(app: &mut App, r: Result<(), BwError>) {
    let Some(state) = app.attachment_delete.as_ref() else {
        return;
    };
    let item_id = state.item_id.clone();
    let item_name = state.item_name.clone();
    let attachment_id = state.attachment_id.clone();
    let cmd = format!("bw delete attachment {attachment_id} --itemid {item_id}");
    match r {
        Ok(()) => {
            app.push_cmd(&cmd, true, &format!("deleted from {item_name}"));

            if app.begin(InFlight::DeleteAttachmentRefresh {
                item_id: item_id.clone(),
            }) {
                let _ = app.worker_tx.send(WorkerRequest::GetItemJson { item_id });
            }
        }
        Err(e) => app.cmd_err(&cmd, &e, "Delete failed"),
    }
}

pub fn handle_delete_attachment_refresh(
    app: &mut App,
    item_id: String,
    r: Result<Zeroizing<String>, BwError>,
) {
    let file_name = app
        .attachment_delete
        .as_ref()
        .map(|s| s.file_name.clone())
        .unwrap_or_default();

    if let Ok(json) = r
        && let Ok(refreshed) = serde_json::from_str::<crate::domain::Item>(&json)
        && let Some(slot) = app.vault.items.iter_mut().find(|i| i.id == item_id)
    {
        *slot = refreshed;
        app.vault.rebuild_caches();
    }
    let count = app.detail_field_count();
    if count > 0 && app.detail_field >= count {
        app.detail_field = count - 1;
    }
    app.set_action(ActionState::Done(format!("Deleted \"{file_name}\" ✓")));
    app.attachment_delete = None;
    app.screen = Screen::Detail;
}

pub fn commit_attachment_upload(app: &mut App) {
    let Some(state) = app.attachment_upload.as_ref() else {
        return;
    };
    let path = state.path.text().trim().to_string();
    if path.is_empty() {
        app.set_action(ActionState::Error("File path cannot be empty.".into()));
        return;
    }
    let item_id = state.item_id.clone();
    app.submit(
        InFlight::UploadAttachment,
        "Uploading…",
        WorkerRequest::UploadAttachment {
            item_id,
            file_path: path,
        },
    );
}

pub fn handle_upload_attachment(app: &mut App, r: Result<crate::domain::Item, BwError>) {
    let Some(state) = app.attachment_upload.as_ref() else {
        return;
    };
    let item_id = state.item_id.clone();
    let item_name = state.item_name.clone();
    let cmd = format!("bw create attachment --file <path> --itemid {item_id}");
    match r {
        Ok(updated) => {
            app.push_cmd(&cmd, true, &format!("uploaded to {item_name}"));
            if let Some(slot) = app.vault.items.iter_mut().find(|i| i.id == item_id) {
                *slot = updated;
                app.vault.rebuild_caches();
            }
            app.set_action(ActionState::Done(format!("Attached to \"{item_name}\" ✓")));
            app.attachment_upload = None;
            app.screen = Screen::Detail;
        }
        Err(e) => app.cmd_err(&cmd, &e, "Upload failed"),
    }
}

#[derive(Debug, Clone)]
pub struct RenameFieldState {
    pub input: LineEditor,

    pub target_idx: usize,
}

impl RenameFieldState {
    fn new(target_idx: usize, current: &str) -> Self {
        Self {
            input: LineEditor::with_text(current),
            target_idx,
        }
    }
}

pub fn open_rename_field(app: &mut App) {
    if !app.edit.active {
        return;
    }
    let Some(field) = app.edit.fields.get(app.edit.field_idx) else {
        return;
    };
    if !field.is_custom() {
        app.set_action(ActionState::Error(
            "Only custom fields can be renamed.".into(),
        ));
        return;
    }
    app.rename_field = Some(RenameFieldState::new(app.edit.field_idx, &field.label));
    app.screen = Screen::RenameField;
}

pub fn commit_rename_field(app: &mut App) {
    let Some(state) = app.rename_field.take() else {
        return;
    };
    let new_label = state.input.text().trim().to_string();
    if new_label.is_empty() {
        app.rename_field = Some(state);
        app.set_action(ActionState::Error("Field name cannot be empty.".into()));
        return;
    }

    let current_label: Option<String> = app
        .edit
        .fields
        .get(state.target_idx)
        .filter(|f| f.is_custom())
        .map(|f| f.label.clone());
    let siblings: Vec<&str> = app
        .edit
        .fields
        .iter()
        .enumerate()
        .filter(|(i, f)| *i != state.target_idx && f.is_custom())
        .map(|(_, f)| f.label.as_str())
        .collect();
    if let Err(msg) = crate::domain::validation::check_name_unique(
        &new_label,
        &siblings,
        current_label.as_deref(),
    ) {
        app.rename_field = Some(state);
        app.set_action(ActionState::Error(msg));
        return;
    }
    if let Some(field) = app.edit.fields.get_mut(state.target_idx) {
        if !field.is_custom() {
            app.set_action(ActionState::Error(
                "Field is no longer a custom row.".into(),
            ));
        } else {
            field.label = new_label;
            app.set_action(ActionState::Done("Renamed ✓".into()));
        }
    }
    app.screen = Screen::Detail;
}

pub fn cancel_rename_field(app: &mut App) {
    app.rename_field = None;
    app.screen = Screen::Detail;
}

pub fn add_custom_field(app: &mut App) {
    if !app.edit.active {
        return;
    }
    let next_n = app
        .edit
        .fields
        .iter()
        .filter_map(|f| {
            f.label
                .strip_prefix("Custom ")
                .and_then(|s| s.parse::<u32>().ok())
        })
        .max()
        .map(|n| n + 1)
        .unwrap_or(1);
    let label = format!("Custom {next_n}");
    let new_field = crate::tui::edit_field::EditField::custom(&label, "", 0);
    app.edit.fields.push(new_field);
    app.edit.field_idx = app.edit.fields.len() - 1;
    app.set_action(ActionState::Done(format!("Added {label} ✓")));
}

pub fn remove_current_field(app: &mut App) {
    if !app.edit.active {
        return;
    }
    let Some(field) = app.edit.fields.get(app.edit.field_idx) else {
        return;
    };
    if field.is_uri() {
        return remove_uri_row(app);
    }
    if !field.is_custom() {
        app.set_action(ActionState::Error(
            "Only custom and URL rows can be removed.".into(),
        ));
        return;
    }
    let removed_label = field.label.clone();
    app.edit.fields.remove(app.edit.field_idx);
    if app.edit.field_idx >= app.edit.fields.len() && !app.edit.fields.is_empty() {
        app.edit.field_idx = app.edit.fields.len() - 1;
    }
    app.set_action(ActionState::Done(format!("Removed {removed_label} ✓")));
}

pub fn add_uri_row(app: &mut App) {
    if !app.edit.active {
        return;
    }
    use crate::tui::edit_field::{EditField, EditFieldKind};

    let next_idx = app
        .edit
        .fields
        .iter()
        .filter_map(|f| match f.kind {
            EditFieldKind::Uri { index, .. } => Some(index),
            _ => None,
        })
        .max()
        .map(|i| i + 1)
        .unwrap_or(0);

    let last_uri_pos = app
        .edit
        .fields
        .iter()
        .rposition(|f| matches!(f.kind, EditFieldKind::Uri { .. }));
    let insert_at = last_uri_pos.map(|p| p + 1).unwrap_or_else(|| {
        app.edit
            .fields
            .iter()
            .position(|f| f.label == "Password")
            .map(|p| p + 1)
            .unwrap_or(app.edit.fields.len())
    });

    let url_label = format!("URL {}", next_idx + 1);
    let match_label = format!("URL {} Match", next_idx + 1);
    app.edit
        .fields
        .insert(insert_at, EditField::uri_url(&url_label, "", next_idx));
    app.edit.fields.insert(
        insert_at + 1,
        EditField::uri_match(&match_label, "", next_idx),
    );

    relabel_uris(app);

    app.edit.field_idx = insert_at;
    app.set_action(ActionState::Done(format!("Added URL {} ✓", next_idx + 1)));
}

pub fn remove_uri_row(app: &mut App) {
    if !app.edit.active {
        return;
    }
    use crate::tui::edit_field::EditFieldKind;

    let Some(focused) = app.edit.fields.get(app.edit.field_idx) else {
        return;
    };
    let target_index = match focused.kind {
        EditFieldKind::Uri { index, .. } => index,
        _ => {
            app.set_action(ActionState::Error("Focused row is not a URL.".into()));
            return;
        }
    };

    app.edit
        .fields
        .retain(|f| !matches!(f.kind, EditFieldKind::Uri { index, .. } if index == target_index));
    if app.edit.field_idx >= app.edit.fields.len() && !app.edit.fields.is_empty() {
        app.edit.field_idx = app.edit.fields.len() - 1;
    }

    relabel_uris(app);
    app.set_action(ActionState::Done(format!(
        "Removed URL {} ✓",
        target_index + 1
    )));
}

fn relabel_uris(app: &mut App) {
    use crate::tui::edit_field::{EditFieldKind, UriRole};

    let positions: Vec<(usize, UriRole)> = app
        .edit
        .fields
        .iter()
        .enumerate()
        .filter_map(|(pos, f)| match f.kind {
            EditFieldKind::Uri { role, .. } => Some((pos, role)),
            _ => None,
        })
        .collect();

    let url_count = positions
        .iter()
        .filter(|(_, r)| matches!(r, UriRole::Url))
        .count();
    let multi = url_count > 1;

    let mut next_visual: usize = 0;
    for (pos, role) in positions {
        if matches!(role, UriRole::Url) {
            next_visual += 1;
        }
        let label = match (role, multi) {
            (UriRole::Url, false) => "URL".to_string(),
            (UriRole::Match, false) => "URL Match".to_string(),
            (UriRole::Url, true) => format!("URL {next_visual}"),
            (UriRole::Match, true) => format!("URL {next_visual} Match"),
        };
        if let Some(field) = app.edit.fields.get_mut(pos) {
            field.label = label;

            field.kind = EditFieldKind::Uri {
                index: next_visual.saturating_sub(1),
                role,
            };
        }
    }
}

pub fn cycle_field_type(app: &mut App) {
    if !app.edit.active {
        return;
    }
    let Some(field) = app.edit.fields.get_mut(app.edit.field_idx) else {
        return;
    };
    let Some(t) = field.custom_type() else {
        app.set_action(ActionState::Error(
            "Only custom fields have a configurable type.".into(),
        ));
        return;
    };
    if t == 3 {
        app.set_action(ActionState::Error(
            "Linked fields are read-only here — use the Bitwarden GUI to change them.".into(),
        ));
        return;
    }
    let next = (t + 1) % 3;
    field.set_custom_type(next);
    let label = match next {
        0 => "text",
        1 => "hidden",
        2 => "boolean",
        _ => "?",
    };
    app.set_action(ActionState::Done(format!("Type → {label} ✓")));
}

pub fn queue_save_edit(app: &mut App) {
    let item_id = app.edit.item_id.clone();
    app.submit(
        InFlight::SaveEditFetch,
        "Saving…",
        WorkerRequest::GetItemJson { item_id },
    );
}

pub fn handle_save_edit_fetch(app: &mut App, r: Result<Zeroizing<String>, BwError>) {
    let item_id = app.edit.item_id.clone();
    let cmd = format!("bw edit item {item_id}");
    let base_json = match r {
        Ok(j) => j,
        Err(e) => return app.cmd_err(&cmd, &e, "Fetch failed"),
    };

    let folders_snapshot = app.folders.clone();
    let edit_fields_resolved: Vec<crate::tui::edit_field::EditField> = app
        .edit
        .fields
        .iter()
        .map(|f| {
            if f.label != "Folder" {
                return f.clone();
            }
            let mut clone = f.clone();
            clone.editor.set(
                crate::tui::flows::folders::id_by_name(&folders_snapshot, f.value())
                    .unwrap_or_default(),
            );
            clone
        })
        .collect();

    let patched = Zeroizing::new(patch_edit_payload(&base_json, &edit_fields_resolved));

    if app.begin(InFlight::SaveEditCommit) {
        let _ = app.worker_tx.send(WorkerRequest::EditItem {
            item_id,
            json: patched,
        });
    }
}

pub fn handle_save_edit_commit(app: &mut App, r: Result<crate::domain::Item, BwError>) {
    let item_id = app.edit.item_id.clone();
    let cmd = format!("bw edit item {item_id}");
    match r {
        Ok(updated) => {
            let name = updated.name.clone();
            if let Some(i) = app.vault.items.iter_mut().find(|i| i.id == item_id) {
                *i = updated;
            }
            app.vault.sort_items();
            app.push_cmd(&cmd, true, &format!("saved: {name}"));
            app.set_action(ActionState::Done("Saved ✓".into()));
            app.edit.active = false;
        }
        Err(e) => app.cmd_err(&cmd, &e, "Save failed"),
    }
}

pub fn open_confirm_delete(app: &mut App) {
    if app.vault.selected_item().is_some() {
        app.screen = Screen::ConfirmDelete;
    }
}

pub fn queue_delete_item(app: &mut App, permanent: bool) {
    let Some(item) = app.vault.selected_item() else {
        return;
    };
    let (item_id, name) = (item.id.clone(), item.name.clone());
    let label = if permanent {
        "Deleting…"
    } else {
        "Trashing…"
    };
    app.screen = Screen::Vault;
    app.submit(
        InFlight::DeleteItem {
            permanent,
            item_id: item_id.clone(),
            name,
        },
        label,
        WorkerRequest::DeleteItem { item_id, permanent },
    );
}

pub fn handle_delete(
    app: &mut App,
    permanent: bool,
    item_id: String,
    name: String,
    r: Result<(), BwError>,
) {
    let perm_str = if permanent { " --permanent" } else { "" };
    let cmd = format!("bw delete item {item_id}{perm_str}");
    match r {
        Ok(()) => {
            app.vault.items.retain(|i| i.id != item_id);
            app.vault.rebuild_caches();

            app.vault.reanchor_selection(None);
            let label = if permanent {
                "deleted permanently"
            } else {
                "moved to trash"
            };
            app.push_cmd(&cmd, true, &format!("{name} {label}"));
            app.set_action(ActionState::Done(
                if permanent {
                    "Deleted ✓"
                } else {
                    "Trashed ✓"
                }
                .into(),
            ));

            if app.begin(InFlight::DeleteReloadTrash) {
                let _ = app.worker_tx.send(WorkerRequest::ListTrash);
            }
        }
        Err(e) => app.cmd_err(&cmd, &e, "Delete failed"),
    }
}

pub fn handle_delete_reload_trash(app: &mut App, r: Result<Vec<crate::domain::Item>, BwError>) {
    match r {
        Ok(items) => {
            vault::set_trash(app, items);
        }
        Err(e) => app.push_cmd("bw list items --trash", false, &e),
    }
}

pub fn queue_restore_item(app: &mut App) {
    let Some(item) = app.vault.selected_item() else {
        return;
    };
    let (item_id, name) = (item.id.clone(), item.name.clone());
    app.submit(
        InFlight::RestoreItem {
            item_id: item_id.clone(),
            name,
        },
        "Restoring…",
        WorkerRequest::RestoreItem { item_id },
    );
}

pub fn handle_restore(app: &mut App, item_id: String, name: String, r: Result<(), BwError>) {
    let cmd = format!("bw restore item {item_id}");
    match r {
        Ok(()) => {
            app.vault.trashed_items.retain(|i| i.id != item_id);
            app.vault.rebuild_caches();
            app.push_cmd(&cmd, true, &format!("{name} restored to vault"));
            app.screen = Screen::Vault;
            app.vault.active_filter = ItemFilter::All;
            app.vault.filter_selected = 0;
            app.vault.selected_index = 0;
            app.vault.scroll_offset = 0;
            app.focus = Focus::Search;
            app.set_action(ActionState::Done("Restored ✓".into()));

            if app.begin(InFlight::RestoreReloadItems) {
                let _ = app.worker_tx.send(WorkerRequest::ListItems);
            }
        }
        Err(e) => app.cmd_err(&cmd, &e, "Restore failed"),
    }
}

pub fn handle_restore_reload(app: &mut App, r: Result<Vec<crate::domain::Item>, BwError>) {
    match r {
        Ok(items) => vault::set_items(app, items),
        Err(e) => app.push_cmd("bw list items", false, &e),
    }
}

pub fn queue_check_exposed(app: &mut App) {
    let Some(item) = app.vault.selected_item() else {
        return;
    };
    if item.login.is_none() {
        app.set_action(ActionState::Error(
            "Only login items can be checked.".into(),
        ));
        return;
    }
    let id = item.id.clone();
    app.submit(
        InFlight::CheckExposed,
        "Checking HIBP…",
        WorkerRequest::CheckExposed { item_id: id },
    );
}

pub fn handle_check_exposed(app: &mut App, r: Result<u32, BwError>) {
    let cmd = "bw get exposed".to_string();
    match r {
        Ok(0) => {
            app.push_cmd(&cmd, true, "0 breaches");
            app.set_action(ActionState::Done("Not in any known breach ✓".into()));
        }
        Ok(n) => {
            app.push_cmd(&cmd, true, &format!("{n} breaches"));

            app.set_action(ActionState::Error(format!(
                "{} Found in {n} breach{} — rotate this password",
                app.icons.warning(),
                if n == 1 { "" } else { "es" }
            )));
        }
        Err(e) => app.cmd_err(&cmd, &e, "HIBP check failed"),
    }
}

pub fn toggle_favorite(app: &mut App) {
    let Some(item) = app.vault.selected_item() else {
        return;
    };
    let item_id = item.id.clone();
    app.submit(
        InFlight::ToggleFavoriteFetch {
            item_id: item_id.clone(),
        },
        "Updating…",
        WorkerRequest::GetItemJson { item_id },
    );
}

pub fn handle_toggle_fetch(app: &mut App, item_id: String, r: Result<Zeroizing<String>, BwError>) {
    let cmd = format!("bw edit item {item_id}");
    let json = match r {
        Ok(j) => j,
        Err(e) => return app.cmd_err(&cmd, &e, "Fetch failed"),
    };
    let new_fav = match app.vault.items.iter().find(|i| i.id == item_id) {
        Some(i) => !i.favorite,
        None => return,
    };
    let mut val: Value = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(e) => return app.cmd_err(&cmd, &format!("JSON parse error: {e}"), "Failed"),
    };
    val["favorite"] = Value::Bool(new_fav);
    let new_json = match serde_json::to_string(&val) {
        Ok(s) => Zeroizing::new(s),
        Err(e) => return app.cmd_err(&cmd, &format!("JSON serialize error: {e}"), "Failed"),
    };

    if app.begin(InFlight::ToggleFavoriteCommit {
        new_favorite: new_fav,
    }) {
        let _ = app.worker_tx.send(WorkerRequest::EditItem {
            item_id,
            json: new_json,
        });
    }
}

pub fn handle_toggle_commit(
    app: &mut App,
    new_favorite: bool,
    r: Result<crate::domain::Item, BwError>,
) {
    let cmd = "bw edit item".to_string();
    match r {
        Ok(updated) => {
            if let Some(i) = app.vault.items.iter_mut().find(|i| i.id == updated.id) {
                i.favorite = new_favorite;
            }

            app.vault.rebuild_filtered_cache();
            let label = if new_favorite {
                "★ Favorited"
            } else {
                "Unfavorited"
            };
            app.set_action(ActionState::Done(label.into()));
            app.push_cmd(&cmd, true, label);
        }
        Err(e) => app.cmd_err(&cmd, &e, "Failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn split_name_handles_normal_extension() {
        assert_eq!(split_name("file.pdf"), ("file".into(), "pdf".into()));
        assert_eq!(
            split_name("photo.tar.gz"),
            ("photo.tar".into(), "gz".into())
        );
    }

    #[test]
    fn split_name_handles_no_extension() {
        assert_eq!(split_name("README"), ("README".into(), "".into()));
    }

    #[test]
    fn split_name_dotfile_is_all_stem() {
        assert_eq!(split_name(".bashrc"), (".bashrc".into(), "".into()));
    }

    #[test]
    fn unique_path_returns_input_when_free() {
        let tmp = TempDir::new().unwrap();
        let p = unique_path(tmp.path(), "fresh.pdf");
        assert!(p.ends_with("fresh.pdf"));
    }

    #[test]
    fn unique_path_suffixes_when_target_exists() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("dup.pdf"), b"x").unwrap();
        let p = unique_path(tmp.path(), "dup.pdf");
        assert!(p.ends_with("dup_1.pdf"), "got {p}");
    }

    #[test]
    fn unique_path_skips_taken_numbered_suffixes() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("note.txt"), b"x").unwrap();
        std::fs::write(tmp.path().join("note_1.txt"), b"x").unwrap();
        std::fs::write(tmp.path().join("note_2.txt"), b"x").unwrap();
        let p = unique_path(tmp.path(), "note.txt");
        assert!(p.ends_with("note_3.txt"), "got {p}");
    }

    #[test]
    fn unique_path_handles_extensionless_files() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("README"), b"x").unwrap();
        let p = unique_path(tmp.path(), "README");
        assert!(p.ends_with("README_1"), "got {p}");
    }

    #[test]
    fn default_download_path_uses_home_downloads_when_set() {
        let tmp = TempDir::new().unwrap();

        let prev = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", tmp.path());
        }
        std::fs::create_dir_all(tmp.path().join("Downloads")).unwrap();
        let path = default_download_path("file.pdf");
        let expected = tmp.path().join("Downloads/file.pdf");
        assert_eq!(path, expected.to_string_lossy());
        unsafe {
            match prev {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
        }
    }
}
