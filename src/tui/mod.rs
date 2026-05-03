//! Terminal User Interface — the *driving* adapter of the application.
//!
//! The TUI is responsible for:
//!
//! 1. Holding the [`App`] state container,
//! 2. Reading terminal events and dispatching them to per-screen handlers,
//! 3. Rendering the current state to the screen via Ratatui,
//! 4. Driving the asynchronous-feeling action queue
//!    ([`action::PendingAction`]) so blocking calls happen between two
//!    spinner frames instead of freezing the UI.
//!
//! The bottom-level domain ports ([`crate::ports`]) are injected at
//! construction time, so the whole layer is testable against fakes.

pub mod action;
pub mod app;
pub mod debug_log;
pub mod detail_fields;
pub mod edit_field;
pub mod export;
pub mod flows;
pub mod folders;
pub mod generator;
pub mod import;
pub mod input;
pub mod mouse_areas;
pub mod screens;
pub mod send;
pub mod session_file;
pub mod theme;
pub mod view;

pub use app::App;

use color_eyre::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use std::time::Duration;

use crate::ports::{ClipboardPort, PasswordGeneratorPort, SettingsPort, VaultPort};
use action::{ActionState, PendingAction};
use screens::Screen;

/// Number of poll ticks (~80 ms each) a Done/Error message stays on screen
/// before reverting to Idle. ~1.5 s total.
const FEEDBACK_TICKS: u8 = 19;

/// Polling interval while idle (controls how quickly resize events are
/// detected when no other event arrives).
const POLL_IDLE_MS: u64 = 500;

/// Polling interval during a Running/Done/Error state (drives spinner
/// animation and feedback expiry).
const POLL_BUSY_MS: u64 = 80;

/// Composition entry point — installs a terminal, runs the event loop,
/// and tears down on exit.
///
/// This function blocks until the user quits the application
/// (`Ctrl+C` or by closing the terminal).
pub fn run(
    vault: Box<dyn VaultPort>,
    clipboard: Box<dyn ClipboardPort>,
    settings: Box<dyn SettingsPort>,
    generator_port: Box<dyn PasswordGeneratorPort>,
) -> Result<()> {
    ratatui::run(|terminal| {
        let mut app = App::new(vault, clipboard, settings, generator_port);
        execute!(std::io::stdout(), EnableMouseCapture)?;

        // Show the splash + spinner while `bw status` runs.
        app.set_action(ActionState::Running("Checking session…".into()));
        terminal.draw(|frame| view::draw(frame, &mut app))?;

        flows::auth::resume_from_status(&mut app);

        // After the status check, settle into the right starting screen.
        if app.screen != Screen::Vault {
            app.screen = Screen::Login;
        }
        if matches!(app.action_state, ActionState::Running(_)) {
            app.set_action(ActionState::Idle);
        }

        let result = run_loop(terminal, &mut app);
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
        result
    })
}

/// Inner event loop — separated from [`run`] so the terminal restore
/// runs even on early returns.
fn run_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    let mut done_ticks: u8 = 0;
    let mut last_size = terminal.size()?;

    loop {
        // Detect terminal resize regardless of whether `Event::Resize`
        // is delivered. `terminal.size()` is a cheap ioctl, safe to
        // call every iteration.
        let size = terminal.size()?;
        if size != last_size {
            last_size = size;
            terminal.clear()?;
        }

        terminal.draw(|frame| view::draw(frame, app))?;

        // Dispatch any pending action *after* the Running frame is drawn,
        // so the spinner is visible before the blocking call.
        if app.pending_action != PendingAction::None {
            dispatch_pending(app);
            done_ticks = 0;
            terminal.draw(|frame| view::draw(frame, app))?;
        }

        if event::poll(poll_timeout(&app.action_state))? {
            let ev = event::read()?;
            input::handle_events(app, ev);
            app.reset_activity();
        } else {
            flows::auth::check_auto_lock(app);
            tick_state(app, &mut done_ticks);
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

/// Executes the queued [`PendingAction`] and clears the slot.
fn dispatch_pending(app: &mut App) {
    let pending = std::mem::replace(&mut app.pending_action, PendingAction::None);
    match pending {
        PendingAction::None => {}
        PendingAction::Login => flows::auth::do_login(app),
        PendingAction::CopyUsername => flows::copy::do_copy_username(app),
        PendingAction::CopyPassword => flows::copy::do_copy_password(app),
        PendingAction::SyncVault => flows::vault::do_sync_vault(app),
        PendingAction::ToggleFavorite => flows::items::do_toggle_favorite(app),
        PendingAction::CopyRaw(text, msg) => flows::copy::do_copy_raw(app, text, msg),
        PendingAction::CopyTotp(item_id) => flows::copy::do_copy_totp(app, item_id),
        PendingAction::SaveEdit => flows::items::do_save_edit(app),
        PendingAction::CreateItem => flows::items::do_create_item(app),
        PendingAction::DeleteItem { permanent } => flows::items::do_delete_item(app, permanent),
        PendingAction::RestoreItem => flows::items::do_restore_item(app),
        PendingAction::LoadTrash => flows::vault::load_trash(app),
        PendingAction::GeneratePassword => flows::generator::do_generate(app),
        PendingAction::CheckExposed(id) => flows::items::do_check_exposed(app, id),
        PendingAction::DownloadAttachment => flows::items::do_download_attachment(app),
        PendingAction::DeleteAttachment => flows::items::do_delete_attachment(app),
    }
}

/// Returns the next event-poll timeout — fast during animation, longer
/// when idle to avoid CPU churn.
fn poll_timeout(state: &ActionState) -> Duration {
    match state {
        ActionState::Idle => Duration::from_millis(POLL_IDLE_MS),
        _ => Duration::from_millis(POLL_BUSY_MS),
    }
}

/// Advances the spinner or expires Done/Error feedback after
/// [`FEEDBACK_TICKS`] ticks (~1.5 s).
fn tick_state(app: &mut App, done_ticks: &mut u8) {
    match &app.action_state {
        ActionState::Running(_) => app.tick_action(),
        ActionState::Done(_) | ActionState::Error(_) => {
            *done_ticks += 1;
            if *done_ticks >= FEEDBACK_TICKS {
                app.set_action(ActionState::Idle);
                *done_ticks = 0;
            }
        }
        ActionState::Idle => {}
    }
}
