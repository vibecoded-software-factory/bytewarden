//! [`App`] — the global state container for the TUI.
//!
//! `App` holds:
//!
//! * Navigation state (current screen, focus).
//! * The vault list ([`crate::tui::vault::Vault`]: items + trash + the
//!   search/filter caches + the list cursor and its invalidation
//!   contract) and the session reference data (folders, collections…).
//! * Per-screen form state, each in its own sub-struct (login, edit,
//!   create, settings overlay) plus the popup states.
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

use crate::domain::filter::{ITEM_FILTERS, ItemFilter};
use crate::domain::folder::Folder;
use crate::domain::item::Item;
use crate::ports::{ClipboardPort, SettingsPort};
use crate::tui::action::{ActionState, CmdEntry};
use crate::tui::cmd_log::CmdLog;
use crate::tui::generator::GeneratorState;
use crate::tui::item_forms::{CreateForm, EditForm};
use crate::tui::login_form::LoginForm;
use crate::tui::mouse_areas::MouseAreas;
use crate::tui::screens::{Focus, LoginField, Screen};
use crate::tui::settings_overlay::{SettingsFocus, SettingsOverlay};
use crate::tui::theme::{self, Theme};
use crate::tui::vault::Vault;
use crate::tui::worker::{InFlight, WorkerRequest, WorkerResponse};

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

/// Global TUI state.
pub struct App {
    // ── Screen / focus ────────────────────────────────────────────────────
    pub screen: Screen,
    pub should_quit: bool,
    pub focus: Focus,

    // ── Vault list ────────────────────────────────────────────────────────
    /// The vault's item data + search/filter caches + list-navigation
    /// cursor, with its own invalidation contract. See
    /// [`crate::tui::vault::Vault`].
    pub vault: Vault,

    // ── Session reference data ────────────────────────────────────────────
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

    // ── Login form ────────────────────────────────────────────────────────
    /// The login screen's form state — buffers, focus, toggles and the
    /// transient 2FA / device-verification flags. See
    /// [`crate::tui::login_form::LoginForm`].
    pub login: LoginForm,
    /// Whether the `bw` CLI is logged into an account (vault Locked or
    /// Unlocked) vs fully signed out. Tracked on `App` (not `LoginForm`)
    /// because it outlives the login screen: the vault now lives on the
    /// worker thread, so the login flow can't call `status()`
    /// synchronously to decide unlock-vs-login. Set from the
    /// boot-status / login response handlers; cleared on logout.
    pub authenticated: bool,

    // ── Detail / edit / create ────────────────────────────────────────────
    pub show_password: bool,
    pub detail_field: usize,

    // ── Command log ───────────────────────────────────────────────────────
    /// The redacted `bw` command backlog + its scroll. See
    /// [`crate::tui::cmd_log::CmdLog`].
    pub cmd_log: CmdLog,

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
    /// The edit-item form (Detail screen's editable mode). See
    /// [`crate::tui::item_forms::EditForm`].
    pub edit: EditForm,
    /// The create-item form. See [`crate::tui::item_forms::CreateForm`].
    pub create: CreateForm,

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

    // ── Command palette state ─────────────────────────────────────────────
    /// Buffer for the in-flight command palette (`Ctrl+P`). `None`
    /// outside the palette.
    pub palette: Option<crate::tui::flows::palette::PaletteState>,

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
    /// The Settings overlay's transient state. See
    /// [`crate::tui::settings_overlay::SettingsOverlay`].
    pub settings_ui: SettingsOverlay,

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
            vault: Vault::default(),
            folders: Vec::new(),
            collections: Vec::new(),
            organizations: Vec::new(),
            import_formats: Vec::new(),
            login: LoginForm::new(saved_email, cfg.save_email, cfg.keep_session),
            authenticated: false,
            show_password: false,
            detail_field: 0,
            cmd_log: CmdLog::default(),
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
            edit: EditForm::default(),
            create: CreateForm::default(),
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
            palette: None,
            reprompt_verified: false,
            help_from: None,
            help_scroll: (0, 0),
            theme: theme.clone(),
            settings_ui: SettingsOverlay {
                focus: SettingsFocus::Sidebar,
                section: 0,
                theme_idx: settings_theme_idx,
                theme_before: theme,
                from: Screen::Vault,
            },
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
        self.settings_ui.from = self.screen.clone();
        self.settings_ui.theme_before = self.theme.clone();
        self.settings_ui.focus = SettingsFocus::Sidebar;
        self.settings_ui.section = 0;
        self.screen = Screen::Settings;
    }

    /// Applies the highlighted preset to [`Self::theme`] as a live
    /// preview — no persistence. Called whenever the picker moves.
    pub fn settings_preview_theme(&mut self) {
        if let Some(&p) = theme::Preset::ALL.get(self.settings_ui.theme_idx) {
            self.theme = theme::adapt(
                Theme::from_palette(&p.palette()),
                theme::ColorCaps::detect(),
            );
        }
    }

    /// Confirms the highlighted preset: applies it, persists
    /// `name = "<preset>"` to `config.toml`, and closes the overlay.
    pub fn settings_confirm_theme(&mut self) {
        if let Some(&p) = theme::Preset::ALL.get(self.settings_ui.theme_idx) {
            self.theme = theme::adapt(
                Theme::from_palette(&p.palette()),
                theme::ColorCaps::detect(),
            );
            self.settings.write_theme_name(p.name());
            self.push_cmd("theme", true, &format!("saved {}", p.name()));
            self.set_action(ActionState::Done(format!("Theme: {}", p.label())));
        }
        self.screen = self.settings_ui.from.clone();
    }

    /// Cancels the Settings overlay: restores the theme that was active
    /// when it opened (dropping any live preview) and closes it.
    pub fn settings_cancel(&mut self) {
        self.theme = self.settings_ui.theme_before.clone();
        self.screen = self.settings_ui.from.clone();
    }

    pub fn go_to_vault(&mut self) {
        self.screen = Screen::Vault;
        self.vault.selected_index = 0;
        self.vault.scroll_offset = 0;
        self.focus = Focus::Search;
    }

    pub fn go_to_detail(&mut self) {
        if !self.vault.filtered_items().is_empty() {
            self.screen = Screen::Detail;
            self.show_password = false;
            self.detail_field = 0;
        }
    }

    pub fn go_back(&mut self) {
        match self.screen {
            Screen::Detail => {
                if self.edit.active {
                    self.edit.active = false;
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

    // ── Filter / search (cross into focus) ────────────────────────────────

    /// Activates the highlighted filter. Returns `true` when the new
    /// filter is [`ItemFilter::Trash`] so the caller can kick off the
    /// trash load on the worker (the trash list is fetched on demand).
    pub fn apply_filter(&mut self) -> bool {
        self.vault.active_filter = ITEM_FILTERS[self.vault.filter_selected].clone();
        self.vault.selected_index = 0;
        self.vault.scroll_offset = 0;
        self.focus = Focus::List;
        self.vault.rebuild_filtered_cache();
        self.vault.is_trash_view()
    }

    pub fn clear_search(&mut self) {
        self.vault.search_query.clear();
        self.focus = Focus::List;
        self.vault.selected_index = 0;
        self.vault.scroll_offset = 0;
        self.vault.rebuild_filtered_cache();
    }

    // ── Command log + action state ────────────────────────────────────────

    /// Appends a redacted command + its result to the log.
    ///
    /// When `BYTEWARDEN_DEBUG=1` is set the same redacted line is also
    /// appended to `~/.bytewarden.log` for offline troubleshooting —
    /// see [`crate::tui::debug_log`]. The check is cheap when the env
    /// var is unset, so leaving it off costs nothing. The capping +
    /// scroll bookkeeping lives on [`CmdLog::push`].
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

    // ── Login form plumbing (settings-backed) ─────────────────────────────

    /// Persists the e-mail when the "save e-mail" box is ticked — call
    /// after editing the Email field so a typed address survives a
    /// relaunch (the side effect the old `insert_char`/`delete_char_*`
    /// carried inline).
    pub fn persist_email_if_saving(&mut self) {
        if self.login.active_field == LoginField::Email && self.login.save_email {
            let e = self.login.email_input.text().to_string();
            self.settings.write(true, Some(&e));
        }
    }

    pub fn toggle_save_email(&mut self) {
        self.login.save_email = !self.login.save_email;
        if self.login.save_email {
            let e = self.login.email_input.text().to_string();
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
        self.login.keep_session = !self.login.keep_session;
        self.settings.write_keep_session(self.login.keep_session);
        if !self.login.keep_session {
            crate::tui::session_file::clear();
        }
    }

    /// Number of detail-screen rows for the currently selected item.
    ///
    /// Delegates to the shared [`crate::tui::detail_fields`] builder so
    /// the count never diverges from what the renderer actually shows.
    pub fn detail_field_count(&self) -> usize {
        let Some(item) = self.vault.selected_item() else {
            return 0;
        };
        crate::tui::detail_fields::build_detail_fields(item, false, 0).len()
    }
}

/// Pure helper extracted from [`crate::tui::vault::Vault::rebuild_filtered_cache`] so the
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
    fn command_palette_filters_moves_and_cancels() {
        use crate::tui::flows::palette;
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.screen = Screen::Vault;
        palette::open(&mut app);
        // Vault context with no selected item → the app-wide commands only.
        let total = app.palette.as_ref().unwrap().all.len();
        assert!(total >= 11, "expected the app-wide commands, got {total}");
        assert_eq!(app.screen, Screen::CommandPalette);

        // Typing narrows the filtered set by label substring.
        app.palette.as_mut().unwrap().query.insert_str("sync");
        palette::rebuild_filter(&mut app);
        assert_eq!(app.palette.as_ref().unwrap().filtered.len(), 1);

        // Selection clamps within the filtered list.
        palette::move_selection(&mut app, 10);
        assert_eq!(app.palette.as_ref().unwrap().selected, 0);

        // Cancel restores the origin screen and drops the state.
        palette::cancel(&mut app);
        assert!(app.palette.is_none());
        assert_eq!(app.screen, Screen::Vault);
    }

    #[test]
    fn reanchor_selection_follows_the_item_by_id() {
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.vault.items = vec![
            item("a", "A", 1, None),
            item("b", "B", 1, None),
            item("c", "C", 1, None),
        ];
        app.vault.rebuild_caches();
        app.vault.selected_index = 1; // "b"
        assert_eq!(app.vault.selected_item_id().as_deref(), Some("b"));
        // The list comes back reordered — the cursor must follow "b".
        app.vault.items = vec![
            item("c", "C", 1, None),
            item("b", "B", 1, None),
            item("a", "A", 1, None),
        ];
        app.vault.rebuild_caches();
        app.vault.reanchor_selection(Some("b"));
        assert_eq!(
            app.vault.selected_item().map(|i| i.id.clone()),
            Some("b".into())
        );
    }

    #[test]
    fn reanchor_clamps_when_the_item_is_gone() {
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.vault.items = vec![item("a", "A", 1, None), item("b", "B", 1, None)];
        app.vault.rebuild_caches();
        app.vault.selected_index = 1; // "b"
        // "b" deleted elsewhere — the list is now shorter.
        app.vault.items = vec![item("a", "A", 1, None)];
        app.vault.rebuild_caches();
        app.vault.reanchor_selection(Some("b"));
        assert_eq!(app.vault.selected_index, 0);
        assert!(app.vault.selected_item().is_some());
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
