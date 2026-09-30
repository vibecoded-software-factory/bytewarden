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
pub mod glyph;
pub mod import;
pub mod input;
pub mod item_actions;
pub mod item_forms;
pub mod keyboard;
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

const FEEDBACK_TICKS: u8 = 19;

const POLL_IDLE_MS: u64 = 500;

const POLL_BUSY_MS: u64 = 80;

pub fn run(
    vault: Box<dyn VaultPort + Send>,
    clipboard: Box<dyn ClipboardPort>,
    settings: Box<dyn SettingsPort>,
    generator: Box<dyn PasswordGeneratorPort + Send>,
    list_items_timeout: std::sync::Arc<std::sync::atomic::AtomicU64>,
) -> Result<()> {
    ratatui::run(|terminal| {
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
        install_mouse_teardown_hook();

        app.set_action(ActionState::Running("Checking session…".into()));
        draw_frame(terminal, &mut app)?;
        flows::auth::request_resume(&mut app);

        let result = run_loop(terminal, &mut app);
        let _ = write_mouse_teardown(&mut std::io::stdout());

        drop(app);
        drop(worker);
        result
    })
}

fn write_mouse_teardown(out: &mut impl std::io::Write) -> std::io::Result<()> {
    execute!(out, DisableMouseCapture)
}

fn install_mouse_teardown_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = write_mouse_teardown(&mut std::io::stdout());
        previous(info);
    }));
}

fn draw_frame(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    terminal.draw(|frame| {
        view::draw(frame, app);
        if app.glyphs == crate::tui::glyph::GlyphCaps::Console {
            crate::tui::glyph::sanitize_buffer(frame.buffer_mut());
        }
    })?;
    Ok(())
}

fn run_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    let mut done_ticks: u8 = 0;
    let mut last_size = terminal.size()?;

    loop {
        let size = terminal.size()?;
        if size != last_size {
            last_size = size;
            terminal.clear()?;
        }

        draw_frame(terminal, app)?;

        loop {
            match app.worker_rx.try_recv() {
                Ok(resp) => {
                    flows::apply_response(app, resp);
                    done_ticks = 0;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,

                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    app.on_worker_dead();
                    break;
                }
            }
        }

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

fn poll_timeout(state: &ActionState, busy: bool) -> Duration {
    if busy {
        return Duration::from_millis(POLL_BUSY_MS);
    }
    match state {
        ActionState::Idle => Duration::from_millis(POLL_IDLE_MS),
        _ => Duration::from_millis(POLL_BUSY_MS),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_teardown_disables_every_mode_mouse_capture_enables() {
        let mut out = Vec::new();
        write_mouse_teardown(&mut out).unwrap();
        let written = String::from_utf8(out).unwrap();
        for mode in ["?1000l", "?1002l", "?1003l", "?1015l", "?1006l"] {
            assert!(
                written.contains(&format!("\x1b[{mode}")),
                "missing {mode} in {written:?}"
            );
        }
    }
}
