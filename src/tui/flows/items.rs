//! Create / edit / delete / restore / favorite flows.

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

// ── Create ────────────────────────────────────────────────────────────────

/// Opens the "create item" screen on the type-picker step.
pub fn open_create(app: &mut App) {
    app.create.type_idx = 0;
    app.create.item_type = CreateItemType::Login;
    app.create.choosing_type = true;
    app.create.fields = Vec::new();
    app.create.field_idx = 0;
    app.screen = Screen::Create;
}

/// Confirms the type-picker selection and renders the matching form.
pub fn create_select_type(app: &mut App) {
    app.create.item_type = CREATE_ITEM_TYPES[app.create.type_idx].clone();
    app.create.fields = build_create_fields_with_orgs(&app.create.item_type, &app.organizations);
    app.create.field_idx = 0;
    app.create.choosing_type = false;
}

/// Cycles the create-form's "Organization" row by `dir` (+1 right,
/// -1 left) through `[Personal, Org A, Org B, …, Personal]`.
///
/// Side-effect: when the new selection differs from the previous
/// one, any sibling "Collections" row is rebuilt from scratch
/// (removed if going to Personal, replaced with an empty row if
/// going to a different org). The user fills the Collections row
/// via the regular `Alt+L` popup.
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
    // Build the cycle list: [None, org0.id, org1.id, …]. Cycling
    // right goes from the current position to the next; cycling
    // left goes to the previous.
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
        f.value = zeroize::Zeroizing::new(new_display);
        f.cursor = f.value.chars().count();
        f.organization_id = new_id.clone();
    }
    // Sync the sibling Collections row.
    let org_idx = app.create.field_idx;
    let coll_pos = app.create.fields.iter().position(|f| f.is_collections());
    match (coll_pos, new_id.as_ref()) {
        (Some(pos), None) => {
            // Going Personal → drop the row.
            app.create.fields.remove(pos);
        }
        (Some(pos), Some(_)) => {
            // Switched org → reset the row (user must reselect).
            if let Some(f) = app.create.fields.get_mut(pos) {
                f.value = zeroize::Zeroizing::new(String::new());
                f.cursor = 0;
                f.collection_ids = Vec::new();
            }
        }
        (None, Some(_)) => {
            // Personal → real org. Insert a Collections row right
            // after the Organization row so the form layout stays
            // grouped.
            app.create.fields.insert(
                org_idx + 1,
                crate::tui::edit_field::EditField::collections("", Vec::new()),
            );
        }
        (None, None) => {} // Both absent — nothing to do.
    }
}

/// Validates the create form and dispatches the create to the worker.
pub fn queue_create_item(app: &mut App) {
    let name = app
        .create
        .fields
        .first()
        .map(|f| f.value.trim().to_string())
        .unwrap_or_default();
    if name.is_empty() {
        app.set_action(ActionState::Error("Name is required".into()));
        return;
    }
    // Bw requires org-owned items to live in ≥1 collection. The
    // form's Organization row carries the resolved id; if it's set
    // we expect a sibling Collections row with at least one UUID.
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

/// `bw create item` response.
pub fn handle_create(app: &mut App, r: Result<crate::domain::Item, BwError>) {
    let cmd = "bw create item".to_string();
    match r {
        Ok(item) => {
            let new_id = item.id.clone();
            let name = item.name.clone();
            app.items.push(item);
            app.sort_items();
            // Resolve the new item's position against the *visible* list
            // (which may differ from `app.items` if the user has an
            // active search query). When the new item is hidden by the
            // current filter / search, fall back to the top of the
            // visible list so the highlight is never out of bounds.
            let new_idx = app.filtered_items().iter().position(|i| i.id == new_id);
            match new_idx {
                Some(idx) => {
                    app.selected_index = idx;
                    app.scroll_offset = idx.saturating_sub(5);
                }
                None => {
                    app.selected_index = 0;
                    app.scroll_offset = 0;
                }
            }
            app.push_cmd(&cmd, true, &format!("created: {name}"));
            app.set_action(ActionState::Done("Created ✓".into()));
            app.screen = Screen::Vault;
        }
        Err(e) => app.cmd_err(&cmd, &e, "Create failed"),
    }
}

// ── Edit ──────────────────────────────────────────────────────────────────

/// Switches the detail screen into edit mode.
///
/// Tries to land the cursor on the same conceptual field the user was
/// viewing in the detail screen. Detail and edit field lists are *not*
/// position-equivalent (detail skips empty optionals; edit shows them
/// all), so we look up the focused detail row's label and find the
/// matching label in the edit form. When no match exists (e.g. the
/// "Type" pseudo-field, which isn't editable), fall back to the first
/// editable row.
pub fn enter_edit_mode(app: &mut App) {
    let Some(item) = app.selected_item() else {
        return;
    };
    let item = item.clone();

    // Resolve the label of the currently-focused detail row before we
    // build the edit form, so the lookup uses the same data the user
    // was looking at.
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

// ── Attachment upload popup ───────────────────────────────────────────────

/// Buffer for the in-flight attachment-upload popup.
#[derive(Debug, Clone)]
pub struct AttachmentUploadState {
    /// Filesystem path the user wants to upload.
    pub path: LineEditor,
    /// Item the attachment will be uploaded to.
    pub item_id: String,
    /// Item display name — surfaced in the popup header so the user
    /// can confirm they're attaching to the right item.
    pub item_name: String,
}

/// Opens the attachment-upload popup for the currently selected item.
/// No-op + error toast when there's nothing selected.
pub fn open_attachment_upload(app: &mut App) {
    let Some(item) = app.selected_item() else {
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

/// Cancels the upload, returns to the detail screen.
pub fn cancel_attachment_upload(app: &mut App) {
    app.attachment_upload = None;
    app.screen = Screen::Detail;
}

// ── Attachment download popup ─────────────────────────────────────────────

/// Buffer for the in-flight attachment-download popup.
#[derive(Debug, Clone)]
pub struct AttachmentDownloadState {
    /// Filesystem path the user wants to write the file to.
    pub path: LineEditor,
    /// Item the attachment belongs to.
    pub item_id: String,
    /// Item display name — surfaced in the popup header.
    pub item_name: String,
    /// Original `fileName` of the attachment — required by `bw get
    /// attachment` (it is the lookup key, not the attachment id).
    pub file_name: String,
}

/// Resolves the destination path for the download popup so the file
/// goes to `~/Downloads/<filename>` by default, suffixing with
/// `_1`, `_2`, … if a file already exists at that path.
///
/// Pure helper, returned as a `String` so the popup pre-fills it.
pub fn default_download_path(file_name: &str) -> String {
    let downloads = std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join("Downloads"))
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    unique_path(&downloads, file_name)
}

/// Returns `<dir>/<file_name>` if it does not exist, otherwise the
/// first available `<dir>/<stem>_<n><ext>` for `n = 1, 2, …`.
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

/// Splits a filename into `(stem, extension)` without a leading dot
/// on the extension. Treats files with no extension as `("name", "")`.
/// Files starting with a dot (`.bashrc`) are treated as all-stem.
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

/// Opens the attachment-download popup for the attachment at the
/// currently focused detail row. No-op + error toast when the focused
/// row is not an attachment.
pub fn open_attachment_download(app: &mut App) {
    let Some(item) = app.selected_item() else {
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

/// Cancels the download, returns to the detail screen.
pub fn cancel_attachment_download(app: &mut App) {
    app.attachment_download = None;
    app.screen = Screen::Detail;
}

/// Sends the attachment download request (worker). The popup state is
/// kept so the response handler can build the success toast.
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

/// `bw get attachment` response.
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

// ── Attachment delete confirm popup ───────────────────────────────────────

/// Buffer for the in-flight delete-attachment confirmation.
#[derive(Debug, Clone)]
pub struct AttachmentDeleteState {
    pub item_id: String,
    pub item_name: String,
    /// Stable Bitwarden attachment id — passed to `bw delete attachment`.
    pub attachment_id: String,
    /// `fileName` shown in the confirm dialog.
    pub file_name: String,
}

/// Opens the confirm-delete popup for the attachment at the focused
/// detail row. No-op + error toast otherwise.
pub fn open_confirm_delete_attachment(app: &mut App) {
    let Some(item) = app.selected_item() else {
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

/// Cancels the delete confirmation.
pub fn cancel_delete_attachment(app: &mut App) {
    app.attachment_delete = None;
    app.screen = Screen::Detail;
}

/// Sends the delete-attachment request (worker). The popup state is kept
/// so the refresh step / toast can use it.
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

/// `bw delete attachment` response — step 1. Chains a `get item` to
/// refresh the in-memory copy so the detail row count drops.
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
            // Chained best-effort refresh — keep the "Deleting…" toast.
            if app.begin(InFlight::DeleteAttachmentRefresh {
                item_id: item_id.clone(),
            }) {
                let _ = app.worker_tx.send(WorkerRequest::GetItemJson { item_id });
            }
        }
        Err(e) => app.cmd_err(&cmd, &e, "Delete failed"),
    }
}

/// `get item` refresh after an attachment delete — step 2.
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
    // The delete already succeeded; the refresh is best-effort. Whatever
    // happens, finish on the detail screen with the success toast.
    if let Ok(json) = r
        && let Ok(refreshed) = serde_json::from_str::<crate::domain::Item>(&json)
        && let Some(slot) = app.items.iter_mut().find(|i| i.id == item_id)
    {
        *slot = refreshed;
        app.rebuild_caches();
    }
    let count = app.detail_field_count();
    if count > 0 && app.detail_field >= count {
        app.detail_field = count - 1;
    }
    app.set_action(ActionState::Done(format!("Deleted \"{file_name}\" ✓")));
    app.attachment_delete = None;
    app.screen = Screen::Detail;
}

/// Sends the attachment upload request (worker). The popup state is kept
/// for the response handler.
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

/// `bw create attachment` response.
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
            if let Some(slot) = app.items.iter_mut().find(|i| i.id == item_id) {
                *slot = updated;
                app.rebuild_caches();
            }
            app.set_action(ActionState::Done(format!("Attached to \"{item_name}\" ✓")));
            app.attachment_upload = None;
            app.screen = Screen::Detail;
        }
        Err(e) => app.cmd_err(&cmd, &e, "Upload failed"),
    }
}

// ── Custom-field rename popup ─────────────────────────────────────────────

/// Buffer for the in-flight rename popup.
#[derive(Debug, Clone)]
pub struct RenameFieldState {
    /// New label being typed.
    pub input: LineEditor,
    /// Index of the edit-form row being renamed. The flow validates
    /// it is still a custom row at commit time, so a pending rename
    /// across an unrelated mutation simply no-ops.
    pub target_idx: usize,
}

impl RenameFieldState {
    /// Initialises the popup with the current label of the row being
    /// renamed pre-filled (cursor at end), so a quick edit doesn't
    /// require retyping the whole name.
    fn new(target_idx: usize, current: &str) -> Self {
        Self {
            input: LineEditor::with_text(current),
            target_idx,
        }
    }
}

/// Opens the rename popup for the focused custom field. No-op + error
/// toast when the focused row is not a custom field.
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

/// Commits the in-flight rename: copies the popup's input into the
/// target row's label, then closes the popup. Trims surrounding
/// whitespace; rejects the commit if the resulting label is empty.
pub fn commit_rename_field(app: &mut App) {
    let Some(state) = app.rename_field.take() else {
        return;
    };
    let new_label = state.input.text().trim().to_string();
    if new_label.is_empty() {
        // Re-open the popup with the same buffer so the user can fix
        // it instead of losing whatever they had typed.
        app.rename_field = Some(state);
        app.set_action(ActionState::Error("Field name cannot be empty.".into()));
        return;
    }
    // Reject collisions with sibling custom-field labels — the user
    // wouldn't be able to tell which is which on the detail screen.
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

/// Discards the in-flight rename and returns to the edit screen.
pub fn cancel_rename_field(app: &mut App) {
    app.rename_field = None;
    app.screen = Screen::Detail;
}

// ── Custom-field manipulation (edit mode) ─────────────────────────────────

/// Appends a new custom field at the end of the edit form and parks
/// focus on it so the user can start typing the value immediately.
///
/// The new row's label defaults to `Custom N` where `N` is one above
/// the highest existing `Custom <n>` label. Bw's `name` field on the
/// resulting record will be that label — labels are not currently
/// renameable from the TUI, so the user picks the type via Alt+T but
/// inherits the auto-generated name.
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

/// Removes the focused row when it is a custom field or a URI row
/// (in which case its sibling URL/URL-Match row is removed too via
/// [`remove_uri_row`]). Built-in schema rows cannot be removed and
/// produce an error toast.
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

// ── Multi-URI manipulation (edit mode, login items) ───────────────────────

/// Appends a new (URL, URL Match) pair to the edit form for items
/// that already have a login URIs section. The new URL gets the
/// next available slot index; focus parks on the new URL row.
///
/// No-op when the focused item is not a login (the form has no URI
/// rows in that case).
pub fn add_uri_row(app: &mut App) {
    if !app.edit.active {
        return;
    }
    use crate::tui::edit_field::{EditField, EditFieldKind};

    // Find the highest existing URI slot, then add 1.
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

    // Determine where to insert the new pair: right after the last
    // existing URI row (so they stay contiguous), or just before the
    // TOTP row if no URIs exist yet but a Login section is present.
    let last_uri_pos = app
        .edit
        .fields
        .iter()
        .rposition(|f| matches!(f.kind, EditFieldKind::Uri { .. }));
    let insert_at = last_uri_pos.map(|p| p + 1).unwrap_or_else(|| {
        // Place it just after Password if present, else at the end.
        app.edit
            .fields
            .iter()
            .position(|f| f.label == "Password")
            .map(|p| p + 1)
            .unwrap_or(app.edit.fields.len())
    });

    // Always emit indexed labels — adding a URI guarantees the form
    // has 2+ entries from now on, so the user sees the slot number.
    let url_label = format!("URL {}", next_idx + 1);
    let match_label = format!("URL {} Match", next_idx + 1);
    app.edit
        .fields
        .insert(insert_at, EditField::uri_url(&url_label, "", next_idx));
    app.edit.fields.insert(
        insert_at + 1,
        EditField::uri_match(&match_label, "", next_idx),
    );

    // Re-label the existing single-URI rows ("URL", "URL Match") to
    // their indexed form so the visual scheme is consistent.
    relabel_uris(app);

    app.edit.field_idx = insert_at;
    app.set_action(ActionState::Done(format!("Added URL {} ✓", next_idx + 1)));
}

/// Removes the URI pair (URL + URL Match rows) the focused row
/// belongs to. No-op when the focused row is not a URI row.
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

/// Renumbers the URI rows so their displayed indices stay contiguous
/// (1, 2, 3…) and match the positional slot a user sees, regardless
/// of any add / remove churn. Single-URI items collapse back to the
/// unsuffixed `"URL"` / `"URL Match"` labels.
fn relabel_uris(app: &mut App) {
    use crate::tui::edit_field::{EditFieldKind, UriRole};

    // Collect the URI row positions in form order, paired with their
    // role, so we can rewrite both labels and slot indices.
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

    // Group consecutive (Url, Match) pairs by visual order — count
    // the unique URL rows to decide whether to use suffixes.
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
            // Renumber the slot index too so the patcher emits a
            // tightly-packed `uris[]` (no gaps).
            field.kind = EditFieldKind::Uri {
                index: next_visual.saturating_sub(1),
                role,
            };
        }
    }
}

/// Cycles the focused custom field's type: text (0) → hidden (1) →
/// boolean (2) → text (0). No-op for built-in rows.
///
/// "Linked" custom fields (type 3) are not part of the cycle: bytewarden
/// has no UI yet to pick the target field, and converting a linked
/// field to anything else would silently drop the `linkedId` reference.
/// Linked fields created in the official Bitwarden GUI are preserved
/// read-only — Alt+T on one of them surfaces an explanatory toast and
/// leaves the type alone.
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

/// Save edit — step 1: fetch the item JSON to patch (worker).
pub fn queue_save_edit(app: &mut App) {
    let item_id = app.edit.item_id.clone();
    app.submit(
        InFlight::SaveEditFetch,
        "Saving…",
        WorkerRequest::GetItemJson { item_id },
    );
}

/// Save edit — step 1 response: patch the fetched JSON and commit it.
pub fn handle_save_edit_fetch(app: &mut App, r: Result<Zeroizing<String>, BwError>) {
    let item_id = app.edit.item_id.clone();
    let cmd = format!("bw edit item {item_id}");
    let base_json = match r {
        Ok(j) => j,
        Err(e) => return app.cmd_err(&cmd, &e, "Fetch failed"),
    };

    // Resolve the "Folder" row (which carries the folder *name* the user
    // typed) into an actual folder id before patching. Empty / unknown
    // name → null (no folder); the patcher is forgiving by design.
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
            clone.value = zeroize::Zeroizing::new(
                crate::tui::flows::folders::id_by_name(&folders_snapshot, &f.value)
                    .unwrap_or_default(),
            );
            clone
        })
        .collect();

    // The patched payload still carries plaintext credentials, so wrap
    // the intermediate buffer in `Zeroizing`.
    let patched = Zeroizing::new(patch_edit_payload(&base_json, &edit_fields_resolved));
    // Chained commit — keep the "Saving…" toast.
    if app.begin(InFlight::SaveEditCommit) {
        let _ = app.worker_tx.send(WorkerRequest::EditItem {
            item_id,
            json: patched,
        });
    }
}

/// Save edit — step 2 response: the committed item.
pub fn handle_save_edit_commit(app: &mut App, r: Result<crate::domain::Item, BwError>) {
    let item_id = app.edit.item_id.clone();
    let cmd = format!("bw edit item {item_id}");
    match r {
        Ok(updated) => {
            let name = updated.name.clone();
            if let Some(i) = app.items.iter_mut().find(|i| i.id == item_id) {
                *i = updated;
            }
            app.sort_items();
            app.push_cmd(&cmd, true, &format!("saved: {name}"));
            app.set_action(ActionState::Done("Saved ✓".into()));
            app.edit.active = false;
        }
        Err(e) => app.cmd_err(&cmd, &e, "Save failed"),
    }
}

// ── Delete / restore ──────────────────────────────────────────────────────

/// Opens the confirm-delete popup if there is an item selected.
pub fn open_confirm_delete(app: &mut App) {
    if app.selected_item().is_some() {
        app.screen = Screen::ConfirmDelete;
    }
}

/// Queues a delete action (worker).
pub fn queue_delete_item(app: &mut App, permanent: bool) {
    let Some(item) = app.selected_item() else {
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

/// `bw delete item` response.
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
            app.items.retain(|i| i.id != item_id);
            app.rebuild_caches();
            if app.selected_index >= app.items.len() && !app.items.is_empty() {
                app.selected_index = app.items.len() - 1;
            }
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
            // Refresh the trash list silently so the badge count updates;
            // the "Deleted ✓" toast above survives.
            if app.begin(InFlight::DeleteReloadTrash) {
                let _ = app.worker_tx.send(WorkerRequest::ListTrash);
            }
        }
        Err(e) => app.cmd_err(&cmd, &e, "Delete failed"),
    }
}

/// Silent post-delete trash reload.
pub fn handle_delete_reload_trash(app: &mut App, r: Result<Vec<crate::domain::Item>, BwError>) {
    match r {
        Ok(items) => {
            vault::set_trash(app, items);
        }
        Err(e) => app.push_cmd("bw list items --trash", false, &e),
    }
}

/// Queues a restore action for the selected (trashed) item (worker).
pub fn queue_restore_item(app: &mut App) {
    let Some(item) = app.selected_item() else {
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

/// `bw restore item` response.
pub fn handle_restore(app: &mut App, item_id: String, name: String, r: Result<(), BwError>) {
    let cmd = format!("bw restore item {item_id}");
    match r {
        Ok(()) => {
            app.trashed_items.retain(|i| i.id != item_id);
            app.rebuild_caches();
            app.push_cmd(&cmd, true, &format!("{name} restored to vault"));
            app.screen = Screen::Vault;
            app.active_filter = ItemFilter::All;
            app.filter_selected = 0;
            app.selected_index = 0;
            app.scroll_offset = 0;
            app.focus = Focus::Search;
            app.set_action(ActionState::Done("Restored ✓".into()));
            // Re-sync items silently so the restored entry is present; the
            // "Restored ✓" toast survives.
            if app.begin(InFlight::RestoreReloadItems) {
                let _ = app.worker_tx.send(WorkerRequest::ListItems);
            }
        }
        Err(e) => app.cmd_err(&cmd, &e, "Restore failed"),
    }
}

/// Silent post-restore item reload.
pub fn handle_restore_reload(app: &mut App, r: Result<Vec<crate::domain::Item>, BwError>) {
    match r {
        Ok(items) => vault::set_items(app, items),
        Err(e) => app.push_cmd("bw list items", false, &e),
    }
}

// ── Exposed (HaveIBeenPwned) ──────────────────────────────────────────────

/// Dispatches a HaveIBeenPwned check for the currently selected item to
/// the worker. No-op when the selection is empty or the item is not a
/// login (the backend would reject the request anyway, but failing fast
/// saves a round trip).
pub fn queue_check_exposed(app: &mut App) {
    let Some(item) = app.selected_item() else {
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

/// `bw get exposed` response. Reports the result as a coloured toast:
///
/// * `0` hits — green ✓ "Not in any known breach".
/// * `1+`     — error (red) "Found in N breaches — rotate this password".
pub fn handle_check_exposed(app: &mut App, r: Result<u32, BwError>) {
    let cmd = "bw get exposed".to_string();
    match r {
        Ok(0) => {
            app.push_cmd(&cmd, true, "0 breaches");
            app.set_action(ActionState::Done("Not in any known breach ✓".into()));
        }
        Ok(n) => {
            app.push_cmd(&cmd, true, &format!("{n} breaches"));
            // Surface as an Error so the strip uses the warning color
            // — semantically it is an action item for the user.
            app.set_action(ActionState::Error(format!(
                "⚠ Found in {n} breach{} — rotate this password",
                if n == 1 { "" } else { "es" }
            )));
        }
        Err(e) => app.cmd_err(&cmd, &e, "HIBP check failed"),
    }
}

// ── Favorite ──────────────────────────────────────────────────────────────

/// Favorite-toggle — step 1: fetch the item JSON (worker).
pub fn toggle_favorite(app: &mut App) {
    let Some(item) = app.selected_item() else {
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

/// Favorite-toggle — step 1 response: flip the `favorite` flag and
/// commit. The flip lives here (not on the port) because it's app-level
/// logic any backend would perform the same way.
pub fn handle_toggle_fetch(app: &mut App, item_id: String, r: Result<Zeroizing<String>, BwError>) {
    let cmd = format!("bw edit item {item_id}");
    let json = match r {
        Ok(j) => j,
        Err(e) => return app.cmd_err(&cmd, &e, "Fetch failed"),
    };
    let new_fav = match app.items.iter().find(|i| i.id == item_id) {
        Some(i) => !i.favorite,
        None => return, // item gone from under us
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
    // Chained commit — keep the "Updating…" toast.
    if app.begin(InFlight::ToggleFavoriteCommit {
        new_favorite: new_fav,
    }) {
        let _ = app.worker_tx.send(WorkerRequest::EditItem {
            item_id,
            json: new_json,
        });
    }
}

/// Favorite-toggle — step 2 response: apply the flipped flag.
pub fn handle_toggle_commit(
    app: &mut App,
    new_favorite: bool,
    r: Result<crate::domain::Item, BwError>,
) {
    let cmd = "bw edit item".to_string();
    match r {
        Ok(updated) => {
            if let Some(i) = app.items.iter_mut().find(|i| i.id == updated.id) {
                i.favorite = new_favorite;
            }
            // Favorite isn't a search field, but the Favorites filter
            // depends on the boolean, so rebuild the filtered cache.
            app.rebuild_filtered_cache();
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
        // Files starting with a dot (e.g. `.bashrc`) are configuration
        // files, not extensions — leave them alone.
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
        // SAFETY: env mutation is local to this single-threaded test
        // block and the value is restored when the temp dir drops.
        // The function only reads HOME, no other thread is involved.
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
