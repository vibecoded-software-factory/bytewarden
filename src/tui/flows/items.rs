//! Create / edit / delete / restore / favorite flows.

use serde_json::Value;

use crate::domain::filter::{CREATE_ITEM_TYPES, CreateItemType, ItemFilter};
use crate::tui::action::{ActionState, PendingAction};
use crate::tui::app::App;
use crate::tui::detail_fields::build_detail_fields;
use crate::tui::edit_field::{build_create_fields_with_orgs, build_edit_fields_with_folders};
use crate::tui::flows::item_json::{build_create_payload, patch_edit_payload};
use crate::tui::flows::vault;
use crate::tui::screens::{Focus, Screen};

// ── Create ────────────────────────────────────────────────────────────────

/// Opens the "create item" screen on the type-picker step.
pub fn open_create(app: &mut App) {
    app.create_type_idx = 0;
    app.create_type = CreateItemType::Login;
    app.create_choosing_type = true;
    app.create_fields = Vec::new();
    app.create_field_idx = 0;
    app.screen = Screen::Create;
}

/// Confirms the type-picker selection and renders the matching form.
pub fn create_select_type(app: &mut App) {
    app.create_type = CREATE_ITEM_TYPES[app.create_type_idx].clone();
    app.create_fields = build_create_fields_with_orgs(&app.create_type, &app.organizations);
    app.create_field_idx = 0;
    app.create_choosing_type = false;
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
    let Some(field) = app.create_fields.get(app.create_field_idx) else {
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
    if let Some(f) = app.create_fields.get_mut(app.create_field_idx) {
        f.value = zeroize::Zeroizing::new(new_display);
        f.cursor = f.value.chars().count();
        f.organization_id = new_id.clone();
    }
    // Sync the sibling Collections row.
    let org_idx = app.create_field_idx;
    let coll_pos = app.create_fields.iter().position(|f| f.is_collections());
    match (coll_pos, new_id.as_ref()) {
        (Some(pos), None) => {
            // Going Personal → drop the row.
            app.create_fields.remove(pos);
        }
        (Some(pos), Some(_)) => {
            // Switched org → reset the row (user must reselect).
            if let Some(f) = app.create_fields.get_mut(pos) {
                f.value = zeroize::Zeroizing::new(String::new());
                f.cursor = 0;
                f.collection_ids = Vec::new();
            }
        }
        (None, Some(_)) => {
            // Personal → real org. Insert a Collections row right
            // after the Organization row so the form layout stays
            // grouped.
            app.create_fields.insert(
                org_idx + 1,
                crate::tui::edit_field::EditField::collections("", Vec::new()),
            );
        }
        (None, None) => {} // Both absent — nothing to do.
    }
}

/// Validates and queues a [`PendingAction::CreateItem`].
pub fn queue_create_item(app: &mut App) {
    let name = app
        .create_fields
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
        .create_fields
        .iter()
        .find(|f| f.is_organization())
        .and_then(|f| f.organization_id.clone());
    if org_set.is_some() {
        let coll_count = app
            .create_fields
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
    app.set_action(ActionState::Running("Creating…".into()));
    app.pending_action = PendingAction::CreateItem;
}

/// Pending-action executor for [`PendingAction::CreateItem`].
pub fn do_create_item(app: &mut App) {
    let json = build_create_payload(&app.create_type, &app.create_fields);
    let cmd = "bw create item".to_string();
    match app.vault.create_item(&json) {
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

    app.edit_item_id = item.id.clone();
    app.edit_fields = fields;
    app.edit_field_idx = initial_idx;
    app.edit_mode = true;
}

// ── Attachment upload popup ───────────────────────────────────────────────

/// Buffer for the in-flight attachment-upload popup.
#[derive(Debug, Clone)]
pub struct AttachmentUploadState {
    /// Filesystem path the user wants to upload.
    pub path: String,
    /// Cursor in the path field (character index).
    pub path_cursor: usize,
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
        path: String::new(),
        path_cursor: 0,
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
    pub path: String,
    /// Cursor in the path field (character index).
    pub path_cursor: usize,
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
        path_cursor: path.chars().count(),
        path,
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

/// Queues the download — the dispatcher runs the blocking call after
/// rendering the spinner.
pub fn queue_attachment_download(app: &mut App) {
    let Some(state) = app.attachment_download.as_ref() else {
        return;
    };
    if state.path.trim().is_empty() {
        app.set_action(ActionState::Error("Output path cannot be empty.".into()));
        return;
    }
    app.set_action(ActionState::Running("Downloading…".into()));
    app.pending_action = PendingAction::DownloadAttachment;
}

/// Performs the download via `bw get attachment`.
pub fn do_download_attachment(app: &mut App) {
    let Some(state) = app.attachment_download.as_ref() else {
        return;
    };
    let item_id = state.item_id.clone();
    let item_name = state.item_name.clone();
    let file_name = state.file_name.clone();
    let path = state.path.trim().to_string();
    let cmd = format!("bw get attachment {file_name} --itemid {item_id} --output <path>");
    match app.vault.download_attachment(&item_id, &file_name, &path) {
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

/// Queues the delete — the dispatcher runs the blocking call after
/// rendering the spinner.
pub fn queue_delete_attachment(app: &mut App) {
    if app.attachment_delete.is_none() {
        return;
    }
    app.set_action(ActionState::Running("Deleting attachment…".into()));
    app.pending_action = PendingAction::DeleteAttachment;
}

/// Performs the delete via `bw delete attachment` and refreshes the
/// item from the server so the in-memory copy drops the gone row.
pub fn do_delete_attachment(app: &mut App) {
    let Some(state) = app.attachment_delete.as_ref() else {
        return;
    };
    let item_id = state.item_id.clone();
    let item_name = state.item_name.clone();
    let attachment_id = state.attachment_id.clone();
    let file_name = state.file_name.clone();
    let cmd = format!("bw delete attachment {attachment_id} --itemid {item_id}");
    match app.vault.delete_attachment(&item_id, &attachment_id) {
        Ok(()) => {
            app.push_cmd(&cmd, true, &format!("deleted from {item_name}"));
            // bw doesn't return the updated item from `delete attachment` —
            // refresh it via `get item` so the detail row count drops.
            if let Ok(json) = app.vault.get_item_json(&item_id)
                && let Ok(refreshed) = serde_json::from_str::<crate::domain::Item>(&json)
                && let Some(slot) = app.items.iter_mut().find(|i| i.id == item_id)
            {
                *slot = refreshed;
                app.rebuild_caches();
            }
            // Clamp the focused row in case it pointed at the deleted
            // attachment, otherwise the detail screen would highlight
            // something past the new end of the list.
            let count = app.detail_field_count();
            if count > 0 && app.detail_field >= count {
                app.detail_field = count - 1;
            }
            app.set_action(ActionState::Done(format!("Deleted \"{file_name}\" ✓")));
            app.attachment_delete = None;
            app.screen = Screen::Detail;
        }
        Err(e) => app.cmd_err(&cmd, &e, "Delete failed"),
    }
}

/// Performs the upload via `bw create attachment`. On success the
/// item record in `app.items` is replaced with the freshly returned
/// one (which now carries the new attachment).
pub fn commit_attachment_upload(app: &mut App) {
    let Some(state) = app.attachment_upload.as_ref() else {
        return;
    };
    let path = state.path.trim().to_string();
    if path.is_empty() {
        app.set_action(ActionState::Error("File path cannot be empty.".into()));
        return;
    }
    let item_id = state.item_id.clone();
    let item_name = state.item_name.clone();
    let cmd = format!("bw create attachment --file <path> --itemid {item_id}");
    app.set_action(ActionState::Running("Uploading…".into()));
    match app.vault.upload_attachment(&item_id, &path) {
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
    pub input: String,
    /// Cursor position as a *character* index.
    pub cursor: usize,
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
            input: current.to_string(),
            cursor: current.chars().count(),
            target_idx,
        }
    }
}

/// Opens the rename popup for the focused custom field. No-op + error
/// toast when the focused row is not a custom field.
pub fn open_rename_field(app: &mut App) {
    if !app.edit_mode {
        return;
    }
    let Some(field) = app.edit_fields.get(app.edit_field_idx) else {
        return;
    };
    if !field.is_custom() {
        app.set_action(ActionState::Error(
            "Only custom fields can be renamed.".into(),
        ));
        return;
    }
    app.rename_field = Some(RenameFieldState::new(app.edit_field_idx, &field.label));
    app.screen = Screen::RenameField;
}

/// Commits the in-flight rename: copies the popup's input into the
/// target row's label, then closes the popup. Trims surrounding
/// whitespace; rejects the commit if the resulting label is empty.
pub fn commit_rename_field(app: &mut App) {
    let Some(state) = app.rename_field.take() else {
        return;
    };
    let new_label = state.input.trim().to_string();
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
        .edit_fields
        .get(state.target_idx)
        .filter(|f| f.is_custom())
        .map(|f| f.label.clone());
    let siblings: Vec<&str> = app
        .edit_fields
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
    if let Some(field) = app.edit_fields.get_mut(state.target_idx) {
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
    if !app.edit_mode {
        return;
    }
    let next_n = app
        .edit_fields
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
    app.edit_fields.push(new_field);
    app.edit_field_idx = app.edit_fields.len() - 1;
    app.set_action(ActionState::Done(format!("Added {label} ✓")));
}

/// Removes the focused row when it is a custom field or a URI row
/// (in which case its sibling URL/URL-Match row is removed too via
/// [`remove_uri_row`]). Built-in schema rows cannot be removed and
/// produce an error toast.
pub fn remove_current_field(app: &mut App) {
    if !app.edit_mode {
        return;
    }
    let Some(field) = app.edit_fields.get(app.edit_field_idx) else {
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
    app.edit_fields.remove(app.edit_field_idx);
    if app.edit_field_idx >= app.edit_fields.len() && !app.edit_fields.is_empty() {
        app.edit_field_idx = app.edit_fields.len() - 1;
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
    if !app.edit_mode {
        return;
    }
    use crate::tui::edit_field::{EditField, EditFieldKind};

    // Find the highest existing URI slot, then add 1.
    let next_idx = app
        .edit_fields
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
        .edit_fields
        .iter()
        .rposition(|f| matches!(f.kind, EditFieldKind::Uri { .. }));
    let insert_at = last_uri_pos.map(|p| p + 1).unwrap_or_else(|| {
        // Place it just after Password if present, else at the end.
        app.edit_fields
            .iter()
            .position(|f| f.label == "Password")
            .map(|p| p + 1)
            .unwrap_or(app.edit_fields.len())
    });

    // Always emit indexed labels — adding a URI guarantees the form
    // has 2+ entries from now on, so the user sees the slot number.
    let url_label = format!("URL {}", next_idx + 1);
    let match_label = format!("URL {} Match", next_idx + 1);
    app.edit_fields
        .insert(insert_at, EditField::uri_url(&url_label, "", next_idx));
    app.edit_fields.insert(
        insert_at + 1,
        EditField::uri_match(&match_label, "", next_idx),
    );

    // Re-label the existing single-URI rows ("URL", "URL Match") to
    // their indexed form so the visual scheme is consistent.
    relabel_uris(app);

    app.edit_field_idx = insert_at;
    app.set_action(ActionState::Done(format!("Added URL {} ✓", next_idx + 1)));
}

/// Removes the URI pair (URL + URL Match rows) the focused row
/// belongs to. No-op when the focused row is not a URI row.
pub fn remove_uri_row(app: &mut App) {
    if !app.edit_mode {
        return;
    }
    use crate::tui::edit_field::EditFieldKind;

    let Some(focused) = app.edit_fields.get(app.edit_field_idx) else {
        return;
    };
    let target_index = match focused.kind {
        EditFieldKind::Uri { index, .. } => index,
        _ => {
            app.set_action(ActionState::Error("Focused row is not a URL.".into()));
            return;
        }
    };

    app.edit_fields
        .retain(|f| !matches!(f.kind, EditFieldKind::Uri { index, .. } if index == target_index));
    if app.edit_field_idx >= app.edit_fields.len() && !app.edit_fields.is_empty() {
        app.edit_field_idx = app.edit_fields.len() - 1;
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
        .edit_fields
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
        if let Some(field) = app.edit_fields.get_mut(pos) {
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
    if !app.edit_mode {
        return;
    }
    let Some(field) = app.edit_fields.get_mut(app.edit_field_idx) else {
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

/// Queues a [`PendingAction::SaveEdit`].
pub fn queue_save_edit(app: &mut App) {
    app.set_action(ActionState::Running("Saving…".into()));
    app.pending_action = PendingAction::SaveEdit;
}

/// Pending-action executor for [`PendingAction::SaveEdit`].
pub fn do_save_edit(app: &mut App) {
    let item_id = app.edit_item_id.clone();
    let cmd = format!("bw edit item {item_id}");
    let base_json = match app.vault.get_item_json(&item_id) {
        Ok(j) => j,
        Err(e) => {
            app.cmd_err(&cmd, &e, "Fetch failed");
            return;
        }
    };

    // Resolve the "Folder" row (which carries the folder *name* the
    // user typed) into an actual folder id before patching. Empty or
    // unrecognised name → null (no folder). The user gets no error
    // for a typo because the patcher is forgiving by design — bw
    // accepts any UUID and the item just shows under "(No folder)"
    // if it doesn't match.
    let folders_snapshot = app.folders.clone();
    let edit_fields_resolved: Vec<crate::tui::edit_field::EditField> = app
        .edit_fields
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

    // The patched payload still carries plaintext credentials (the
    // password / TOTP / key bytes the user just edited), so wrap the
    // intermediate buffer in `Zeroizing` — it is freed with zeros once
    // `edit_item` returns.
    let patched = zeroize::Zeroizing::new(patch_edit_payload(&base_json, &edit_fields_resolved));
    match app.vault.edit_item(&item_id, &patched) {
        Ok(updated) => {
            let name = updated.name.clone();
            if let Some(i) = app.items.iter_mut().find(|i| i.id == item_id) {
                *i = updated;
            }
            // sort_items rebuilds the caches; no need for an explicit
            // call here.
            app.sort_items();
            app.push_cmd(&cmd, true, &format!("saved: {name}"));
            app.set_action(ActionState::Done("Saved ✓".into()));
            app.edit_mode = false;
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

/// Queues a delete action.
pub fn queue_delete_item(app: &mut App, permanent: bool) {
    app.set_action(ActionState::Running(
        if permanent {
            "Deleting…"
        } else {
            "Trashing…"
        }
        .into(),
    ));
    app.pending_action = PendingAction::DeleteItem { permanent };
    app.screen = Screen::Vault;
}

/// Pending-action executor for [`PendingAction::DeleteItem`].
pub fn do_delete_item(app: &mut App, permanent: bool) {
    let Some(item) = app.selected_item() else {
        return;
    };
    let (id, name) = (item.id.clone(), item.name.clone());
    let perm_str = if permanent { " --permanent" } else { "" };
    let cmd = format!("bw delete item {id}{perm_str}");
    match app.vault.delete_item(&id, permanent) {
        Ok(()) => {
            app.items.retain(|i| i.id != id);
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
            // Refresh the trash list silently so the badge count updates,
            // but the success toast set below is what the user sees.
            vault::refresh_trash_silent(app);
            app.set_action(ActionState::Done(
                if permanent {
                    "Deleted ✓"
                } else {
                    "Trashed ✓"
                }
                .into(),
            ));
        }
        Err(e) => app.cmd_err(&cmd, &e, "Delete failed"),
    }
}

/// Queues a restore action for the selected (trashed) item.
pub fn queue_restore_item(app: &mut App) {
    if app.selected_item().is_some() {
        app.set_action(ActionState::Running("Restoring…".into()));
        app.pending_action = PendingAction::RestoreItem;
    }
}

/// Pending-action executor for [`PendingAction::RestoreItem`].
pub fn do_restore_item(app: &mut App) {
    let Some(item) = app.selected_item() else {
        return;
    };
    let (id, name) = (item.id.clone(), item.name.clone());
    let cmd = format!("bw restore item {id}");
    match app.vault.restore_item(&id) {
        Ok(()) => {
            app.trashed_items.retain(|i| i.id != id);
            app.rebuild_caches();
            app.push_cmd(&cmd, true, &format!("{name} restored to vault"));
            // Bring the user back to the regular vault list.
            app.screen = Screen::Vault;
            app.active_filter = ItemFilter::All;
            app.filter_selected = 0;
            app.selected_index = 0;
            app.scroll_offset = 0;
            app.focus = Focus::Search;
            // Re-sync the items silently so the restored entry is
            // present in the list, but keep the "Restored ✓" toast.
            vault::refresh_items_silent(app);
            app.set_action(ActionState::Done("Restored ✓".into()));
        }
        Err(e) => app.cmd_err(&cmd, &e, "Restore failed"),
    }
}

// ── Exposed (HaveIBeenPwned) ──────────────────────────────────────────────

/// Queues a [`PendingAction::CheckExposed`] for the currently selected
/// item. No-op when the selection is empty or the item is not a login
/// (the backend would reject the request anyway, but failing fast saves
/// a round trip).
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
    app.set_action(ActionState::Running("Checking HIBP…".into()));
    app.pending_action = PendingAction::CheckExposed(id);
}

/// Pending-action executor for [`PendingAction::CheckExposed`].
///
/// Calls `bw get exposed` and reports the result as a coloured toast:
///
/// * `0` hits — green ✓ "Not in any known breach".
/// * `1+`     — error (red) "Found in N breaches — rotate this password".
pub fn do_check_exposed(app: &mut App, item_id: String) {
    let cmd = format!("bw get exposed {item_id}");
    match app.vault.check_exposed(&item_id) {
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

/// Queues a favorite-toggle.
pub fn toggle_favorite(app: &mut App) {
    if app.selected_item().is_some() {
        app.set_action(ActionState::Running("Updating…".into()));
        app.pending_action = PendingAction::ToggleFavorite;
    }
}

/// Pending-action executor for [`PendingAction::ToggleFavorite`].
///
/// Fetches the existing JSON, flips the `favorite` boolean, and re-edits
/// the item. The flip lives here (not on the port) because it is
/// app-level logic that any vault backend would have to perform the
/// same way.
pub fn do_toggle_favorite(app: &mut App) {
    let Some(item) = app.selected_item() else {
        return;
    };
    let (id, name, new_fav) = (item.id.clone(), item.name.clone(), !item.favorite);
    let cmd = format!("bw edit item {id}");

    let json = match app.vault.get_item_json(&id) {
        Ok(j) => j,
        Err(e) => {
            app.cmd_err(&cmd, &e, "Fetch failed");
            return;
        }
    };
    let mut val: Value = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(e) => {
            app.cmd_err(&cmd, &format!("JSON parse error: {e}"), "Failed");
            return;
        }
    };
    val["favorite"] = Value::Bool(new_fav);
    // Same hygiene as `do_save_edit`: the serialized payload still
    // carries the item's secrets — wrap it before handing off to
    // `edit_item` so the buffer is zeroed on drop. The `serde_json::
    // Value` parsed above is short-lived and not directly reachable
    // outside this function.
    let new_json = match serde_json::to_string(&val) {
        Ok(s) => zeroize::Zeroizing::new(s),
        Err(e) => {
            app.cmd_err(&cmd, &format!("JSON serialize error: {e}"), "Failed");
            return;
        }
    };

    match app.vault.edit_item(&id, &new_json) {
        Ok(_) => {
            if let Some(i) = app.items.iter_mut().find(|i| i.id == id) {
                i.favorite = new_fav;
            }
            // The fuzzy-search lowered cache doesn't change on a
            // favorite flip (favorite isn't a search field), but the
            // Favorites filter relies on the boolean to decide
            // membership, so the filtered cache must be rebuilt.
            app.rebuild_filtered_cache();
            let label = if new_fav {
                "★ Favorited"
            } else {
                "Unfavorited"
            };
            app.set_action(ActionState::Done(label.into()));
            app.push_cmd(&cmd, true, &format!("{name} {label}"));
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
