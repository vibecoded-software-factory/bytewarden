//! Terminal User Interface — the *driving* adapter of the application.
//!
//! The TUI is responsible for:
//!
//! 1. Holding the [`App`] state container,
//! 2. Reading terminal events and dispatching them to per-screen handlers,
//! 3. Rendering the current state to the screen via Ratatui,
//! 4. Draining [`worker::WorkerResponse`]s from the worker thread that
//!    owns the vault + generator ports, so blocking `bw` calls never
//!    freeze the render loop.
//!
//! The bottom-level domain ports ([`crate::ports`]) are injected at
//! construction time, so the whole layer is testable against fakes.

pub mod action;
pub mod app;
pub mod assign_collections;
pub mod auto_lock;
pub mod cmd_log;
pub mod debug_log;
pub mod detail_fields;
pub mod edit_field;
pub mod export;
pub mod flows;
pub mod folders;
pub mod generator;
pub mod import;
pub mod input;
pub mod item_forms;
pub mod login_form;
pub mod mouse_areas;
pub mod reprompt;
pub mod screens;
pub mod send;
pub mod session_file;
pub mod settings_overlay;
pub mod theme;
pub mod vault;
pub mod view;
pub mod worker;

pub use app::App;

use color_eyre::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use std::time::Duration;

use crate::ports::{ClipboardPort, PasswordGeneratorPort, SettingsPort, VaultPort};
use action::ActionState;

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
    vault: Box<dyn VaultPort + Send>,
    clipboard: Box<dyn ClipboardPort>,
    settings: Box<dyn SettingsPort>,
    generator: Box<dyn PasswordGeneratorPort + Send>,
    list_items_timeout: std::sync::Arc<std::sync::atomic::AtomicU64>,
) -> Result<()> {
    ratatui::run(|terminal| {
        // Move the vault + generator ports onto the worker thread so the
        // render loop never blocks on a `bw` call.
        let mut worker = worker::WorkerHandle::spawn(vault, generator);
        let worker_tx = worker.tx();
        let worker_rx = worker.take_rx();
        let mut app = App::new(
            worker_tx,
            worker_rx,
            clipboard,
            settings,
            list_items_timeout.clone(),
        );

        execute!(std::io::stdout(), EnableMouseCapture)?;

        // Show the splash + spinner while the boot `bw status` runs on
        // the worker; the response handler routes to login / vault.
        app.set_action(ActionState::Running("Checking session…".into()));
        terminal.draw(|frame| view::draw(frame, &mut app))?;
        flows::auth::request_resume(&mut app);

        let result = run_loop(terminal, &mut app);
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
        // Drop the app (releasing its request sender + response receiver)
        // before the worker handle so `Shutdown` + join run cleanly.
        drop(app);
        drop(worker);
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

        // Drain every worker response that has arrived, applying each to
        // `App`. Non-blocking — the worker runs the `bw` call off-thread,
        // so the loop keeps animating the spinner while it's in flight.
        loop {
            match app.worker_rx.try_recv() {
                Ok(resp) => {
                    flows::apply_response(app, resp);
                    done_ticks = 0;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                // The worker thread is gone — no response will ever come.
                // Unwedge the UI instead of spinning "busy" forever.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    app.on_worker_dead();
                    break;
                }
            }
        }
        // Belt-and-suspenders for a lost ticket (worker died mid-call, or a
        // response was dropped): release a slot that outlived every per-op
        // timeout so `busy_blocks` can't lock input permanently.
        app.watchdog_release_stuck_request();

        if event::poll(poll_timeout(&app.action_state, app.is_busy()))? {
            let ev = event::read()?;
            input::handle_events(app, ev);
            app.auto_lock.reset();
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

/// Returns the next event-poll timeout — fast while busy / animating,
/// longer when idle to avoid CPU churn.
fn poll_timeout(state: &ActionState, busy: bool) -> Duration {
    if busy {
        return Duration::from_millis(POLL_BUSY_MS);
    }
    match state {
        ActionState::Idle => Duration::from_millis(POLL_IDLE_MS),
        _ => Duration::from_millis(POLL_BUSY_MS),
    }
}

/// Advances the spinner or expires the success toast after
/// [`FEEDBACK_TICKS`] ticks (~1.5 s).
///
/// **Errors are sticky** (mutt/lazygit): a failure is a condition the
/// user must read, not a 1.5 s event, so it persists until the next
/// keypress clears it (`input::handle_events`). Success toasts keep the
/// short fuse.
fn tick_state(app: &mut App, done_ticks: &mut u8) {
    match &app.action_state {
        ActionState::Running(_) => app.tick_action(),
        ActionState::Done(_) => {
            *done_ticks += 1;
            if *done_ticks >= FEEDBACK_TICKS {
                app.set_action(ActionState::Idle);
                *done_ticks = 0;
            }
        }
        ActionState::Error(_) | ActionState::Idle => {}
    }
}
