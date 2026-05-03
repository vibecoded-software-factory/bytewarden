//! [`App`] — the global state container for the TUI.
//!
//! `App` holds:
//!
//! * Navigation state (current screen, focus, scroll offsets, selection).
//! * Form-input state (login form, search box, edit/create field arrays).
//! * Cached domain data ([`Item`] vectors for the vault and the trash).
//! * The asynchronous-feeling action queue ([`PendingAction`]).
//! * The injected [`crate::ports`] trait objects.
//!
//! The struct is intentionally large but only ~30 cheap small-value
//! fields. Behaviour is implemented in [`crate::tui::flows`] and the
//! input/view layers; methods on `App` itself are deliberately limited
//! to thin getters/mutators.

use std::time::Instant;

use zeroize::Zeroizing;

use crate::domain::filter::{CreateItemType, ITEM_FILTERS, ItemFilter};
use crate::domain::folder::Folder;
use crate::domain::item::Item;
use crate::ports::{ClipboardPort, PasswordGeneratorPort, SettingsPort, VaultPort};
use crate::tui::action::{ActionState, CmdEntry, PendingAction};
use crate::tui::edit_field::EditField;
use crate::tui::generator::GeneratorState;
use crate::tui::mouse_areas::MouseAreas;
use crate::tui::screens::{Focus, LoginField, Screen};
use crate::tui::theme::{self, Theme};

/// Maximum number of entries kept in the command-log panel.
const CMD_LOG_LIMIT: usize = 50;

/// Per-page step size when paging through the vault list.
pub const PAGE_STEP: usize = 10;

/// Visible vault-list rows used to compute scroll behaviour.
pub const VAULT_VIEWPORT_ROWS: usize = 20;

/// Global TUI state.
pub struct App {
    // ── Screen / focus / filter ───────────────────────────────────────────
    pub screen: Screen,
    pub should_quit: bool,
    pub focus: Focus,
    pub active_filter: ItemFilter,
    pub filter_selected: usize,

    // ── Vault data ────────────────────────────────────────────────────────
    pub items: Vec<Item>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    /// Trashed items — fetched on demand when [`ItemFilter::Trash`] is
    /// selected.
    pub trashed_items: Vec<Item>,
    /// All folders visible in the current session (sorted alphabetically
    /// by name). Refreshed by [`crate::tui::flows::folders::load_folders`].
    pub folders: Vec<Folder>,
    /// Currently active folder filter (ANDed with `active_filter`).
    pub active_folder: crate::tui::folders::FolderFilter,
    /// Highlight index in the Folders sidebar panel.
    pub folder_selected: usize,

    // ── Login form ────────────────────────────────────────────────────────
    /// Current Bitwarden server URL — populated from `bw status` at
    /// boot and editable from the login screen.
    pub server_input: String,
    pub server_cursor: usize,
    /// Server URL as last persisted by `bw config server`. Used to
    /// decide whether the field is dirty and needs a re-config call.
    pub server_committed: String,
    pub email_input: String,
    pub email_cursor: usize,
    /// Master password buffer. Wrapped in [`Zeroizing`] so the bytes
    /// are overwritten when the field is cleared (or when `App` itself
    /// drops) — no plaintext copy lingers in the heap after login.
    pub password_input: Zeroizing<String>,
    pub password_cursor: usize,
    /// One-time-code buffer. Same zeroizing rationale as
    /// `password_input` — short-lived but worth scrubbing.
    pub otp_input: Zeroizing<String>,
    pub otp_cursor: usize,
    /// Toggled on after the backend reports a "new device" challenge.
    pub otp_required: bool,
    pub active_field: LoginField,
    pub login_error: bool,
    pub save_email: bool,
    /// Whether the unlocked session key should be persisted to a
    /// per-PPID runtime file (cleaned up when the parent shell dies).
    pub keep_session: bool,

    // ── Search ────────────────────────────────────────────────────────────
    pub search_query: String,

    // ── Detail / edit / create ────────────────────────────────────────────
    pub show_password: bool,
    pub detail_field: usize,
    /// Whether the master password on the login screen is shown in plain
    /// text.
    pub login_password_visible: bool,

    // ── Command log ───────────────────────────────────────────────────────
    pub cmd_log: Vec<CmdEntry>,
    pub cmd_log_scroll: usize,

    // ── Action queue ──────────────────────────────────────────────────────
    pub action_state: ActionState,
    pub action_tick: u8,
    pub pending_action: PendingAction,

    // ── Auto-lock ─────────────────────────────────────────────────────────
    pub auto_lock: bool,
    pub lock_after_secs: u64,
    pub last_activity: Instant,

    // ── Mouse hit-testing ─────────────────────────────────────────────────
    pub mouse_areas: MouseAreas,
    pub last_click: Option<(u16, u16)>,

    // ── Edit / create forms ───────────────────────────────────────────────
    pub edit_fields: Vec<EditField>,
    pub edit_field_idx: usize,
    pub edit_item_id: String,
    pub create_fields: Vec<EditField>,
    pub create_field_idx: usize,
    pub create_type: CreateItemType,
    pub create_type_idx: usize,
    pub create_choosing_type: bool,
    pub edit_mode: bool,

    // ── Generator state ───────────────────────────────────────────────────
    pub generator: GeneratorState,

    // ── Rename-field popup state ──────────────────────────────────────────
    /// Buffer for the in-flight custom-field rename. Carries the new
    /// label, the cursor position, and the index of the edit-form row
    /// being renamed. `None` outside the popup.
    pub rename_field: Option<crate::tui::flows::items::RenameFieldState>,

    // ── Folder name popup state (Create / Rename) ─────────────────────────
    /// Buffer for the in-flight folder-name popup. `None` outside the
    /// popup.
    pub folder_name: Option<crate::tui::flows::folders::FolderNameState>,

    // ── Export popup state ────────────────────────────────────────────────
    /// Buffer for the in-flight export popup. `None` outside the popup.
    pub export: Option<crate::tui::export::ExportState>,

    // ── Import popup state ────────────────────────────────────────────────
    /// Buffer for the in-flight import popup. `None` outside the popup.
    pub import: Option<crate::tui::import::ImportState>,

    // ── Attachment-upload popup state ─────────────────────────────────────
    /// Buffer for the in-flight attachment-upload popup.
    pub attachment_upload: Option<crate::tui::flows::items::AttachmentUploadState>,

    // ── Attachment-download popup state ───────────────────────────────────
    /// Buffer for the in-flight attachment-download popup.
    pub attachment_download: Option<crate::tui::flows::items::AttachmentDownloadState>,

    // ── Confirm-delete-attachment popup state ─────────────────────────────
    /// Buffer for the in-flight delete-attachment confirmation popup.
    pub attachment_delete: Option<crate::tui::flows::items::AttachmentDeleteState>,

    // ── Send-create popup state ───────────────────────────────────────────
    /// Buffer for the in-flight send-create popup.
    pub send_create: Option<crate::tui::send::SendCreateState>,

    // ── Memberships popup state ───────────────────────────────────────────
    /// Snapshot for the read-only memberships popup. `None` outside
    /// the popup.
    pub memberships: Option<crate::tui::flows::memberships::MembershipState>,

    // ── Help popup state ──────────────────────────────────────────────────
    /// Screen the user was on when they opened the help popup. The help
    /// renderer reads this to draw the correct background and to scope
    /// the shortcut list to the screen the user is actually looking at.
    /// `None` when help is not active.
    pub help_from: Option<Screen>,
    /// `(vertical, horizontal)` scroll offset for the help popup, in
    /// rows / columns. Reset to `(0, 0)` whenever the popup is opened.
    /// Clamped by the renderer once it knows the inner viewport size,
    /// so the input handler can increment freely without bookkeeping.
    pub help_scroll: (u16, u16),

    // ── Theme ─────────────────────────────────────────────────────────────
    pub theme: Theme,

    // ── Injected ports ────────────────────────────────────────────────────
    pub vault: Box<dyn VaultPort>,
    pub clipboard: Box<dyn ClipboardPort>,
    pub settings: Box<dyn SettingsPort>,
    pub generator_port: Box<dyn PasswordGeneratorPort>,
}

impl App {
    /// Constructs the initial state, reading user preferences via the
    /// settings port.
    pub fn new(
        vault: Box<dyn VaultPort>,
        clipboard: Box<dyn ClipboardPort>,
        settings: Box<dyn SettingsPort>,
        generator_port: Box<dyn PasswordGeneratorPort>,
    ) -> Self {
        let cfg = settings.read();
        let saved_email = cfg.email.clone().unwrap_or_default();
        let theme = theme::load(&settings.config_dir());
        Self {
            screen: Screen::Splash,
            should_quit: false,
            focus: Focus::Search,
            active_filter: ItemFilter::All,
            filter_selected: 0,
            items: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            trashed_items: Vec::new(),
            folders: Vec::new(),
            active_folder: crate::tui::folders::FolderFilter::All,
            folder_selected: 0,
            server_input: String::new(),
            server_cursor: 0,
            server_committed: String::new(),
            email_cursor: saved_email.chars().count(),
            email_input: saved_email,
            password_input: Zeroizing::new(String::new()),
            password_cursor: 0,
            otp_input: Zeroizing::new(String::new()),
            otp_cursor: 0,
            otp_required: false,
            active_field: if cfg.save_email {
                LoginField::Password
            } else {
                LoginField::Email
            },
            login_error: false,
            save_email: cfg.save_email,
            keep_session: cfg.keep_session,
            search_query: String::new(),
            show_password: false,
            detail_field: 0,
            login_password_visible: false,
            cmd_log: Vec::new(),
            cmd_log_scroll: 0,
            action_state: ActionState::Idle,
            action_tick: 0,
            pending_action: PendingAction::None,
            auto_lock: cfg.auto_lock,
            lock_after_secs: cfg.lock_after_secs,
            last_activity: Instant::now(),
            mouse_areas: MouseAreas::default(),
            last_click: None,
            edit_fields: Vec::new(),
            edit_field_idx: 0,
            edit_item_id: String::new(),
            create_fields: Vec::new(),
            create_field_idx: 0,
            create_type: CreateItemType::Login,
            create_type_idx: 0,
            create_choosing_type: true,
            edit_mode: false,
            generator: GeneratorState::default(),
            rename_field: None,
            folder_name: None,
            export: None,
            import: None,
            attachment_upload: None,
            attachment_download: None,
            attachment_delete: None,
            send_create: None,
            memberships: None,
            help_from: None,
            help_scroll: (0, 0),
            theme,
            vault,
            clipboard,
            settings,
            generator_port,
        }
    }

    // ── Activity / navigation ─────────────────────────────────────────────

    /// Records "user is active right now" — resets the auto-lock timer.
    pub fn reset_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn go_to_vault(&mut self) {
        self.screen = Screen::Vault;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.focus = Focus::Search;
    }

    pub fn go_to_detail(&mut self) {
        if !self.filtered_items().is_empty() {
            self.screen = Screen::Detail;
            self.show_password = false;
            self.detail_field = 0;
        }
    }

    pub fn go_back(&mut self) {
        match self.screen {
            Screen::Detail => {
                if self.edit_mode {
                    self.edit_mode = false;
                } else {
                    self.screen = Screen::Vault;
                }
            }
            Screen::Help => {
                // Closing help returns the user to whichever screen they
                // were on when they opened it — never silently teleport
                // them to the vault.
                self.screen = self.help_from.take().unwrap_or(Screen::Vault);
            }
            Screen::Create | Screen::ConfirmDelete => {
                self.screen = Screen::Vault;
            }
            _ => {}
        }
    }

    // ── Focus cycling ─────────────────────────────────────────────────────

    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Status | Focus::CmdLog => Focus::Search,
            Focus::Search => Focus::Folders,
            Focus::Folders => Focus::Items,
            Focus::Items => Focus::List,
            Focus::List => Focus::CmdLog,
        };
    }

    pub fn focus_panel(&mut self, n: u8) {
        self.focus = match n {
            0 => Focus::Status,
            1 => Focus::Folders,
            2 => Focus::Items,
            3 => Focus::List,
            4 => Focus::CmdLog,
            _ => return,
        };
    }

    // ── Filter / list movement ────────────────────────────────────────────

    pub fn filter_move_down(&mut self) {
        if self.filter_selected < ITEM_FILTERS.len() - 1 {
            self.filter_selected += 1;
        }
    }
    pub fn filter_move_up(&mut self) {
        if self.filter_selected > 0 {
            self.filter_selected -= 1;
        }
    }

    /// Activates the highlighted filter, queues a trash refresh if
    /// switching to [`ItemFilter::Trash`].
    pub fn apply_filter(&mut self) {
        self.active_filter = ITEM_FILTERS[self.filter_selected].clone();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.focus = Focus::List;
        if self.active_filter == ItemFilter::Trash {
            self.pending_action = PendingAction::LoadTrash;
        }
    }

    pub fn move_down(&mut self) {
        let len = self.filtered_items().len();
        if len > 0 && self.selected_index < len - 1 {
            self.selected_index += 1;
            if self.selected_index >= self.scroll_offset + VAULT_VIEWPORT_ROWS {
                self.scroll_offset += 1;
            }
        }
    }
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }
    pub fn move_down_page(&mut self) {
        for _ in 0..PAGE_STEP {
            self.move_down();
        }
    }
    pub fn move_up_page(&mut self) {
        for _ in 0..PAGE_STEP {
            self.move_up();
        }
    }

    // ── Vault data accessors ──────────────────────────────────────────────

    /// Returns references to the items that should currently be visible
    /// in the main list, after applying the active item-type filter,
    /// the active folder filter, and the search-box ranking.
    pub fn filtered_items(&self) -> Vec<&Item> {
        use crate::domain::search::fuzzy_score;

        let base: Vec<&Item> = if self.active_filter == ItemFilter::Trash {
            // Trash is a separate bucket — folder filter does not
            // apply (items in the trash often have lost their folder
            // context anyway).
            self.trashed_items.iter().collect()
        } else {
            self.items
                .iter()
                .filter(|item| {
                    self.active_filter.matches(item)
                        && self.active_folder.matches(item.folder_id.as_deref())
                })
                .collect()
        };

        if self.search_query.is_empty() {
            return base;
        }

        let query = self.search_query.to_lowercase();
        let mut scored: Vec<(i32, &Item)> = base
            .into_iter()
            .filter_map(|item| {
                let s = fuzzy_score(item, &query);
                if s > 0 { Some((s, item)) } else { None }
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, i)| i).collect()
    }

    pub fn selected_item(&self) -> Option<&Item> {
        self.filtered_items().get(self.selected_index).copied()
    }

    /// Item count for a given filter — used to render sidebar badges.
    pub fn count_for(&self, filter: &ItemFilter) -> usize {
        match filter {
            ItemFilter::All => self.items.len(),
            ItemFilter::Favorites => self.items.iter().filter(|i| i.favorite).count(),
            ItemFilter::Trash => self.trashed_items.len(),
            f => self
                .items
                .iter()
                .filter(|i| f.type_id() == Some(i.item_type))
                .count(),
        }
    }

    pub fn perform_search(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.focus = Focus::List;
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    // ── Trash mode ────────────────────────────────────────────────────────

    /// Shorthand — `true` when the active filter is [`ItemFilter::Trash`].
    pub fn is_trash_view(&self) -> bool {
        self.active_filter == ItemFilter::Trash
    }

    // ── Command log + action state ────────────────────────────────────────

    /// Appends a redacted command + its result to the log. Caps total
    /// entries at [`CMD_LOG_LIMIT`].
    ///
    /// When `BYTEWARDEN_DEBUG=1` is set the same redacted line is also
    /// appended to `~/.bytewarden.log` for offline troubleshooting —
    /// see [`crate::tui::debug_log`]. The check is cheap when the env
    /// var is unset, so leaving it off costs nothing.
    pub fn push_cmd(&mut self, cmd: &str, ok: bool, detail: &str) {
        let session_marker = self
            .vault
            .session_key()
            .unwrap_or("__NO_SESSION__")
            .to_string();
        let redacted = cmd.replace(&session_marker, "***");
        crate::tui::debug_log::append(&redacted, ok, detail);
        self.cmd_log.push(CmdEntry {
            cmd: redacted,
            ok,
            detail: detail.to_string(),
        });
        if self.cmd_log.len() > CMD_LOG_LIMIT {
            self.cmd_log.remove(0);
        }
        self.cmd_log_scroll = 0;
    }

    pub fn cmd_log_scroll_up(&mut self, n: usize) {
        let max = self.cmd_log.len().saturating_sub(1);
        self.cmd_log_scroll = (self.cmd_log_scroll + n).min(max);
    }
    pub fn cmd_log_scroll_down(&mut self, n: usize) {
        self.cmd_log_scroll = self.cmd_log_scroll.saturating_sub(n);
    }

    pub fn set_action(&mut self, state: ActionState) {
        self.action_state = state;
        self.action_tick = 0;
    }
    pub fn tick_action(&mut self) {
        self.action_tick = self.action_tick.wrapping_add(1);
    }

    /// Logs a failed `bw` command and surfaces the error in the feedback
    /// strip.
    pub fn cmd_err(&mut self, cmd: &str, e: &str, label: &str) {
        self.push_cmd(cmd, false, e);
        self.set_action(ActionState::Error(format!("{label}: {e}")));
    }

    // ── Login error helpers ───────────────────────────────────────────────

    pub fn set_login_error(&mut self) {
        self.login_error = true;
        self.password_input.clear();
        self.password_cursor = 0;
        self.otp_input.clear();
        self.otp_cursor = 0;
        self.otp_required = false;
        self.active_field = LoginField::Password;
    }

    pub fn clear_login_error(&mut self) {
        self.login_error = false;
    }

    // ── Edit / create field accessors ─────────────────────────────────────

    pub fn edit_field_mut(&mut self) -> Option<&mut EditField> {
        self.edit_fields.get_mut(self.edit_field_idx)
    }

    pub fn create_field_mut(&mut self) -> Option<&mut EditField> {
        self.create_fields.get_mut(self.create_field_idx)
    }

    /// Toggles the reveal flag on the focused (hidden) edit field.
    pub fn edit_toggle_reveal(&mut self) {
        if let Some(f) = self.edit_field_mut()
            && f.hidden
        {
            f.revealed = !f.revealed;
        }
    }

    // ── Login text field plumbing ─────────────────────────────────────────

    /// Returns mutable refs to the (input, cursor) pair for the focused
    /// login text field, or `None` for checkboxes.
    ///
    /// The password and OTP fields go through a `&mut *…` deref so the
    /// caller sees a plain `&mut String` regardless of whether the
    /// underlying buffer is wrapped in `Zeroizing` — keeps the input
    /// helpers (`insert_char`, `delete_char_*`, …) generic.
    pub fn login_text_mut(&mut self) -> Option<(&mut String, &mut usize)> {
        match self.active_field {
            LoginField::Server => Some((&mut self.server_input, &mut self.server_cursor)),
            LoginField::Email => Some((&mut self.email_input, &mut self.email_cursor)),
            LoginField::Password => Some((&mut *self.password_input, &mut self.password_cursor)),
            LoginField::Otp => Some((&mut *self.otp_input, &mut self.otp_cursor)),
            _ => None,
        }
    }

    /// Length (in characters) of the focused login text field.
    pub fn login_text_len(&self) -> usize {
        match self.active_field {
            LoginField::Server => self.server_input.chars().count(),
            LoginField::Email => self.email_input.chars().count(),
            LoginField::Password => self.password_input.chars().count(),
            LoginField::Otp => self.otp_input.chars().count(),
            _ => 0,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        let save = self.active_field == LoginField::Email && self.save_email;
        if let Some((input, cursor)) = self.login_text_mut() {
            let byte = input
                .char_indices()
                .nth(*cursor)
                .map(|(b, _)| b)
                .unwrap_or(input.len());
            input.insert(byte, c);
            *cursor += 1;
        }
        if save {
            let e = self.email_input.clone();
            self.settings.write(true, Some(&e));
        }
    }

    pub fn delete_char_before(&mut self) {
        let save = self.active_field == LoginField::Email && self.save_email;
        if let Some((input, cursor)) = self.login_text_mut()
            && *cursor > 0
        {
            let byte = input
                .char_indices()
                .nth(*cursor - 1)
                .map(|(b, _)| b)
                .unwrap_or(0);
            input.remove(byte);
            *cursor -= 1;
        }
        if save {
            let e = self.email_input.clone();
            self.settings.write(true, Some(&e));
        }
    }

    pub fn delete_char_at(&mut self) {
        let save = self.active_field == LoginField::Email && self.save_email;
        if let Some((input, cursor)) = self.login_text_mut()
            && *cursor < input.chars().count()
        {
            let byte = input
                .char_indices()
                .nth(*cursor)
                .map(|(b, _)| b)
                .unwrap_or(0);
            input.remove(byte);
        }
        if save {
            let e = self.email_input.clone();
            self.settings.write(true, Some(&e));
        }
    }

    pub fn cursor_left(&mut self) {
        if let Some((_, cursor)) = self.login_text_mut()
            && *cursor > 0
        {
            *cursor -= 1;
        }
    }
    pub fn cursor_right(&mut self) {
        let len = self.login_text_len();
        if let Some((_, cursor)) = self.login_text_mut()
            && *cursor < len
        {
            *cursor += 1;
        }
    }
    pub fn cursor_home(&mut self) {
        if let Some((_, cursor)) = self.login_text_mut() {
            *cursor = 0;
        }
    }
    pub fn cursor_end(&mut self) {
        let len = self.login_text_len();
        if let Some((_, cursor)) = self.login_text_mut() {
            *cursor = len;
        }
    }

    pub fn toggle_save_email(&mut self) {
        self.save_email = !self.save_email;
        if self.save_email {
            let e = self.email_input.clone();
            self.settings.write(true, Some(&e));
        } else {
            self.settings.write(false, None);
        }
    }

    /// Flips `keep_session`, persists the new value, and immediately
    /// clears any on-disk session file when turning the option off so
    /// the user's choice takes effect right away (instead of waiting
    /// for the parent shell to die).
    pub fn toggle_keep_session(&mut self) {
        self.keep_session = !self.keep_session;
        self.settings.write_keep_session(self.keep_session);
        if !self.keep_session {
            crate::tui::session_file::clear();
        }
    }

    /// Number of detail-screen rows for the currently selected item.
    ///
    /// Delegates to the shared [`crate::tui::detail_fields`] builder so
    /// the count never diverges from what the renderer actually shows.
    pub fn detail_field_count(&self) -> usize {
        let Some(item) = self.selected_item() else {
            return 0;
        };
        crate::tui::detail_fields::build_detail_fields(item, false, 0).len()
    }

    /// Sorts the in-memory vault list alphabetically (case-insensitive).
    pub fn sort_items(&mut self) {
        self.items
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    }

    /// Returns the current session key for log redaction (or `***` when
    /// none).
    pub fn session_key_display(&self) -> String {
        self.vault.session_key().unwrap_or("***").to_string()
    }
}
