use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use zeroize::Zeroizing;

use crate::domain::filter::{ITEM_FILTERS, ItemFilter};
use crate::domain::folder::Folder;
use crate::domain::item::Item;
use crate::ports::{ClipboardPort, SettingsPort};
use crate::tui::action::{ActionState, CmdEntry};
use crate::tui::auto_lock::AutoLock;
use crate::tui::cmd_log::CmdLog;
use crate::tui::generator::GeneratorState;
use crate::tui::glyph::GlyphCaps;
use crate::tui::item_forms::{CreateForm, EditForm};
use crate::tui::login_form::LoginForm;
use crate::tui::mouse_areas::MouseAreas;
use crate::tui::screens::{Focus, LoginField, Screen};
use crate::tui::settings_overlay::{SettingRow, SettingsFocus, SettingsOverlay};
use crate::tui::theme::{self, Theme};
use crate::tui::vault::Vault;
use crate::tui::view::icons::{self, IconSet};
use crate::tui::worker::{InFlight, WorkerRequest, WorkerResponse};

pub const PAGE_STEP: usize = 10;

pub const VAULT_VIEWPORT_ROWS: usize = 20;

fn on_off(b: bool) -> String {
    if b { "On".into() } else { "Off".into() }
}

pub(crate) fn redact_cmd(cmd: &str, marker: Option<&str>) -> String {
    match marker {
        Some(key) if !key.is_empty() => cmd.replace(key, "***"),
        _ => cmd.to_string(),
    }
}

pub struct App {
    pub screen: Screen,
    pub should_quit: bool,
    pub focus: Focus,

    pub vault: Vault,

    pub folders: Vec<Folder>,

    pub collections: Vec<crate::domain::Collection>,

    pub organizations: Vec<crate::domain::Organization>,

    pub import_formats: Vec<String>,

    pub login: LoginForm,

    pub authenticated: bool,

    pub show_password: bool,
    pub detail_field: usize,

    pub cmd_log: CmdLog,

    pub debug_log_path: Option<std::path::PathBuf>,

    pub debug_log_failed: bool,

    pub action_state: ActionState,
    pub action_tick: u8,

    pub in_flight: Option<InFlight>,

    pub request_started: Option<Instant>,

    pub worker_dead: bool,

    pub list_items_timeout: Arc<AtomicU64>,

    pub auto_lock: AutoLock,

    pub clipboard_clear_secs: u64,

    pub mouse_areas: MouseAreas,
    pub last_click: Option<(u16, u16)>,

    pub edit: EditForm,

    pub create: CreateForm,

    pub generator: GeneratorState,

    pub rename_field: Option<crate::tui::flows::items::RenameFieldState>,

    pub folder_name: Option<crate::tui::flows::folders::FolderNameState>,

    pub export: Option<crate::tui::export::ExportState>,

    pub import: Option<crate::tui::import::ImportState>,

    pub attachment_upload: Option<crate::tui::flows::items::AttachmentUploadState>,

    pub attachment_download: Option<crate::tui::flows::items::AttachmentDownloadState>,

    pub attachment_delete: Option<crate::tui::flows::items::AttachmentDeleteState>,

    pub send_create: Option<crate::tui::send::SendCreateState>,

    pub memberships: Option<crate::tui::flows::memberships::MembershipState>,

    pub assign_collections: Option<crate::tui::assign_collections::AssignCollectionsState>,

    pub reprompt: Option<crate::tui::reprompt::RepromptState>,

    pub palette: Option<crate::tui::flows::palette::PaletteState>,

    pub item_actions: Option<crate::tui::item_actions::ItemActionsState>,

    pub reprompt_verified: bool,

    pub help_from: Option<Screen>,

    pub help_scroll: (u16, u16),

    pub theme: Theme,

    pub glyphs: GlyphCaps,

    pub icon_style: String,

    pub icons: IconSet,

    pub settings_ui: SettingsOverlay,

    pub worker_tx: Sender<WorkerRequest>,

    pub worker_rx: Receiver<WorkerResponse>,

    pub session_marker: Option<Zeroizing<String>>,

    pub clipboard: Box<dyn ClipboardPort>,
    pub settings: Box<dyn SettingsPort>,
}

impl App {
    pub fn new(
        worker_tx: Sender<WorkerRequest>,
        worker_rx: Receiver<WorkerResponse>,
        clipboard: Box<dyn ClipboardPort>,
        settings: Box<dyn SettingsPort>,
        list_items_timeout: Arc<AtomicU64>,
    ) -> Self {
        let cfg = settings.read();
        let saved_email = cfg.email.clone().unwrap_or_default();
        let theme = theme::load(&settings.config_dir());
        let glyphs = GlyphCaps::detect();
        let icons = icons::resolve_icons(&cfg.icon_style, glyphs);

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
            debug_log_path: crate::tui::debug_log::enabled_path(),
            debug_log_failed: false,
            action_state: ActionState::Idle,
            action_tick: 0,
            in_flight: None,
            request_started: None,
            worker_dead: false,
            list_items_timeout,
            auto_lock: AutoLock::new(cfg.auto_lock, cfg.lock_after_secs),
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
            item_actions: None,
            reprompt: None,
            palette: None,
            reprompt_verified: false,
            help_from: None,
            help_scroll: (0, 0),
            theme: theme.clone(),
            glyphs,
            icon_style: cfg.icon_style.clone(),
            icons,
            settings_ui: SettingsOverlay {
                focus: SettingsFocus::Sidebar,
                section: 0,
                theme_idx: settings_theme_idx,
                row: 0,
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

    pub fn is_busy(&self) -> bool {
        self.in_flight.is_some()
    }

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

    pub fn watchdog_release_stuck_request(&mut self) {
        let Some(started) = self.request_started else {
            return;
        };
        if self.in_flight.is_none() {
            return;
        }

        let budget = self
            .list_items_timeout
            .load(Ordering::Relaxed)
            .max(180)
            .saturating_add(60);
        if started.elapsed() > Duration::from_secs(budget) {
            self.in_flight = None;
            self.request_started = None;
            self.set_action(ActionState::Error(
                "request got no response in time — released".into(),
            ));
            self.push_cmd("worker watchdog", false, "abandoned in-flight request");
        }
    }

    pub fn open_settings(&mut self) {
        self.settings_ui.from = self.screen.clone();
        self.settings_ui.theme_before = self.theme.clone();
        self.settings_ui.focus = SettingsFocus::Sidebar;
        self.settings_ui.section = 0;
        self.settings_ui.row = 0;
        self.screen = Screen::Settings;
    }

    pub fn settings_row_value(&self, row: SettingRow) -> String {
        match row {
            SettingRow::AutoLock => on_off(self.auto_lock.enabled),
            SettingRow::LockAfter => format!("{} min", self.auto_lock.after_secs / 60),
            SettingRow::KeepSession => on_off(self.login.keep_session),
            SettingRow::RememberEmail => on_off(self.login.save_email),
            SettingRow::ClipboardClear => {
                if self.clipboard_clear_secs == 0 {
                    "Off".into()
                } else {
                    format!("{} s", self.clipboard_clear_secs)
                }
            }
            SettingRow::ListTimeout => {
                format!("{} s", self.list_items_timeout.load(Ordering::Relaxed))
            }
            SettingRow::IconStyle => {
                if self.icon_style.eq_ignore_ascii_case("nerd") {
                    "Nerd".into()
                } else {
                    "Unicode".into()
                }
            }
        }
    }

    pub fn settings_adjust(&mut self, row: SettingRow, forward: bool) {
        let step = |cur: u64, by: i64, lo: u64, hi: u64| -> u64 {
            (cur as i64 + if forward { by } else { -by }).clamp(lo as i64, hi as i64) as u64
        };
        match row {
            SettingRow::AutoLock => {
                self.auto_lock.enabled = !self.auto_lock.enabled;
                self.settings.write_auto_lock(self.auto_lock.enabled);
            }
            SettingRow::KeepSession => self.toggle_keep_session(),
            SettingRow::RememberEmail => self.toggle_save_email(),
            SettingRow::LockAfter => {
                let mins = step(self.auto_lock.after_secs / 60, 1, 1, 240);
                self.auto_lock.after_secs = mins * 60;
                self.settings
                    .write_lock_after_secs(self.auto_lock.after_secs);
            }
            SettingRow::ClipboardClear => {
                self.clipboard_clear_secs = step(self.clipboard_clear_secs, 5, 0, 600);
                self.settings
                    .write_clipboard_clear_secs(self.clipboard_clear_secs);
            }
            SettingRow::ListTimeout => {
                let secs = step(self.list_items_timeout.load(Ordering::Relaxed), 30, 30, 900);

                self.list_items_timeout.store(secs, Ordering::Relaxed);
                self.settings.write_list_items_timeout_secs(secs);
            }
            SettingRow::IconStyle => {
                self.icon_style = if self.icon_style.eq_ignore_ascii_case("nerd") {
                    "unicode".to_string()
                } else {
                    "nerd".to_string()
                };
                self.icons = icons::resolve_icons(&self.icon_style, self.glyphs);
                self.settings.write_icon_style(&self.icon_style);
            }
        }
    }

    pub fn settings_preview_theme(&mut self) {
        if let Some(&p) = theme::Preset::ALL.get(self.settings_ui.theme_idx) {
            self.theme = theme::adapt(
                Theme::from_palette(&p.palette()),
                theme::ColorCaps::detect(),
            );
        }
    }

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
                self.screen = self.help_from.take().unwrap_or(Screen::Vault);
            }
            Screen::Create | Screen::ConfirmDelete => {
                self.screen = Screen::Vault;
            }
            _ => {}
        }
    }

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

    pub fn push_cmd(&mut self, cmd: &str, ok: bool, detail: &(impl std::fmt::Display + ?Sized)) {
        let detail = detail.to_string();

        let redacted = redact_cmd(cmd, self.session_marker.as_deref().map(|s| s.as_str()));
        let write_err = match &self.debug_log_path {
            Some(path) if !self.debug_log_failed => {
                crate::tui::debug_log::append(path, &redacted, ok, &detail)
                    .err()
                    .map(|e| format!("could not write {}: {e}", path.display()))
            }
            _ => None,
        };
        self.cmd_log.push(CmdEntry {
            cmd: redacted,
            ok,
            detail,
        });
        if let Some(detail) = write_err {
            self.debug_log_failed = true;
            self.cmd_log.push(CmdEntry {
                cmd: "debug log".to_string(),
                ok: false,
                detail,
            });
        }
    }

    pub fn set_action(&mut self, state: ActionState) {
        self.action_state = state;
        self.action_tick = 0;
    }
    pub fn tick_action(&mut self) {
        self.action_tick = self.action_tick.wrapping_add(1);
    }

    pub fn cmd_err(&mut self, cmd: &str, e: &(impl std::fmt::Display + ?Sized), label: &str) {
        let e = e.to_string();
        self.push_cmd(cmd, false, &e);
        self.set_action(ActionState::Error(format!("{label}: {e}")));
    }

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

    pub fn toggle_keep_session(&mut self) {
        self.login.keep_session = !self.login.keep_session;
        self.settings.write_keep_session(self.login.keep_session);
        if !self.login.keep_session {
            crate::tui::session_file::clear();
        }
    }

    pub fn detail_field_count(&self) -> usize {
        let Some(item) = self.vault.selected_item() else {
            return 0;
        };
        crate::tui::detail_fields::build_detail_fields(item, false, 0).len()
    }
}

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

        if let Some(rest) = query.strip_prefix("url:") {
            let needle = rest.trim();
            if needle.is_empty() {
                return indices;
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
        fn write_lock_after_secs(&self, _: u64) {}
        fn write_clipboard_clear_secs(&self, _: u64) {}
        fn write_list_items_timeout_secs(&self, _: u64) {}
        fn write_icon_style(&self, _: &str) {}
        fn write_theme_name(&self, _: &str) {}
        fn config_dir(&self) -> std::path::PathBuf {
            std::path::PathBuf::from(".")
        }
    }

    fn fresh_app() -> (App, Receiver<WorkerRequest>, Sender<WorkerResponse>) {
        let (worker_tx, req_rx) = channel::<WorkerRequest>();
        let (resp_tx, worker_rx) = channel::<WorkerResponse>();
        let app = App::new(
            worker_tx,
            worker_rx,
            Box::new(NoopClipboard),
            Box::new(DefaultSettings),
            Arc::new(AtomicU64::new(180)),
        );
        (app, req_rx, resp_tx)
    }

    #[test]
    fn begin_enforces_single_in_flight() {
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        assert!(app.begin(InFlight::LoadItems));
        assert!(app.is_busy());

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
    fn clicking_outside_an_overlay_dismisses_it() {
        use crate::tui::screens::Screen;
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.screen = Screen::ConfirmLogout;

        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| crate::tui::view::draw(f, &mut app)).unwrap();

        crate::tui::input::mouse::handle(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_ne!(
            app.screen,
            Screen::ConfirmLogout,
            "a click outside the overlay dismissed it"
        );
    }

    #[test]
    fn clicking_the_help_anchor_opens_help() {
        use crate::tui::screens::Screen;
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.screen = Screen::Vault;
        let mut term = Terminal::new(TestBackend::new(90, 24)).unwrap();
        term.draw(|f| crate::tui::view::draw(f, &mut app)).unwrap();

        let buf = term.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                if let Some(c) = buf.cell((x, y)) {
                    text.push_str(c.symbol());
                }
            }
            text.push('\n');
        }
        let (col, row) = text
            .lines()
            .enumerate()
            .find_map(|(y, line)| {
                line.find("F1 help")
                    .map(|b| (line[..b].chars().count() as u16, y as u16))
            })
            .expect("the F1 help anchor is rendered");
        crate::tui::input::mouse::handle(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: col + 1,
                row,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(
            app.screen,
            Screen::Help,
            "clicking the F1 anchor opened help"
        );
    }

    #[test]
    fn clicking_a_detail_field_card_focuses_it() {
        use crate::tui::screens::Screen;
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.vault.items = vec![login_item("a", "Acct", "user@example.com")];
        app.vault.rebuild_caches();
        app.vault.selected_index = 0;
        app.screen = Screen::Detail;
        app.detail_field = 0;
        let mut term = Terminal::new(TestBackend::new(90, 40)).unwrap();
        term.draw(|f| crate::tui::view::draw(f, &mut app)).unwrap();

        let buf = term.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                if let Some(c) = buf.cell((x, y)) {
                    text.push_str(c.symbol());
                }
            }
            text.push('\n');
        }
        let (col, row) = text
            .lines()
            .enumerate()
            .find_map(|(y, line)| {
                line.find("Username")
                    .map(|b| (line[..b].chars().count() as u16, y as u16))
            })
            .expect("the Username field card is rendered");
        crate::tui::input::mouse::handle(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: col,
                row,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(
            app.detail_field > 0,
            "clicking the Username card moved the field cursor off the first field"
        );
    }

    #[test]
    fn clicking_a_login_field_focuses_it() {
        use crate::tui::screens::{LoginField, Screen};
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.screen = Screen::Login;
        app.login.active_field = LoginField::Server;
        let mut term = Terminal::new(TestBackend::new(90, 30)).unwrap();
        term.draw(|f| crate::tui::view::draw(f, &mut app)).unwrap();

        let buf = term.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                if let Some(c) = buf.cell((x, y)) {
                    text.push_str(c.symbol());
                }
            }
            text.push('\n');
        }
        let (col, row) = text
            .lines()
            .enumerate()
            .find_map(|(y, line)| {
                line.find("Master Password:")
                    .map(|b| (line[..b].chars().count() as u16, y as u16))
            })
            .expect("the password label is rendered");
        crate::tui::input::mouse::handle(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: col,
                row,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(
            app.login.active_field,
            LoginField::Password,
            "clicking the password label focused the password field"
        );
    }

    #[test]
    fn clicking_a_settings_sidebar_section_selects_it() {
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.open_settings();
        assert_eq!(
            app.settings_ui.section, 0,
            "settings open on the first section"
        );
        let mut term = Terminal::new(TestBackend::new(90, 24)).unwrap();
        term.draw(|f| crate::tui::view::draw(f, &mut app)).unwrap();

        let buf = term.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                if let Some(c) = buf.cell((x, y)) {
                    text.push_str(c.symbol());
                }
            }
            text.push('\n');
        }
        let (col, row) = text
            .lines()
            .enumerate()
            .find_map(|(y, line)| {
                line.find("Security")
                    .map(|b| (line[..b].chars().count() as u16, y as u16))
            })
            .expect("the Security section row is rendered");
        crate::tui::input::mouse::handle(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: col,
                row,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(
            app.settings_ui.section, 1,
            "clicking the Security sidebar row selected it"
        );
    }

    #[test]
    fn submit_dispatches_toast_and_request() {
        let (mut app, req_rx, _resp_tx) = fresh_app();
        assert!(app.submit(InFlight::Sync, "Syncing…", WorkerRequest::Sync));
        assert!(app.is_busy());
        assert!(matches!(app.action_state, ActionState::Running(_)));
        assert!(app.request_started.is_some());

        assert!(matches!(req_rx.try_recv(), Ok(WorkerRequest::Sync)));
    }

    #[test]
    fn submit_on_dead_channel_marks_worker_dead() {
        let (mut app, req_rx, _resp_tx) = fresh_app();
        drop(req_rx);
        assert!(!app.submit(InFlight::Sync, "Syncing…", WorkerRequest::Sync));
        assert!(app.worker_dead);
        assert!(app.in_flight.is_none());
    }

    #[test]
    fn watchdog_leaves_a_fresh_request_alone() {
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.submit(InFlight::Sync, "Syncing…", WorkerRequest::Sync);

        app.watchdog_release_stuck_request();
        assert!(app.is_busy());
    }

    #[test]
    fn command_palette_filters_moves_and_cancels() {
        use crate::tui::flows::palette;
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.screen = Screen::Vault;
        palette::open(&mut app);

        let total = app.palette.as_ref().unwrap().all.len();
        assert!(total >= 11, "expected the app-wide commands, got {total}");
        assert_eq!(app.screen, Screen::CommandPalette);

        app.palette.as_mut().unwrap().query.insert_str("sync");
        palette::rebuild_filter(&mut app);
        assert_eq!(app.palette.as_ref().unwrap().filtered.len(), 1);

        palette::move_selection(&mut app, 10);
        assert_eq!(app.palette.as_ref().unwrap().selected, 0);

        palette::cancel(&mut app);
        assert!(app.palette.is_none());
        assert_eq!(app.screen, Screen::Vault);
    }

    #[test]
    fn the_settings_panel_describes_the_focused_row() {
        use crate::tui::screens::Screen;
        use crate::tui::settings_overlay::{SettingsFocus, SettingsSection};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.screen = Screen::Vault;
        app.open_settings();
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let render = |app: &mut App, term: &mut Terminal<TestBackend>| -> String {
            term.draw(|f| crate::tui::view::draw(f, app)).unwrap();
            let buf = term.backend().buffer();
            let mut text = String::new();
            for y in 0..buf.area().height {
                for x in 0..buf.area().width {
                    if let Some(c) = buf.cell((x, y)) {
                        text.push_str(c.symbol());
                    }
                }
                text.push('\n');
            }
            text
        };

        let security = SettingsSection::ALL
            .iter()
            .position(|s| *s == SettingsSection::Security)
            .expect("a Security section");
        app.settings_ui.section = security;
        app.settings_ui.focus = SettingsFocus::Panel;
        app.settings_ui.row = 0;
        let rows = SettingsSection::Security.rows();
        assert!(render(&mut app, &mut term).contains(rows[0].hint()));

        app.settings_ui.row = 1;
        let moved = render(&mut app, &mut term);
        assert!(moved.contains(rows[1].hint()), "it tracks the selection");
        assert!(
            !moved.contains(rows[0].hint()),
            "and drops the previous one"
        );

        app.settings_ui.section = SettingsSection::ALL
            .iter()
            .position(|s| *s == SettingsSection::Theme)
            .expect("a Theme section");
        let theme_panel = render(&mut app, &mut term);
        for row in SettingsSection::Security.rows() {
            assert!(!theme_panel.contains(row.hint()), "no stale description");
        }
        assert!(theme_panel.contains("apply+save"), "the legend survives");
    }

    #[test]
    fn indicator_markers_keep_the_type_column_aligned() {
        use crate::tui::screens::Screen;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (mut app, _req_rx, _resp_tx) = fresh_app();

        let mut guarded = login_item("a", "Guarded", "a@example.com");
        guarded.reprompt = 1;
        app.vault.items = vec![guarded, login_item("b", "Plain", "b@example.com")];
        app.vault.rebuild_caches();
        app.screen = Screen::Vault;

        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| crate::tui::view::draw(f, &mut app)).unwrap();
        let buf = term.backend().buffer();
        let mut rows: Vec<String> = Vec::new();
        for y in 0..buf.area().height {
            let mut line = String::new();
            for x in 0..buf.area().width {
                if let Some(c) = buf.cell((x, y)) {
                    line.push_str(c.symbol());
                }
            }
            rows.push(line);
        }

        let col_of = |name: &str| -> usize {
            let row = rows
                .iter()
                .find(|r| r.contains(name))
                .unwrap_or_else(|| panic!("{name} is rendered"));
            row.find("[Login]")
                .map(|b| row[..b].chars().count())
                .unwrap_or_else(|| panic!("{name} has a type tag"))
        };
        assert_eq!(
            col_of("Guarded"),
            col_of("Plain"),
            "the type tag must start at the same column with and without an indicator"
        );
    }

    #[test]
    fn the_hint_bar_writes_shortcuts_in_the_host_convention() {
        use crate::tui::keyboard::{self, Keyboard};
        use crate::tui::screens::{Focus, Screen};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.screen = Screen::Vault;
        app.focus = Focus::Search;
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| crate::tui::view::draw(f, &mut app)).unwrap();
        let buf = term.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                if let Some(c) = buf.cell((x, y)) {
                    text.push_str(c.symbol());
                }
            }
            text.push('\n');
        }

        let expected = keyboard::label("Alt+N").into_owned();
        assert!(
            text.contains(&expected),
            "the hint bar should print {expected:?}"
        );
        match keyboard::host() {
            Keyboard::Mac => {
                assert_eq!(expected, "⌥N");
                assert!(!text.contains("Alt+N"), "the PC spelling must not leak");
            }
            Keyboard::Pc => assert_eq!(expected, "Alt+N"),
        }
    }

    #[test]
    fn the_palette_reaches_the_form_verbs_that_only_have_an_alt_chord() {
        use crate::tui::flows::palette::palette_commands;
        use crate::tui::screens::Screen;

        let labels = |app: &App| -> Vec<&'static str> {
            palette_commands(app).into_iter().map(|c| c.label).collect()
        };

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.vault.items = vec![login_item("a", "Acct", "user@example.com")];
        app.vault.rebuild_caches();

        app.screen = Screen::Detail;
        crate::tui::flows::items::enter_edit_mode(&mut app);
        assert!(app.edit.active);
        let edit = labels(&app);
        for expected in [
            "Add custom field",
            "Add URL row",
            "Rename custom field",
            "Cycle field type",
            "Assign collections",
            "Remove field / URL row",
        ] {
            assert!(edit.contains(&expected), "edit mode is missing {expected}");
        }
        assert!(!edit.contains(&"Lock vault"), "a form owns its screen");
        assert!(!edit.contains(&"Sync vault"), "a form owns its screen");

        let name_idx = app
            .edit
            .fields
            .iter()
            .position(|f| f.label == "Name")
            .expect("every item has a Name row");
        app.edit.field_idx = name_idx;
        assert!(!labels(&app).contains(&"Generate into this field"));
        let pw_idx = app
            .edit
            .fields
            .iter()
            .position(|f| f.hidden)
            .expect("a login has a hidden row");
        app.edit.field_idx = pw_idx;
        assert!(labels(&app).contains(&"Generate into this field"));

        app.edit.active = false;
        app.screen = Screen::Generator;
        app.generator = crate::tui::generator::GeneratorState::default();
        let standalone = labels(&app);
        assert!(standalone.contains(&"Copy result"));
        assert!(!standalone.contains(&"Use result in form"));
        app.generator.return_target = Some(crate::tui::generator::ReturnTarget::EditField(0));
        assert!(labels(&app).contains(&"Use result in form"));
    }

    #[test]
    fn a_click_while_busy_is_swallowed_like_a_key() {
        use crate::tui::screens::Screen;
        use crate::tui::worker::InFlight;
        use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.vault.items = vec![
            login_item("a", "Alpha", "a@example.com"),
            login_item("b", "Bravo", "b@example.com"),
        ];
        app.vault.rebuild_caches();
        app.screen = Screen::Vault;

        let mut term = Terminal::new(TestBackend::new(90, 30)).unwrap();
        term.draw(|f| crate::tui::view::draw(f, &mut app)).unwrap();

        let buf = term.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                if let Some(c) = buf.cell((x, y)) {
                    text.push_str(c.symbol());
                }
            }
            text.push('\n');
        }
        let (col, row) = text
            .lines()
            .enumerate()
            .find_map(|(y, line)| {
                line.find("Bravo")
                    .map(|b| (line[..b].chars().count() as u16, y as u16))
            })
            .expect("the second row is rendered");
        let click = Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        });

        assert!(app.begin(InFlight::Sync), "claim the in-flight slot");
        crate::tui::input::handle_events(&mut app, click.clone());
        assert_eq!(app.vault.selected_index, 0, "a click while busy is inert");

        app.in_flight = None;
        crate::tui::input::handle_events(&mut app, click);
        assert_eq!(app.vault.selected_index, 1, "and lands once idle");
    }

    #[test]
    fn settings_overlay_is_centered_and_bounded_at_both_extremes() {
        use crate::tui::screens::Screen;
        use crate::tui::view::widgets::active_modal_rect;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let rect_for = |w: u16, h: u16| {
            let (mut app, _req_rx, _resp_tx) = fresh_app();
            app.screen = Screen::Vault;
            app.open_settings();
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| crate::tui::view::draw(f, &mut app)).unwrap();
            active_modal_rect().expect("settings registers its modal rect")
        };

        for (w, h) in [(80u16, 24u16), (200, 60), (62, 20)] {
            let r = rect_for(w, h);
            assert!(
                r.width <= w && r.height <= h,
                "{w}x{h}: overflows the frame"
            );
            assert!(
                r.x + r.width <= w && r.y + r.height <= h,
                "{w}x{h}: runs off the frame"
            );

            let (left, right) = (r.x, w - (r.x + r.width));
            assert!(left.abs_diff(right) <= 1, "{w}x{h}: not centered");
        }

        assert_eq!(rect_for(200, 60).width, 72);

        assert_eq!(rect_for(62, 20).width, 56);
    }

    #[test]
    fn sidebar_lists_cue_their_overflow() {
        use crate::domain::Folder;
        use crate::tui::screens::Screen;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let render = |app: &mut App, w: u16, h: u16| -> String {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| crate::tui::view::draw(f, app)).unwrap();
            let buf = term.backend().buffer();
            let mut text = String::new();
            for y in 0..buf.area().height {
                for x in 0..buf.area().width {
                    if let Some(c) = buf.cell((x, y)) {
                        text.push_str(c.symbol());
                    }
                }
                text.push('\n');
            }
            text
        };

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.screen = Screen::Vault;
        app.folders = (0..3)
            .map(|i| Folder {
                id: format!("f{i}"),
                name: format!("Folder-{i}"),
            })
            .collect();
        assert!(!render(&mut app, 90, 40).contains('█'), "nothing overflows");

        app.folders = (0..30)
            .map(|i| Folder {
                id: format!("f{i}"),
                name: format!("Folder-{i:02}"),
            })
            .collect();
        assert!(
            render(&mut app, 90, 40).contains('█'),
            "an overflowing Folders sidebar draws its scrollbar"
        );
    }

    #[test]
    fn command_log_teaches_when_empty_and_cues_its_overflow() {
        use crate::tui::screens::Screen;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.screen = Screen::Vault;
        let mut term = Terminal::new(TestBackend::new(90, 30)).unwrap();
        let render = |app: &mut App, term: &mut Terminal<TestBackend>| -> String {
            term.draw(|f| crate::tui::view::draw(f, app)).unwrap();
            let buf = term.backend().buffer();
            let mut text = String::new();
            for y in 0..buf.area().height {
                for x in 0..buf.area().width {
                    if let Some(c) = buf.cell((x, y)) {
                        text.push_str(c.symbol());
                    }
                }
                text.push('\n');
            }
            text
        };

        let empty = render(&mut app, &mut term);
        assert!(empty.contains("No commands yet"), "headline");

        let sync_key = crate::tui::keyboard::label("Alt+S").into_owned();
        assert!(
            empty.contains(&sync_key),
            "the empty state names a key ({sync_key})"
        );
        assert!(!empty.contains("no commands yet"), "the bare line is gone");

        assert!(!empty.contains('█'), "nothing overflows yet");

        for i in 0..40 {
            app.push_cmd(&format!("bw cmd {i}"), true, "ok");
        }
        assert!(
            render(&mut app, &mut term).contains('█'),
            "an overflowing command log draws its scrollbar"
        );
    }

    #[test]
    fn command_log_reports_its_scrollback_in_the_shared_counter() {
        use crate::tui::screens::Screen;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.screen = Screen::Vault;
        for i in 0..12 {
            app.push_cmd(&format!("bw cmd {i}"), true, "ok");
        }
        let mut term = Terminal::new(TestBackend::new(90, 30)).unwrap();
        let render = |app: &mut App, term: &mut Terminal<TestBackend>| -> String {
            term.draw(|f| crate::tui::view::draw(f, app)).unwrap();
            let buf = term.backend().buffer();
            let mut text = String::new();
            for y in 0..buf.area().height {
                for x in 0..buf.area().width {
                    if let Some(c) = buf.cell((x, y)) {
                        text.push_str(c.symbol());
                    }
                }
                text.push('\n');
            }
            text
        };

        assert!(render(&mut app, &mut term).contains("12 of 12"));

        app.cmd_log.scroll_up(3);
        let scrolled = render(&mut app, &mut term);
        assert!(
            scrolled.contains("9 of 12"),
            "counter tracks the scrollback"
        );
        assert!(!scrolled.contains("↑3"), "the faked in-title tag is gone");
    }

    #[test]
    fn memberships_scrolls_instead_of_losing_rows_past_the_fold() {
        use crate::domain::{Collection, Organization};
        use crate::tui::flows::memberships::{MembershipState, move_cursor};
        use crate::tui::screens::Screen;
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        const N: usize = 40;
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.memberships = Some(MembershipState {
            organizations: vec![Organization {
                id: "o1".into(),
                name: "Acme".into(),
            }],
            collections: (0..N)
                .map(|i| Collection {
                    id: format!("c{i}"),
                    name: format!("Collection-{i:02}"),
                    organization_id: Some("o1".into()),
                })
                .collect(),
            cursor: 0,
        });
        app.screen = Screen::Memberships;

        let mut term = Terminal::new(TestBackend::new(90, 30)).unwrap();
        let render = |app: &mut App, term: &mut Terminal<TestBackend>| -> String {
            term.draw(|f| crate::tui::view::draw(f, app)).unwrap();
            let buf = term.backend().buffer();
            let mut text = String::new();
            for y in 0..buf.area().height {
                for x in 0..buf.area().width {
                    if let Some(c) = buf.cell((x, y)) {
                        text.push_str(c.symbol());
                    }
                }
                text.push('\n');
            }
            text
        };

        let last = format!("Collection-{:02}", N - 1);
        let first = "Collection-00";
        let top = render(&mut app, &mut term);
        assert!(top.contains(first), "the list starts at the top");
        assert!(
            !top.contains(&last),
            "the tail is past the fold before scrolling"
        );

        crate::tui::input::memberships::handle(
            &mut app,
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
        );
        let bottom = render(&mut app, &mut term);
        assert!(bottom.contains(&last), "the tail is reachable by scrolling");

        assert_eq!(app.memberships.as_ref().unwrap().cursor, N - 1);
        move_cursor(&mut app, 1);
        assert_eq!(app.memberships.as_ref().unwrap().cursor, N - 1);
        for _ in 0..N + 5 {
            move_cursor(&mut app, -1);
        }
        assert_eq!(app.memberships.as_ref().unwrap().cursor, 0);
    }

    #[test]
    fn palette_rows_mirror_their_keybinding_guards() {
        use crate::tui::flows::palette::palette_commands;
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        let labels = |app: &App| -> Vec<&'static str> {
            palette_commands(app).into_iter().map(|c| c.label).collect()
        };

        app.vault.items = vec![item("a", "GitHub", 1, None)];
        app.vault.rebuild_caches();
        let vault = labels(&app);
        for expected in ["New item", "Sync vault", "Copy password", "Delete item"] {
            assert!(
                vault.contains(&expected),
                "vault view is missing {expected}"
            );
        }
        assert!(!vault.contains(&"Restore item"), "restore is trash-only");

        app.vault.active_filter = ItemFilter::Trash;
        app.vault.trashed_items = vec![item("b", "Old", 1, None)];
        app.vault.rebuild_caches();
        let trash = labels(&app);
        for refused in ["New item", "Sync vault", "Copy password", "Edit item"] {
            assert!(!trash.contains(&refused), "{refused} is refused in trash");
        }

        assert!(trash.contains(&"Restore item"));
        assert!(trash.contains(&"Delete item"));

        app.vault.trashed_items.clear();
        app.vault.rebuild_caches();
        let empty = labels(&app);
        assert!(!empty.contains(&"Restore item"));
        assert!(!empty.contains(&"Delete item"));
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
        app.vault.selected_index = 1;
        assert_eq!(app.vault.selected_item_id().as_deref(), Some("b"));

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
        app.vault.selected_index = 1;

        app.vault.items = vec![item("a", "A", 1, None)];
        app.vault.rebuild_caches();
        app.vault.reanchor_selection(Some("b"));
        assert_eq!(app.vault.selected_index, 0);
        assert!(app.vault.selected_item().is_some());
    }

    #[test]
    fn deleting_under_an_active_search_clamps_against_the_filtered_list() {
        use crate::tui::flows::items;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.vault.items = vec![
            item("a1", "alpha one", 1, None),
            item("a2", "alpha two", 1, None),
            item("z", "zeta", 1, None),
            item("o", "omega", 1, None),
            item("k", "kappa", 1, None),
        ];
        app.vault.search_query.set("alpha");
        app.vault.rebuild_caches();

        assert_eq!(app.vault.filtered_items().len(), 2);
        app.vault.selected_index = 1;
        assert_eq!(app.vault.selected_item_id().as_deref(), Some("a2"));

        items::handle_delete(&mut app, false, "a2".into(), "alpha two".into(), Ok(()));

        assert_eq!(app.vault.filtered_items().len(), 1);
        assert_eq!(app.vault.selected_index, 0);
        assert_eq!(app.vault.selected_item_id().as_deref(), Some("a1"));
    }

    #[test]
    fn deleting_the_only_search_match_leaves_the_cursor_parked_at_zero() {
        use crate::tui::flows::items;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.vault.items = vec![
            item("a1", "alpha one", 1, None),
            item("z", "zeta", 1, None),
            item("o", "omega", 1, None),
        ];
        app.vault.search_query.set("alpha");
        app.vault.rebuild_caches();
        assert_eq!(app.vault.filtered_items().len(), 1);

        items::handle_delete(&mut app, true, "a1".into(), "alpha one".into(), Ok(()));

        assert!(app.vault.filtered_items().is_empty());
        assert_eq!(app.vault.selected_index, 0);
        assert!(app.vault.selected_item().is_none());

        assert_eq!(app.vault.items.len(), 2);
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
    fn item_actions_reflect_the_item_and_view() {
        use crate::tui::item_actions::{ItemAction, actions_for};

        let mut full = login_item("a", "Acct", "user@example.com");
        {
            let l = full.login.as_mut().unwrap();
            l.password = Some("pw".into());
            l.totp = Some("otpauth://x".into());
        }
        let acts = actions_for(&full, false, false);
        assert!(acts.contains(&ItemAction::CopyUsername));
        assert!(acts.contains(&ItemAction::CopyPassword));
        assert!(acts.contains(&ItemAction::CopyTotp));
        assert!(acts.contains(&ItemAction::Edit));
        assert!(acts.contains(&ItemAction::ToggleFavorite));
        assert!(acts.contains(&ItemAction::Delete));

        assert!(!acts.contains(&ItemAction::Move));
        assert!(actions_for(&full, false, true).contains(&ItemAction::Move));

        let note = item("n", "Note", 2, None);
        let note_acts = actions_for(&note, false, false);
        assert!(!note_acts.contains(&ItemAction::CopyUsername));
        assert!(!note_acts.contains(&ItemAction::CopyPassword));
        assert!(!note_acts.contains(&ItemAction::CopyTotp));

        let user_only = login_item("u", "UserOnly", "u@x");
        let uo = actions_for(&user_only, false, false);
        assert!(!uo.contains(&ItemAction::CopyPassword));
        assert!(!uo.contains(&ItemAction::CopyTotp));

        assert_eq!(
            actions_for(&full, true, true),
            vec![ItemAction::Open, ItemAction::Restore, ItemAction::Delete]
        );
    }

    #[test]
    fn right_clicking_a_vault_row_opens_its_action_menu() {
        use crate::tui::screens::Screen;
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.vault.items = vec![login_item("a", "Acct", "user@example.com")];
        app.vault.rebuild_caches();
        app.vault.selected_index = 0;
        app.screen = Screen::Vault;
        let mut term = Terminal::new(TestBackend::new(90, 30)).unwrap();
        term.draw(|f| crate::tui::view::draw(f, &mut app)).unwrap();

        let buf = term.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                if let Some(c) = buf.cell((x, y)) {
                    text.push_str(c.symbol());
                }
            }
            text.push('\n');
        }
        let (col, row) = text
            .lines()
            .enumerate()
            .find_map(|(y, line)| {
                line.find("Acct")
                    .map(|b| (line[..b].chars().count() as u16, y as u16))
            })
            .expect("the item row is rendered");
        crate::tui::input::mouse::handle(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Right),
                column: col,
                row,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(
            app.screen,
            Screen::ItemActions,
            "right-click opened the menu"
        );
        assert!(app.item_actions.is_some(), "menu state is populated");
    }

    #[test]
    fn action_menu_stays_compact_and_its_rows_stay_clickable() {
        use crate::tui::screens::Screen;
        use crate::tui::view::widgets::{MODAL_WIDTH_PCT, active_modal_rect};
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        const W: u16 = 90;
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.vault.items = vec![login_item("a", "Acct", "user@example.com")];
        app.vault.rebuild_caches();
        app.vault.selected_index = 0;
        app.screen = Screen::Vault;
        crate::tui::flows::item_actions::open(&mut app);
        assert_eq!(app.screen, Screen::ItemActions);
        let action_rows = app.item_actions.as_ref().unwrap().actions.len() as u16;

        let mut term = Terminal::new(TestBackend::new(W, 30)).unwrap();
        term.draw(|f| crate::tui::view::draw(f, &mut app)).unwrap();

        let rect = active_modal_rect().expect("the menu registers its modal rect");
        assert!(
            rect.width < W * MODAL_WIDTH_PCT / 100,
            "a context menu must not inherit the picker band ({} cols)",
            rect.width
        );
        assert!(
            rect.height <= action_rows + 3,
            "height is the rows plus borders + legend, got {}",
            rect.height
        );

        let buf = term.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                if let Some(c) = buf.cell((x, y)) {
                    text.push_str(c.symbol());
                }
            }
            text.push('\n');
        }
        let (col, row) = text
            .lines()
            .enumerate()
            .find_map(|(y, line)| {
                line.find("Open")
                    .map(|b| (line[..b].chars().count() as u16, y as u16))
            })
            .expect("the Open row is rendered");
        crate::tui::input::mouse::handle(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: col,
                row,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(app.screen, Screen::Detail, "the clicked row ran its action");
        assert!(app.item_actions.is_none(), "the menu closed behind it");
    }

    #[test]
    fn running_open_from_the_action_menu_goes_to_detail() {
        use crate::tui::flows::item_actions;
        use crate::tui::screens::Screen;

        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.vault.items = vec![login_item("a", "Acct", "user@example.com")];
        app.vault.rebuild_caches();
        app.vault.selected_index = 0;
        app.screen = Screen::Vault;
        item_actions::open(&mut app);
        assert_eq!(app.screen, Screen::ItemActions);

        item_actions::run_selected(&mut app);
        assert_eq!(app.screen, Screen::Detail, "Open navigated to detail");
        assert!(app.item_actions.is_none(), "menu state was cleared");
    }

    #[test]
    fn clicking_a_collection_row_toggles_it() {
        use crate::tui::assign_collections::{AssignCollectionsPurpose, AssignCollectionsState};
        use crate::tui::screens::Screen;
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let coll = |id: &str, name: &str| crate::domain::Collection {
            id: id.into(),
            name: name.into(),
            organization_id: Some("o1".into()),
        };
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.assign_collections = Some(AssignCollectionsState::new(
            vec![coll("c1", "EngTeamColl"), coll("c2", "OpsTeamColl")],
            &[],
            0,
            Screen::Create,
            AssignCollectionsPurpose::UpdateField,
        ));
        app.screen = Screen::AssignCollections;
        let mut term = Terminal::new(TestBackend::new(90, 30)).unwrap();
        term.draw(|f| crate::tui::view::draw(f, &mut app)).unwrap();
        let buf = term.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                if let Some(c) = buf.cell((x, y)) {
                    text.push_str(c.symbol());
                }
            }
            text.push('\n');
        }
        let (col, row) = text
            .lines()
            .enumerate()
            .find_map(|(y, line)| {
                line.find("OpsTeamColl")
                    .map(|b| (line[..b].chars().count() as u16, y as u16))
            })
            .expect("the OpsTeamColl row is rendered");
        crate::tui::input::mouse::handle(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: col,
                row,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(
            app.assign_collections
                .as_ref()
                .unwrap()
                .selected
                .contains("c2"),
            "clicking the OpsTeamColl row checked it"
        );
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
        let trashed = vec![item("a", "x", 1, Some("F1")), item("b", "y", 1, None)];
        let l = lowered(&trashed);
        let idx = compute_filtered_indices(
            &trashed,
            &l,
            &ItemFilter::Trash,
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

        assert_eq!(idx, vec![0, 1]);
    }

    #[test]
    fn url_prefix_skips_fuzzy_ranking() {
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

        let idx = compute_filtered_indices(
            &items,
            &l,
            &ItemFilter::All,
            &FolderFilter::Collection("c1".into()),
            "",
        );
        assert_eq!(idx, vec![0, 1]);

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

    #[test]
    fn settings_adjust_toggles_bools_and_steps_numbers() {
        use crate::tui::settings_overlay::SettingRow;
        let (mut app, _rx, _tx) = fresh_app();

        let before = app.auto_lock.enabled;
        app.settings_adjust(SettingRow::AutoLock, true);
        assert_eq!(app.auto_lock.enabled, !before);
        assert_eq!(
            app.settings_row_value(SettingRow::AutoLock),
            if !before { "On" } else { "Off" }
        );

        app.auto_lock.after_secs = 5 * 60;
        app.settings_adjust(SettingRow::LockAfter, true);
        assert_eq!(app.auto_lock.after_secs, 6 * 60);
        assert_eq!(app.settings_row_value(SettingRow::LockAfter), "6 min");
        app.auto_lock.after_secs = 60;
        app.settings_adjust(SettingRow::LockAfter, false);
        assert_eq!(app.auto_lock.after_secs, 60);

        app.clipboard_clear_secs = 5;
        app.settings_adjust(SettingRow::ClipboardClear, false);
        assert_eq!(app.clipboard_clear_secs, 0);
        assert_eq!(app.settings_row_value(SettingRow::ClipboardClear), "Off");
    }

    #[test]
    fn copy_toast_says_when_the_clipboard_cannot_auto_clear() {
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        app.clipboard_clear_secs = 30;
        crate::tui::flows::copy::copy_raw(&mut app, "s3cret".into(), "Value copied ✓");
        match &app.action_state {
            ActionState::Done(msg) => {
                assert!(msg.contains("won't auto-clear"), "{msg}");
                assert!(!msg.contains("clears in"), "{msg}");
            }
            other => panic!("expected a done toast, got {other:?}"),
        }
    }

    fn session_env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::tui::session_file::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn unlocked_app_gone_idle() -> (App, Receiver<WorkerRequest>, Sender<WorkerResponse>) {
        let (mut app, req_rx, resp_tx) = fresh_app();
        app.authenticated = true;
        app.vault.items = vec![login_item("a", "Alpha", "a@example.com")];
        app.vault.rebuild_caches();
        app.auto_lock.enabled = true;
        app.auto_lock.after_secs = 60;
        app.auto_lock.last_activity = Instant::now()
            .checked_sub(Duration::from_secs(120))
            .expect("the monotonic clock is past two minutes");
        (app, req_rx, resp_tx)
    }

    fn sent_lock(req_rx: &Receiver<WorkerRequest>) -> bool {
        req_rx.try_iter().any(|r| matches!(r, WorkerRequest::Lock))
    }

    #[test]
    fn stale_auto_lock_on_the_create_form_locks_and_wipes_it() {
        let _env = session_env_lock();
        use crate::tui::edit_field::EditField;
        let (mut app, req_rx, _resp_tx) = unlocked_app_gone_idle();
        app.screen = Screen::Create;
        app.create.choosing_type = false;
        app.create.fields = vec![EditField::new("Password", "changeme", true)];
        app.edit.fields = vec![EditField::new("Password", "changeme", true)];
        app.edit.item_id = "a".into();
        app.edit.active = true;

        crate::tui::flows::auth::check_auto_lock(&mut app);

        assert!(sent_lock(&req_rx), "an idle Create form locked the vault");
        assert_eq!(app.screen, Screen::Login);
        assert!(app.create.fields.is_empty());
        assert!(app.create.choosing_type);
        assert!(app.edit.fields.is_empty());
        assert!(app.edit.item_id.is_empty());
        assert!(!app.edit.active);
        assert!(app.vault.items.is_empty());
    }

    #[test]
    fn stale_auto_lock_on_the_generator_overlay_locks_and_wipes_it() {
        let _env = session_env_lock();
        let (mut app, req_rx, _resp_tx) = unlocked_app_gone_idle();
        app.screen = Screen::Generator;
        app.generator.result = Zeroizing::new("changeme".into());
        app.generator.return_target = Some(crate::tui::generator::ReturnTarget::EditField(0));

        crate::tui::flows::auth::check_auto_lock(&mut app);

        assert!(
            sent_lock(&req_rx),
            "an idle Generator overlay locked the vault"
        );
        assert_eq!(app.screen, Screen::Login);
        assert!(app.generator.result.is_empty());
        assert!(app.generator.return_target.is_none());
    }

    #[test]
    fn stale_auto_lock_on_the_command_palette_locks_and_closes_it() {
        let _env = session_env_lock();
        let (mut app, req_rx, _resp_tx) = unlocked_app_gone_idle();
        app.screen = Screen::Create;
        crate::tui::flows::palette::open(&mut app);
        assert_eq!(app.screen, Screen::CommandPalette);

        crate::tui::flows::auth::check_auto_lock(&mut app);

        assert!(sent_lock(&req_rx));
        assert_eq!(app.screen, Screen::Login);
        assert!(app.palette.is_none());
    }

    #[test]
    fn lock_wipes_every_overlay_that_can_hold_decrypted_state() {
        let _env = session_env_lock();
        use crate::tui::flows::items::RenameFieldState;
        use crate::tui::item_actions::ItemActionsState;
        use crate::tui::reprompt::{ProtectedAction, RepromptState};
        let (mut app, _req_rx, _resp_tx) = unlocked_app_gone_idle();
        app.screen = Screen::RepromptUnlock;
        app.folders = vec![Folder {
            id: "f".into(),
            name: "Private".into(),
        }];
        app.show_password = true;
        app.detail_field = 3;
        app.reprompt_verified = true;
        app.help_from = Some(Screen::Detail);
        app.vault.search_query.insert_str("alpha");
        app.rename_field = Some(RenameFieldState {
            input: crate::domain::line_editor::LineEditor::with_text("changeme"),
            target_idx: 0,
        });
        app.send_create = Some(crate::tui::send::SendCreateState::new());
        app.export = Some(crate::tui::export::ExportState::new());
        app.import = Some(crate::tui::import::ImportState::new(&[]));
        app.item_actions = Some(ItemActionsState {
            item_id: "a".into(),
            actions: Vec::new(),
            cursor: 0,
        });
        let mut reprompt = RepromptState::new(ProtectedAction::RevealDetail, Screen::Detail);
        reprompt.input.insert_str("changeme");
        app.reprompt = Some(reprompt);

        crate::tui::flows::auth::lock_vault(&mut app);

        assert_eq!(app.screen, Screen::Login);
        assert!(app.folders.is_empty());
        assert!(!app.show_password);
        assert_eq!(app.detail_field, 0);
        assert!(!app.reprompt_verified);
        assert!(app.help_from.is_none());
        assert!(app.vault.search_query.is_empty());
        assert!(app.rename_field.is_none());
        assert!(app.folder_name.is_none());
        assert!(app.send_create.is_none());
        assert!(app.export.is_none());
        assert!(app.import.is_none());
        assert!(app.attachment_upload.is_none());
        assert!(app.attachment_download.is_none());
        assert!(app.attachment_delete.is_none());
        assert!(app.memberships.is_none());
        assert!(app.assign_collections.is_none());
        assert!(app.item_actions.is_none());
        assert!(app.reprompt.is_none());
        assert!(app.palette.is_none());
    }

    #[test]
    fn a_failing_debug_log_is_reported_once_per_session() {
        let (mut app, _req_rx, _resp_tx) = fresh_app();
        let dir = tempfile::tempdir().unwrap();
        app.debug_log_path = Some(dir.path().to_path_buf());

        for i in 0..3 {
            app.push_cmd(&format!("bw cmd {i}"), true, "ok");
        }

        let notices: Vec<_> = app
            .cmd_log
            .entries
            .iter()
            .filter(|e| e.cmd == "debug log")
            .collect();
        assert_eq!(notices.len(), 1);
        assert!(!notices[0].ok);
        assert!(
            notices[0]
                .detail
                .starts_with(&format!("could not write {}: ", dir.path().display()))
        );
        assert_eq!(app.cmd_log.entries.len(), 4);
    }

    #[test]
    fn lock_clears_the_command_log_down_to_the_lock_entry() {
        let _env = session_env_lock();
        let (mut app, _req_rx, _resp_tx) = unlocked_app_gone_idle();
        app.push_cmd("bw list items", true, "3 items loaded");
        app.push_cmd("bw get item a", true, "fetched");

        crate::tui::flows::auth::lock_vault(&mut app);

        assert_eq!(app.cmd_log.entries.len(), 1);
        assert_eq!(app.cmd_log.entries[0].cmd, "bw lock");
    }

    #[test]
    fn auto_lock_leaves_screens_with_nothing_to_lock_alone() {
        let _env = session_env_lock();
        for screen in [Screen::Splash, Screen::Login] {
            let (mut app, req_rx, _resp_tx) = unlocked_app_gone_idle();
            app.screen = screen.clone();
            crate::tui::flows::auth::check_auto_lock(&mut app);
            assert!(!sent_lock(&req_rx), "{screen:?} must not trigger a lock");
            assert_eq!(app.screen, screen);
        }

        let (mut app, req_rx, _resp_tx) = unlocked_app_gone_idle();
        app.screen = Screen::Login;
        app.help_from = Some(Screen::Login);
        app.screen = Screen::Help;
        crate::tui::flows::auth::check_auto_lock(&mut app);
        assert!(
            !sent_lock(&req_rx),
            "help opened from Login has nothing to lock"
        );
        assert_eq!(app.screen, Screen::Help);

        let (mut app, req_rx, _resp_tx) = unlocked_app_gone_idle();
        app.screen = Screen::Login;
        app.open_settings();
        crate::tui::flows::auth::check_auto_lock(&mut app);
        assert!(
            !sent_lock(&req_rx),
            "settings opened from Login has nothing to lock"
        );
        assert_eq!(app.screen, Screen::Settings);

        let (mut app, req_rx, _resp_tx) = unlocked_app_gone_idle();
        app.authenticated = false;
        app.screen = Screen::Create;
        crate::tui::flows::auth::check_auto_lock(&mut app);
        assert!(
            !sent_lock(&req_rx),
            "an unauthenticated app has nothing to lock"
        );
    }

    #[test]
    fn auto_lock_waits_while_a_request_is_in_flight() {
        let _env = session_env_lock();
        let (mut app, req_rx, _resp_tx) = unlocked_app_gone_idle();
        app.screen = Screen::Create;
        assert!(app.begin(InFlight::Sync));
        crate::tui::flows::auth::check_auto_lock(&mut app);
        assert!(!sent_lock(&req_rx));
        assert_eq!(app.screen, Screen::Create);
    }

    #[test]
    fn the_detail_header_right_anchors_the_status_whatever_the_name_width() {
        use crate::tui::action::ActionState;
        use crate::tui::screens::Screen;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let status_end_in = |name: &str, editing: bool| -> usize {
            let (mut app, _req_rx, _resp_tx) = fresh_app();
            app.vault.items = vec![login_item("a", name, "user@example.com")];
            app.vault.rebuild_caches();
            app.vault.selected_index = 0;
            app.screen = Screen::Detail;
            if editing {
                crate::tui::flows::items::enter_edit_mode(&mut app);
                assert!(app.edit.active);
            }
            app.set_action(ActionState::Done("Copied".into()));
            let mut term = Terminal::new(TestBackend::new(90, 24)).unwrap();
            term.draw(|f| crate::tui::view::draw(f, &mut app)).unwrap();
            let buf = term.backend().buffer();
            let row: Vec<&str> = (0..buf.area().width)
                .map(|x| buf.cell((x, 0)).map_or("", |c| c.symbol()))
                .collect();
            row.windows(6)
                .position(|w| w == ["C", "o", "p", "i", "e", "d"])
                .map(|i| i + 6)
                .unwrap_or_else(|| panic!("the status is rendered next to {name}"))
        };
        let status_end = |name: &str| status_end_in(name, false);
        let ascii = status_end("Acct");
        assert_eq!(ascii, 88, "the status ends two cells short of the edge");
        assert_eq!(
            status_end("Café"),
            ascii,
            "an accented name shifts the status"
        );
        assert_eq!(
            status_end("日本語é"),
            ascii,
            "a wide name shifts the status"
        );
        assert_eq!(
            status_end_in("Acct", true),
            ascii,
            "the [EDIT] tag pushes the status past the edge"
        );
        assert_eq!(
            status_end_in("日本語é", true),
            ascii,
            "a wide name in edit mode shifts the status"
        );
    }
}
