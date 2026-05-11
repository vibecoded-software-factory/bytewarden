//! Key handler for the master-password reverify popup.
//!
//! Esc cancels and pops back to the originating screen. Enter
//! triggers verification — the heavy lifting lives in
//! [`crate::tui::flows::reprompt::verify_and_run`].

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::reprompt::{cancel as cancel_popup, verify_and_run};

/// Dispatches a single key event on the reprompt popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel_popup(app),
        KeyCode::Enter => return verify_and_run(app),
        _ => {}
    }

    let Some(state) = app.reprompt.as_mut() else {
        return;
    };
    // Any keystroke after a failed verify clears the error strip so
    // the user isn't told twice — the "wrong password" state is
    // useful as immediate feedback, not as persistent decoration.
    state.error = false;

    match key.code {
        KeyCode::Left if state.cursor > 0 => {
            state.cursor -= 1;
        }
        KeyCode::Right if state.cursor < state.input.chars().count() => {
            state.cursor += 1;
        }
        KeyCode::Home => state.cursor = 0,
        KeyCode::End => state.cursor = state.input.chars().count(),
        KeyCode::Backspace if state.cursor > 0 => {
            let byte = state
                .input
                .char_indices()
                .nth(state.cursor - 1)
                .map(|(b, _)| b)
                .unwrap_or(0);
            state.input.remove(byte);
            state.cursor -= 1;
        }
        KeyCode::Delete if state.cursor < state.input.chars().count() => {
            let byte = state
                .input
                .char_indices()
                .nth(state.cursor)
                .map(|(b, _)| b)
                .unwrap_or(0);
            state.input.remove(byte);
        }
        KeyCode::Char(c) => {
            let byte = state
                .input
                .char_indices()
                .nth(state.cursor)
                .map(|(b, _)| b)
                .unwrap_or(state.input.len());
            state.input.insert(byte, c);
            state.cursor += 1;
        }
        _ => {}
    }
}
