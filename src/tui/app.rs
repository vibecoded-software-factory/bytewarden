//! [`App`] — the global state container for the TUI.
//!
//! `App` holds:
//!
//! * Navigation state (current screen, focus, scroll offsets, selection).
//! * Form-input state (login form, search box, edit/create field arrays).
//! * Cached domain data ([`Item`] vectors for the vault and the trash).
//! * The worker channels + the in-flight ticket ([`crate::tui::worker`]).
//! * The injected synchronous ports (clipboard, settings).
//!
//! The struct is intentionally large but only ~30 cheap small-value
//! fields. Behaviour is implemented in [`crate::tui::flows`] and the
//! input/view layers; methods on `App` itself are deliberately limited
//! to thin getters/mutators.

use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use zeroize::Zeroizing;

use crate::domain::LoweredItem;
use crate::domain::filter::{CreateItemType, ITEM_FILTERS, ItemFilter};
use crate::domain::folder::Folder;
use crate::domain::item::Item;
use crate::ports::{ClipboardPort, SettingsPort};
use crate::tui::action::{ActionState, CmdEntry};
use crate::tui::edit_field::EditField;
use crate::tui::generator::GeneratorState;
use crate::tui::mouse_areas::MouseAreas;
use crate::tui::screens::{Focus, LoginField, Screen};
use crate::tui::theme::{self, Theme};
use crate::tui::worker::{InFlight, WorkerRequest, WorkerResponse};

/// Maximum number of entries kept in the command-log panel.
const CMD_LOG_LIMIT: usize = 50;

/// Per-page step size when paging through the vault list.
pub const PAGE_STEP: usize = 10;

/// Visible vault-list rows used to compute scroll behaviour.
pub const VAULT_VIEWPORT_ROWS: usize = 20;

/// Redacts a cached session key from a command string before it's logged.
/// The `bw` argv never carries the key (it's passed via env), so this is
/// defense-in-depth. Pure helper so the redaction is unit-testable
/// without constructing an [`App`].
pub(crate) fn redact_cmd(cmd: &str, marker: Option<&str>) -> String {
    match marker {
        Some(key) if !key.is_empty() => cmd.replace(key, "***"),
        _ => cmd.to_string(),
    }
}

/// Which pane of the Settings overlay currently holds focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsFocus {
    /// The left-hand list of sections.
    Sidebar,
    /// The right-hand panel showing the active section's options.
    Panel,
}

/// A section of the Settings overlay. Sectioned so the preferences
/// surface can grow (Security, Clipboard…) without changing the layout.
/// Today only [`SettingsSection::Theme`] exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Theme,
}

impl SettingsSection {
    /// Every section, in sidebar order.
    pub const ALL: [SettingsSection; 1] = [SettingsSection::Theme];

    /// The sidebar label.
    pub fn label(self) -> &'static str {
        match self {
            SettingsSection::Theme => "Theme",
        }
    }
}

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

    /// Pre-lowercased projection of [`Self::items`], kept parallel and
    /// the same length. Refreshed by [`Self::rebuild_search_caches`]
    /// whenever the items vec is replaced or the in-place edits inside
    /// it might have changed a searchable field. Used by the search
    /// hot path so a keystroke doesn't allocate one lowercased string
    /// per item per keystroke.
    pub items_lowered: Vec<LoweredItem>,
    /// Same idea as [`Self::items_lowered`] but for the trash view.
    pub trashed_lowered: Vec<LoweredItem>,
    /// Indices into either [`Self::items`] or [`Self::trashed_items`]
    /// (depending on the active filter), already filtered by the
    /// current item-type / folder / search-query trio and sorted by
    /// fuzzy score when a query is active. Read by
    /// [`Self::filtered_items`] in O(K), eliminating the per-frame
    /// O(N) recomputation.
    pub filtered_cache: Vec<usize>,
    /// Number of items whose `folder_id` is `None`. Cached so the
    /// folders sidebar can render the "(No folder)" badge in O(1)
    /// instead of scanning every item per frame.
    pub no_folder_count: usize,
    /// `folder_id → number of items in that folder`. Cached so the
    /// folders sidebar can render the per-folder badges in O(1) per
    /// row instead of paying an O(items) scan per row per frame
    /// (which on a 5 k-item, 20-folder vault adds up to 100 k+
    /// iterations per redraw — and a redraw happens on every
    /// keystroke).
    pub folder_counts: std::collections::HashMap<String, usize>,
    /// `collection_id → number of items belonging to that collection`.
    /// Same rationale as [`Self::folder_counts`].
    pub collection_counts: std::collections::HashMap<String, usize>,
    /// All folders visible in the current session (sorted alphabetically
    /// by name). Refreshed via the worker on login / after folder edits.
    pub folders: Vec<Folder>,
    /// All collections visible in the current session, across every
    /// organisation the user is a member of. Sorted by `Org / Name`.
    /// Personal-only accounts keep this empty. Used by the Folders
    /// sidebar (rendered after the folder rows) and by the
    /// memberships popup.
    pub collections: Vec<crate::domain::Collection>,
    /// Bitwarden organisations the user is a member of, used to
    /// render `"Org / Collection"` labels in the sidebar and the
    /// memberships popup.
    pub organizations: Vec<crate::domain::Organization>,
    /// Cache of `bw import --formats` output, populated once at
    /// login and consumed by the import popup's dropdown. Empty when
    /// the call fails or hasn't been made yet — the popup falls back
    /// to a hard-coded `bitwardenjson` so it still works.
    pub import_formats: Vec<String>,
    /// Currently active folder/collection filter (ANDed with
    /// `active_filter`).
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
    /// Toggled on after the backend reports a "new device" challenge —
    /// the user has to paste the code bw e-mailed them.
    pub otp_required: bool,
    /// Toggled on after the backend reports the account has a
    /// permanent second factor enrolled (Authenticator / Email /
    /// YubiKey). The same `otp_input` buffer is reused, but the
    /// submit path branches to
    /// [`crate::ports::VaultPort::login_with_two_factor`] instead of
    /// the device-verification path. Mutually exclusive with
    /// `otp_required`.
    pub two_factor_required: bool,
    /// Currently selected 2FA method. Defaults to
    /// [`crate::domain::TwoFactorMethod::Authenticator`] (the most
    /// common case); the user cycles it from the login form when
    /// the popup is up. Only meaningful when `two_factor_required`
    /// is `true`.
    pub two_factor_method: crate::domain::TwoFactorMethod,
    pub active_field: LoginField,
    pub login_error: bool,
    pub save_email: bool,
    /// Whether the unlocked session key should be persisted to a
    /// per-PPID runtime file (cleaned up when the parent shell dies).
    pub keep_session: bool,
    /// Whether the `bw` CLI is logged into an account (vault Locked or
    /// Unlocked) vs fully signed out. Tracked on `App` because the vault
    /// now lives on the worker thread, so the login flow can't call
    /// `status()` synchronously to decide unlock-vs-login. Set from the
    /// boot-status / login response handlers; cleared on logout.
    pub authenticated: bool,

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

    // ── Action / worker state ─────────────────────────────────────────────
    pub action_state: ActionState,
    pub action_tick: u8,
    /// Context for the single user request currently being served by the
    /// worker thread. `Some` ⇒ busy; input is gated and a new request
    /// must not be queued until the matching response clears it. Multi-step
    /// flows chain by setting a fresh ticket from a response handler.
    /// Claim it through [`Self::submit`] / [`Self::begin`], never by
    /// assigning directly — that's what stamps the watchdog timer and
    /// enforces the single-in-flight + worker-dead guards.
    pub in_flight: Option<InFlight>,
    /// When the current in-flight request was claimed. Drives the
    /// [`Self::watchdog_release_stuck_request`] backstop so a lost ticket
    /// (worker died mid-call, response dropped) can't gate input forever.
    pub request_started: Option<Instant>,
    /// Latched once the worker response channel closes — every worker
    /// thread is gone, so no response will ever arrive. [`Self::begin`]
    /// refuses while set and a persistent error is shown.
    pub worker_dead: bool,
    /// Configurable `bw list items` wall-clock budget (from settings),
    /// used to size the watchdog so a legitimately slow load on a huge
    /// vault isn't mistaken for a lost ticket.
    pub list_items_timeout_secs: u64,

    // ── Auto-lock ─────────────────────────────────────────────────────────
    pub auto_lock: bool,
    pub lock_after_secs: u64,
    pub last_activity: Instant,

    // ── Clipboard auto-clear ──────────────────────────────────────────────
    /// Seconds after which a copied secret is wiped from the system
    /// clipboard. `0` disables the feature; default is `30` (matches
    /// the Bitwarden GUI). Read once at boot from the settings port —
    /// changing the value in `config.toml` requires a restart.
    pub clipboard_clear_secs: u64,

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

    // ── Assign-collections popup state ───────────────────────────────────
    /// Buffer for the in-flight collections multi-select popup.
    /// `None` outside the popup. Used by the edit-mode "Collections"
    /// row to choose which of the item's owning org's collections it
    /// belongs to.
    pub assign_collections: Option<crate::tui::assign_collections::AssignCollectionsState>,

    // ── Reprompt popup state ──────────────────────────────────────────────
    /// Buffer for the in-flight master-password reverify popup. `None`
    /// outside the popup.
    pub reprompt: Option<crate::tui::reprompt::RepromptState>,

    /// Transient flag set by [`crate::tui::flows::reprompt::run_protected_action`]
    /// just before re-entering the protected flow. Consumed by the
    /// reprompt guards in `flows::copy` so the deferred action runs
    /// straight through without re-opening the popup it just came
    /// from. Always cleared inside the same call stack.
    pub reprompt_verified: bool,

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

    // ── Settings overlay (F9) ─────────────────────────────────────────────
    /// Which pane of the Settings overlay holds focus.
    pub settings_focus: SettingsFocus,
    /// Highlighted section in the sidebar (index into
    /// [`SettingsSection::ALL`]).
    pub settings_section: usize,
    /// Highlighted preset in the Theme panel (index into
    /// [`theme::Preset::ALL`]). Previews live as it moves.
    pub settings_theme_idx: usize,
    /// Theme active when the Settings overlay opened — restored if the
    /// user cancels (`Esc`/`F9`) instead of confirming.
    pub theme_before_settings: Theme,
    /// Screen the Settings overlay was opened from (returned to on close).
    pub settings_from: Screen,

    // ── Worker channels ───────────────────────────────────────────────────
    /// Send a [`WorkerRequest`] to the thread that owns the vault +
    /// generator ports.
    pub worker_tx: Sender<WorkerRequest>,
    /// Drain [`WorkerResponse`]s from the worker between frames.
    pub worker_rx: Receiver<WorkerResponse>,
    /// Cached session key for command-log redaction. The vault now lives
    /// on the worker thread, so `push_cmd` can no longer call
    /// `session_key()`; instead we cache the key here from the login /
    /// unlock response handlers and clear it on lock / logout. The `bw`
    /// argv never contains the key (it's passed via env), so this is
    /// defense-in-depth. Zeroized on drop / overwrite.
    pub session_marker: Option<Zeroizing<String>>,

    // ── Injected ports (synchronous, stay on the render thread) ───────────
    pub clipboard: Box<dyn ClipboardPort>,
    pub settings: Box<dyn SettingsPort>,
}

impl App {
    /// Constructs the initial state, reading user preferences via the
    /// settings port.
    pub fn new(
        worker_tx: Sender<WorkerRequest>,
        worker_rx: Receiver<WorkerResponse>,
        clipboard: Box<dyn ClipboardPort>,
        settings: Box<dyn SettingsPort>,
    ) -> Self {
        let cfg = settings.read();
        let saved_email = cfg.email.clone().unwrap_or_default();
        let theme = theme::load(&settings.config_dir());
        // Preselect the picker on the configured preset, else Nord.
        let settings_theme_idx = theme::configured_preset(&settings.config_dir())
            .or(Some(theme::Preset::DEFAULT))
            .and_then(|p| theme::Preset::ALL.iter().position(|&q| q == p))
            .unwrap_or(0);
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
            items_lowered: Vec::new(),
            trashed_lowered: Vec::new(),
            filtered_cache: Vec::new(),
            no_folder_count: 0,
            folder_counts: std::collections::HashMap::new(),
            collection_counts: std::collections::HashMap::new(),
            folders: Vec::new(),
            collections: Vec::new(),
            organizations: Vec::new(),
            import_formats: Vec::new(),
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
            two_factor_required: false,
            two_factor_method: crate::domain::TwoFactorMethod::Authenticator,
            active_field: if cfg.save_email {
                LoginField::Password
            } else {
                LoginField::Email
            },
            login_error: false,
            save_email: cfg.save_email,
            keep_session: cfg.keep_session,
            authenticated: false,
            search_query: String::new(),
            show_password: false,
            detail_field: 0,
            login_password_visible: false,
            cmd_log: Vec::new(),
            cmd_log_scroll: 0,
            action_state: ActionState::Idle,
            action_tick: 0,
            in_flight: None,
            request_started: None,
            worker_dead: false,
            list_items_timeout_secs: cfg.list_items_timeout_secs,
            auto_lock: cfg.auto_lock,
            lock_after_secs: cfg.lock_after_secs,
            last_activity: Instant::now(),
            clipboard_clear_secs: cfg.clipboard_clear_secs,
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
            assign_collections: None,
            reprompt: None,
            reprompt_verified: false,
            help_from: None,
            help_scroll: (0, 0),
            theme: theme.clone(),
            settings_focus: SettingsFocus::Sidebar,
            settings_section: 0,
            settings_theme_idx,
            theme_before_settings: theme,
            settings_from: Screen::Vault,
            worker_tx,
            worker_rx,
            session_marker: None,
            clipboard,
            settings,
        }
    }

    /// Whether a worker request is currently in flight. While `true`,
    /// input handlers gate most keys so a second request can't be queued.
    pub fn is_busy(&self) -> bool {
        self.in_flight.is_some()
    }

    // ── Worker request lifecycle ──────────────────────────────────────────

    /// Claims the in-flight slot for `slot` and stamps the watchdog timer,
    /// returning `true`. Refuses (returns `false`, leaving any current
    /// request untouched) when the worker is dead or a request is already
    /// in flight.
    ///
    /// Input is already gated while busy (`input::busy_blocks`), but
    /// `begin` is the belt-and-suspenders guard against a *programmatic*
    /// double-send (e.g. an auto-refresh racing a user action) silently
    /// overwriting `in_flight` and desynchronising the ticket ↔ response
    /// ordering. Every `request_*` flow claims the slot through this
    /// (usually via [`Self::submit`]) rather than assigning `in_flight`
    /// directly. Use bare `begin` only for a *silent* request that must
    /// not set a `Running` toast (the post-mutation reloads).
    pub fn begin(&mut self, slot: InFlight) -> bool {
        if self.worker_dead {
            self.set_action(ActionState::Error(
                "worker thread died — restart bytewarden".into(),
            ));
            return false;
        }
        if self.in_flight.is_some() {
            self.push_cmd("worker request", false, "busy — request ignored");
            return false;
        }
        self.in_flight = Some(slot);
        self.request_started = Some(Instant::now());
        true
    }

    /// Starts a worker request end-to-end: claims the slot ([`Self::begin`]),
    /// shows the `Running` toast, and sends on the worker lane. A failed
    /// send (worker gone) releases the slot and routes through
    /// [`Self::on_worker_dead`] instead of leaving the UI busy forever.
    /// Returns whether the request was dispatched — the shared body of
    /// every non-silent `request_*` flow.
    pub fn submit(&mut self, slot: InFlight, label: &str, req: WorkerRequest) -> bool {
        if !self.begin(slot) {
            return false;
        }
        self.set_action(ActionState::Running(label.to_string()));
        if self.worker_tx.send(req).is_err() {
            self.in_flight = None;
            self.on_worker_dead();
            return false;
        }
        true
    }

    /// Unwedges the UI after the worker response channel closed — every
    /// worker thread is gone, so no response will ever arrive. Releases the
    /// in-flight slot (otherwise `busy_blocks` swallows keys forever) and
    /// surfaces a persistent error, once.
    pub fn on_worker_dead(&mut self) {
        if self.worker_dead {
            return;
        }
        self.worker_dead = true;
        self.in_flight = None;
        self.request_started = None;
        self.set_action(ActionState::Error(
            "worker thread died — bw calls disabled; restart bytewarden".into(),
        ));
        self.push_cmd("worker", false, "response channel closed — worker died");
    }

    /// Watchdog for a lost in-flight ticket: every `bw` call has a per-op
    /// timeout, so a claimed slot must resolve within the largest plausible
    /// budget. If it doesn't (worker died mid-call, response dropped),
    /// release the slot so the UI doesn't stay busy forever. Called once
    /// per run-loop tick.
    pub fn watchdog_release_stuck_request(&mut self) {
        let Some(started) = self.request_started else {
            return;
        };
        if self.in_flight.is_none() {
            return;
        }
        // Above every fixed per-op timeout (≤60 s) and the configurable
        // list budget, plus generous slack — it only ever fires on a
        // genuinely lost ticket, not a slow-but-live call.
        let budget = self.list_items_timeout_secs.max(90).saturating_add(60);
        if started.elapsed() > Duration::from_secs(budget) {
            self.in_flight = None;
            self.request_started = None;
            self.set_action(ActionState::Error(
                "request got no response in time — released".into(),
            ));
            self.push_cmd("worker watchdog", false, "abandoned in-flight request");
        }
    }

    // ── Activity / navigation ─────────────────────────────────────────────

    /// Records "user is active right now" — resets the auto-lock timer.
    pub fn reset_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Opens the Settings overlay over the current screen. Stashes the
    /// originating screen and the active theme (so `Esc`/`F9` can restore
    /// it), and starts focus on the section sidebar.
    pub fn open_settings(&mut self) {
        self.settings_from = self.screen.clone();
        self.theme_before_settings = self.theme.clone();
        self.settings_focus = SettingsFocus::Sidebar;
        self.settings_section = 0;
        self.screen = Screen::Settings;
    }

    /// Applies the highlighted preset to [`Self::theme`] as a live
    /// preview — no persistence. Called whenever the picker moves.
    pub fn settings_preview_theme(&mut self) {
        if let Some(&p) = theme::Preset::ALL.get(self.settings_theme_idx) {
            self.theme = theme::adapt(
                Theme::from_palette(&p.palette()),
                theme::ColorCaps::detect(),
            );
        }
    }

    /// Confirms the highlighted preset: applies it, persists
    /// `name = "<preset>"` to `config.toml`, and closes the overlay.
    pub fn settings_confirm_theme(&mut self) {
        if let Some(&p) = theme::Preset::ALL.get(self.settings_theme_idx) {
            self.theme = theme::adapt(
                Theme::from_palette(&p.palette()),
                theme::ColorCaps::detect(),
            );
            self.settings.write_theme_name(p.name());
            self.push_cmd("theme", true, &format!("saved {}", p.name()));
            self.set_action(ActionState::Done(format!("Theme: {}", p.label())));
        }
        self.screen = self.settings_from.clone();
    }

    /// Cancels the Settings overlay: restores the theme that was active
    /// when it opened (dropping any live preview) and closes it.
    pub fn settings_cancel(&mut self) {
        self.theme = self.theme_before_settings.clone();
        self.screen = self.settings_from.clone();
    }

    /// `true` while bw is asking the user for an interactive code —
    /// either a device-verification OTP (e-mailed on first login from
    /// a new device) or a permanent second-factor code (Authenticator).
    /// Layout, click hit-testing and tab order use this rather than
    /// branching on the two flags individually.
    pub fn awaiting_code(&self) -> bool {
        self.otp_required || self.two_factor_required
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

    /// Activates the highlighted filter. Returns `true` when the new
    /// filter is [`ItemFilter::Trash`] so the caller can kick off the
    /// trash load on the worker (the trash list is fetched on demand).
    pub fn apply_filter(&mut self) -> bool {
        self.active_filter = ITEM_FILTERS[self.filter_selected].clone();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.focus = Focus::List;
        self.rebuild_filtered_cache();
        self.active_filter == ItemFilter::Trash
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
    ///
    /// Reads from [`Self::filtered_cache`], which is rebuilt eagerly
    /// at every relevant mutation by [`Self::rebuild_filtered_cache`].
    /// In the rendering hot path this is O(K) — one indirection per
    /// visible row — instead of the O(N) re-filter-and-rerank the
    /// previous implementation paid per frame.
    pub fn filtered_items(&self) -> Vec<&Item> {
        let source = if self.active_filter == ItemFilter::Trash {
            &self.trashed_items
        } else {
            &self.items
        };
        self.filtered_cache
            .iter()
            .filter_map(|&i| source.get(i))
            .collect()
    }

    pub fn selected_item(&self) -> Option<&Item> {
        self.filtered_items().get(self.selected_index).copied()
    }

    /// Id of the currently selected (filtered) item, if any — captured
    /// before a reload so the cursor can be put back on the same item.
    pub fn selected_item_id(&self) -> Option<String> {
        self.selected_item().map(|i| i.id.clone())
    }

    /// Re-anchors the list cursor onto the item with `id` after a reload
    /// that may have reordered or replaced the list. Falls back to
    /// clamping the old index into range when the item is gone (deleted
    /// elsewhere, filtered out). Keeps `scroll_offset` consistent so the
    /// cursor stays visible. This is the invalidation contract: a
    /// background/post-mutation refresh must never yank the cursor onto
    /// an unrelated row just because indices shifted.
    pub fn reanchor_selection(&mut self, id: Option<&str>) {
        let len = self.filtered_items().len();
        self.selected_index = match id {
            Some(id) => self
                .filtered_items()
                .iter()
                .position(|i| i.id == id)
                .unwrap_or_else(|| self.selected_index.min(len.saturating_sub(1))),
            None => self.selected_index.min(len.saturating_sub(1)),
        };
        if len == 0 {
            self.selected_index = 0;
        }
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
    }

    /// Rebuilds [`Self::items_lowered`] from [`Self::items`] and
    /// [`Self::trashed_lowered`] from [`Self::trashed_items`]. Called
    /// after any mutation that could touch a searchable field
    /// (load, sync, create, edit, delete, restore, favorite toggle).
    ///
    /// Always pairs with a [`Self::rebuild_filtered_cache`] call,
    /// because the filtered cache references items by index — adding
    /// or removing items shifts those indices.
    pub fn rebuild_search_caches(&mut self) {
        self.items_lowered = self.items.iter().map(LoweredItem::from_item).collect();
        self.trashed_lowered = self
            .trashed_items
            .iter()
            .map(LoweredItem::from_item)
            .collect();
    }

    /// Recomputes [`Self::filtered_cache`] from the current items,
    /// active filters and search query. Cheap to call — pure CPU,
    /// no allocations beyond the result vector.
    pub fn rebuild_filtered_cache(&mut self) {
        let (source, lowered): (&[Item], &[LoweredItem]) =
            if self.active_filter == ItemFilter::Trash {
                (&self.trashed_items, &self.trashed_lowered)
            } else {
                (&self.items, &self.items_lowered)
            };
        self.filtered_cache = compute_filtered_indices(
            source,
            lowered,
            &self.active_filter,
            &self.active_folder,
            &self.search_query,
        );
    }

    /// Rebuilds both caches in the order required by their
    /// invariants (lowered first — filtered references the lowered
    /// vec for scoring). Use this from any mutation that replaces
    /// items wholesale or might have altered a searchable field.
    pub fn rebuild_caches(&mut self) {
        self.rebuild_search_caches();
        self.rebuild_filtered_cache();
        self.rebuild_sidebar_counts();
    }

    /// Rebuilds [`Self::no_folder_count`], [`Self::folder_counts`] and
    /// [`Self::collection_counts`] from the current `app.items`. One
    /// pass over the items list, O(N) total — replaces the previous
    /// O(items × (folders + collections)) per-frame work in the folder
    /// sidebar renderer.
    ///
    /// Cleared keys for folders / collections that no longer have any
    /// items are intentionally dropped from the maps so the renderer
    /// reads `Some(0)` only when the row truly has zero entries (the
    /// missing-key path also resolves to zero, so the renderer treats
    /// both identically).
    pub fn rebuild_sidebar_counts(&mut self) {
        self.no_folder_count = 0;
        self.folder_counts.clear();
        self.collection_counts.clear();
        for item in &self.items {
            match item.folder_id.as_deref() {
                None => self.no_folder_count += 1,
                Some(id) => *self.folder_counts.entry(id.to_string()).or_insert(0) += 1,
            }
            for cid in &item.collection_ids {
                *self.collection_counts.entry(cid.clone()).or_insert(0) += 1;
            }
        }
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
        self.rebuild_filtered_cache();
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.focus = Focus::List;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.rebuild_filtered_cache();
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
    pub fn push_cmd(&mut self, cmd: &str, ok: bool, detail: &(impl std::fmt::Display + ?Sized)) {
        // `detail` is `&dyn Display` so a typed `BwError`, a `&str`
        // literal and a `&format!(…)` result all pass without the caller
        // stringifying first — the classified error carries its own
        // message. (`dyn` rather than a generic so an unsized `&str`
        // coerces cleanly and existing `&e` call sites stay unchanged.)
        let detail = detail.to_string();
        // The vault lives on the worker thread, so we can't call
        // `session_key()` here. Redact against the cached `session_marker`
        // (set from the login / unlock response handlers). The `bw` argv
        // never carries the key anyway — this is defense-in-depth.
        let redacted = redact_cmd(cmd, self.session_marker.as_deref().map(|s| s.as_str()));
        crate::tui::debug_log::append(&redacted, ok, &detail);
        self.cmd_log.push(CmdEntry {
            cmd: redacted,
            ok,
            detail,
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
    pub fn cmd_err(&mut self, cmd: &str, e: &(impl std::fmt::Display + ?Sized), label: &str) {
        // Accepts a typed `BwError` (or any `Display`) by reference — the
        // existing `&e` call sites stay unchanged. Rendered once for both
        // the command log and the feedback strip.
        let e = e.to_string();
        self.push_cmd(cmd, false, &e);
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
        self.two_factor_required = false;
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
    ///
    /// Uses `sort_by_cached_key` so the lowercased key is computed once
    /// per item per sort instead of once per comparison. Rebuilds both
    /// search caches because the indices stored in `filtered_cache`
    /// and the parallel positions in `items_lowered` are now stale.
    pub fn sort_items(&mut self) {
        self.items.sort_by_cached_key(|i| i.name.to_lowercase());
        self.rebuild_caches();
    }
}

/// Pure helper extracted from [`App::rebuild_filtered_cache`] so the
/// filtering+ranking logic can be tested in isolation (without
/// instantiating an `App` plus four trait-object adapters).
///
/// Returns the indices into `source` (and the parallel `lowered`) that
/// match the active filter, folder filter and search query, sorted
/// by fuzzy score descending when a query is active and in original
/// order otherwise.
///
/// The trash bucket bypasses the folder filter — trashed items often
/// lost their folder context, so we deliberately surface every one.
pub fn compute_filtered_indices(
    source: &[Item],
    lowered: &[crate::domain::LoweredItem],
    active_filter: &ItemFilter,
    active_folder: &crate::tui::folders::FolderFilter,
    search_query: &str,
) -> Vec<usize> {
    use crate::domain::search::fuzzy_score_lowered;

    let mut indices: Vec<usize> = if *active_filter == ItemFilter::Trash {
        (0..source.len()).collect()
    } else {
        source
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                active_filter.matches(item)
                    && active_folder.matches(item.folder_id.as_deref(), &item.collection_ids)
            })
            .map(|(i, _)| i)
            .collect()
    };

    if !search_query.is_empty() {
        let query = search_query.to_lowercase();
        // The `url:` prefix narrows the search to login URIs only —
        // useful for "what credentials do I have for github.com?"
        // queries, the same use case `bw list items --url <url>`
        // covers from the CLI. The substring is matched
        // case-insensitively against each lowered URI; matches keep
        // the items in their pre-search order (no fuzzy ranking,
        // because URLs aren't free-form names where ordering
        // matters).
        if let Some(rest) = query.strip_prefix("url:") {
            let needle = rest.trim();
            if needle.is_empty() {
                return indices; // bare "url:" matches everything.
            }
            indices.retain(|&i| {
                lowered
                    .get(i)
                    .is_some_and(|l| l.uris.iter().any(|u| u.contains(needle)))
            });
            return indices;
        }
        let mut scored: Vec<(i32, usize)> = indices
            .into_iter()
            .filter_map(|i| {
                let l = lowered.get(i)?;
                let s = fuzzy_score_lowered(l, &query);
                if s > 0 { Some((s, i)) } else { None }
            })
            .collect();
        scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
        indices = scored.into_iter().map(|(_, i)| i).collect();
    }

    indices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::LoweredItem;
    use crate::domain::item::{Item, LoginData};
    use crate::ports::{BwError, UserSettings};
    use crate::tui::folders::FolderFilter;
    use std::sync::mpsc::channel;

    struct NoopClipboard;
    impl ClipboardPort for NoopClipboard {
        fn write(&self, _: &str) -> Result<(), BwError> {
            Ok(())
        }
    }

    struct DefaultSettings;
    impl SettingsPort for DefaultSettings {
        fn read(&self) -> UserSettings {
            UserSettings::default()
        }
        fn write(&self, _: bool, _: Option<&str>) {}
        fn write_auto_lock(&self, _: bool) {}
        fn write_keep_session(&self, _: bool) {}
        fn write_theme_name(&self, _: &str) {}
        fn config_dir(&self) -> std::path::PathBuf {
            std::path::PathBuf::from(".")
        }
    }

    /// Builds an `App` wired to live-but-inert channels. Returns the
    /// worker-request receiver (so a request `submit`s to a connected
    /// channel) and the response sender (kept alive so `App::worker_rx`
    /// stays connected) — hold both for the duration of the test.
    fn fresh_app() -> (App, Receiver<WorkerRequest>, Sender<WorkerResponse>) {
        let (worker_tx, req_rx) = channel::<WorkerRequest>();
        let (resp_tx, worker_rx) = channel::<WorkerResponse>();
        let app = App::new(
            worker_tx,
            worker_rx,
            Box::new(NoopClipboard),
            Box::new(DefaultSettings),
        );
        (app, req_rx, resp_tx)
    }

    #[test]
    fn begin_enforces_single_in_flight() {
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        assert!(app.begin(InFlight::LoadItems));
        assert!(app.is_busy());
        // A second claim is refused while one is in flight, and the
        // original ticket survives (no silent clobber).
        assert!(!app.begin(InFlight::Sync));
        assert_eq!(app.in_flight, Some(InFlight::LoadItems));
    }

    #[test]
    fn begin_refuses_when_worker_dead() {
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.on_worker_dead();
        assert!(app.worker_dead);
        assert!(!app.begin(InFlight::LoadItems));
        assert!(app.in_flight.is_none());
    }

    #[test]
    fn submit_dispatches_toast_and_request() {
        let (mut app, req_rx, _resp_tx) = fresh_app();
        assert!(app.submit(InFlight::Sync, "Syncing…", WorkerRequest::Sync));
        assert!(app.is_busy());
        assert!(matches!(app.action_state, ActionState::Running(_)));
        assert!(app.request_started.is_some());
        // The request actually reached the worker channel.
        assert!(matches!(req_rx.try_recv(), Ok(WorkerRequest::Sync)));
    }

    #[test]
    fn submit_on_dead_channel_marks_worker_dead() {
        let (mut app, req_rx, _resp_tx) = fresh_app();
        drop(req_rx); // the worker is gone — the send will fail
        assert!(!app.submit(InFlight::Sync, "Syncing…", WorkerRequest::Sync));
        assert!(app.worker_dead);
        assert!(app.in_flight.is_none());
    }

    #[test]
    fn watchdog_leaves_a_fresh_request_alone() {
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.submit(InFlight::Sync, "Syncing…", WorkerRequest::Sync);
        // Just claimed — nowhere near the budget, so the slot stays.
        app.watchdog_release_stuck_request();
        assert!(app.is_busy());
    }

    #[test]
    fn reanchor_selection_follows_the_item_by_id() {
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.items = vec![
            item("a", "A", 1, None),
            item("b", "B", 1, None),
            item("c", "C", 1, None),
        ];
        app.rebuild_caches();
        app.selected_index = 1; // "b"
        assert_eq!(app.selected_item_id().as_deref(), Some("b"));
        // The list comes back reordered — the cursor must follow "b".
        app.items = vec![
            item("c", "C", 1, None),
            item("b", "B", 1, None),
            item("a", "A", 1, None),
        ];
        app.rebuild_caches();
        app.reanchor_selection(Some("b"));
        assert_eq!(app.selected_item().map(|i| i.id.clone()), Some("b".into()));
    }

    #[test]
    fn reanchor_clamps_when_the_item_is_gone() {
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.items = vec![item("a", "A", 1, None), item("b", "B", 1, None)];
        app.rebuild_caches();
        app.selected_index = 1; // "b"
        // "b" deleted elsewhere — the list is now shorter.
        app.items = vec![item("a", "A", 1, None)];
        app.rebuild_caches();
        app.reanchor_selection(Some("b"));
        assert_eq!(app.selected_index, 0);
        assert!(app.selected_item().is_some());
    }

    fn item(id: &str, name: &str, item_type: u8, folder: Option<&str>) -> Item {
        Item {
            id: id.into(),
            name: name.into(),
            item_type,
            login: None,
            card: None,
            identity: None,
            ssh_key: None,
            notes: None,
            folder_id: folder.map(|s| s.to_string()),
            organization_id: None,
            collection_ids: Vec::new(),
            favorite: false,
            fields: vec![],
            attachments: None,
            reprompt: 0,
        }
    }

    fn login_item(id: &str, name: &str, username: &str) -> Item {
        let mut i = item(id, name, 1, None);
        i.login = Some(LoginData {
            username: Some(username.into()),
            password: None,
            uris: None,
            totp: None,
        });
        i
    }

    fn lowered(items: &[Item]) -> Vec<LoweredItem> {
        items.iter().map(LoweredItem::from_item).collect()
    }

    #[test]
    fn redact_cmd_replaces_cached_session_key() {
        assert_eq!(
            redact_cmd("bw unlock SECRETKEY", Some("SECRETKEY")),
            "bw unlock ***"
        );
    }

    #[test]
    fn redact_cmd_is_noop_without_a_marker() {
        assert_eq!(redact_cmd("bw status", None), "bw status");
        // An empty marker must not turn every gap into `***`.
        assert_eq!(redact_cmd("bw status", Some("")), "bw status");
    }

    #[test]
    fn all_filter_with_no_query_keeps_original_order() {
        let items = vec![
            item("a", "Zeta", 1, None),
            item("b", "Alpha", 1, None),
            item("c", "Mu", 1, None),
        ];
        let l = lowered(&items);
        let idx = compute_filtered_indices(&items, &l, &ItemFilter::All, &FolderFilter::All, "");
        assert_eq!(idx, vec![0, 1, 2]);
    }

    #[test]
    fn type_filter_drops_non_matching_types() {
        let items = vec![
            item("a", "Login", 1, None),
            item("b", "Card", 3, None),
            item("c", "Login2", 1, None),
        ];
        let l = lowered(&items);
        let idx = compute_filtered_indices(&items, &l, &ItemFilter::Login, &FolderFilter::All, "");
        assert_eq!(idx, vec![0, 2]);
    }

    #[test]
    fn folder_filter_drops_items_outside_the_folder() {
        let items = vec![
            item("a", "x", 1, Some("F1")),
            item("b", "y", 1, Some("F2")),
            item("c", "z", 1, None),
        ];
        let l = lowered(&items);
        let idx = compute_filtered_indices(
            &items,
            &l,
            &ItemFilter::All,
            &FolderFilter::Folder("F1".into()),
            "",
        );
        assert_eq!(idx, vec![0]);
        // No-folder filter keeps only the items with no folder_id.
        let idx_none =
            compute_filtered_indices(&items, &l, &ItemFilter::All, &FolderFilter::NoFolder, "");
        assert_eq!(idx_none, vec![2]);
    }

    #[test]
    fn search_reorders_by_fuzzy_score() {
        let items = vec![
            login_item("a", "GitHub Personal", "alice"),
            login_item("b", "Old GitHub", "alice"),
            login_item("c", "Unrelated", "bob"),
        ];
        let l = lowered(&items);
        let idx =
            compute_filtered_indices(&items, &l, &ItemFilter::All, &FolderFilter::All, "github");
        // "GitHub Personal" — name prefix substring → 100 + 20 = 120.
        // "Old GitHub" — name substring (no prefix) → 100.
        // "Unrelated" — no match → dropped.
        assert_eq!(idx, vec![0, 1]);
    }

    #[test]
    fn search_with_no_match_returns_empty() {
        let items = vec![login_item("a", "Site", "alice")];
        let l = lowered(&items);
        let idx = compute_filtered_indices(
            &items,
            &l,
            &ItemFilter::All,
            &FolderFilter::All,
            "no-such-string",
        );
        assert!(idx.is_empty());
    }

    #[test]
    fn trash_filter_includes_every_source_item_regardless_of_folder() {
        // The trash bucket should bypass the folder filter — we want
        // to surface every trashed item even if its folder context is
        // gone or pointing at a folder the user has since deleted.
        let trashed = vec![item("a", "x", 1, Some("F1")), item("b", "y", 1, None)];
        let l = lowered(&trashed);
        let idx = compute_filtered_indices(
            &trashed,
            &l,
            &ItemFilter::Trash,
            // Even with a strict folder filter that wouldn't match…
            &FolderFilter::Folder("F-NOPE".into()),
            "",
        );
        assert_eq!(idx, vec![0, 1]);
    }

    fn login_item_with_uri(id: &str, name: &str, uri: &str) -> Item {
        use crate::domain::item::UriData;
        let mut i = item(id, name, 1, None);
        i.login = Some(LoginData {
            username: None,
            password: None,
            uris: Some(vec![UriData {
                uri: Some(uri.into()),
                match_type: None,
            }]),
            totp: None,
        });
        i
    }

    #[test]
    fn url_prefix_filters_by_uri_substring_only() {
        let items = vec![
            login_item_with_uri("a", "GitHub Personal", "https://github.com"),
            login_item_with_uri("b", "GitHub Sandbox", "https://github.io/sandbox"),
            login_item_with_uri("c", "Gmail", "https://mail.google.com"),
            // Item whose name contains "github" but URI doesn't —
            // must be excluded under url: search.
            item("d", "github typo", 1, None),
        ];
        let l = lowered(&items);
        let idx = compute_filtered_indices(
            &items,
            &l,
            &ItemFilter::All,
            &FolderFilter::All,
            "url:github",
        );
        assert_eq!(idx, vec![0, 1]);
    }

    #[test]
    fn url_prefix_with_empty_needle_does_not_filter() {
        let items = vec![item("a", "x", 1, None), item("b", "y", 1, None)];
        let l = lowered(&items);
        let idx =
            compute_filtered_indices(&items, &l, &ItemFilter::All, &FolderFilter::All, "url:");
        // Bare prefix → all items (no narrowing).
        assert_eq!(idx, vec![0, 1]);
    }

    #[test]
    fn url_prefix_skips_fuzzy_ranking() {
        // Two items with URIs containing the needle; preserve the
        // input order (don't ranknames or anything).
        let items = vec![
            login_item_with_uri("a", "Z Site", "https://example.com/a"),
            login_item_with_uri("b", "A Site", "https://example.com/b"),
        ];
        let l = lowered(&items);
        let idx = compute_filtered_indices(
            &items,
            &l,
            &ItemFilter::All,
            &FolderFilter::All,
            "url:example.com",
        );
        // Both match — order preserved (a then b).
        assert_eq!(idx, vec![0, 1]);
    }

    #[test]
    fn collection_filter_keeps_items_in_that_collection() {
        let mut items = vec![
            item("a", "x", 1, None),
            item("b", "y", 1, None),
            item("c", "z", 1, None),
        ];
        items[0].collection_ids = vec!["c1".into()];
        items[1].collection_ids = vec!["c1".into(), "c2".into()];
        items[2].collection_ids = vec!["c2".into()];
        let l = lowered(&items);
        // Filter to collection c1 — items 0 and 1 match.
        let idx = compute_filtered_indices(
            &items,
            &l,
            &ItemFilter::All,
            &FolderFilter::Collection("c1".into()),
            "",
        );
        assert_eq!(idx, vec![0, 1]);
        // c2 — items 1 and 2 match.
        let idx = compute_filtered_indices(
            &items,
            &l,
            &ItemFilter::All,
            &FolderFilter::Collection("c2".into()),
            "",
        );
        assert_eq!(idx, vec![1, 2]);
    }

    #[test]
    fn favorites_filter_only_keeps_starred() {
        let mut items = vec![
            item("a", "x", 1, None),
            item("b", "y", 1, None),
            item("c", "z", 1, None),
        ];
        items[1].favorite = true;
        let l = lowered(&items);
        let idx =
            compute_filtered_indices(&items, &l, &ItemFilter::Favorites, &FolderFilter::All, "");
        assert_eq!(idx, vec![1]);
    }
}
