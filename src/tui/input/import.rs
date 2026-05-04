//! Key handler for the vault-import popup.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::import::{cancel, commit, focus_step};
use crate::tui::import::ImportFocus;

/// Dispatches a single key event on the import popup.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel(app),
        KeyCode::Enter => return commit(app),
        KeyCode::Tab | KeyCode::Down => return focus_step(app, 1),
        KeyCode::BackTab | KeyCode::Up => return focus_step(app, -1),
        _ => {}
    }

    let Some(state) = app.import.as_mut() else {
        return;
    };
    // Pick the (input, cursor) pair for the focused field; share the
    // text-edit code below.
    let (input, cursor): (&mut String, &mut usize) = match state.focus {
        ImportFocus::Format => (&mut state.format, &mut state.format_cursor),
        ImportFocus::Path => (&mut state.path, &mut state.path_cursor),
    };

    match key.code {
        KeyCode::Left if *cursor > 0 => {
            *cursor -= 1;
        }
        KeyCode::Right if *cursor < input.chars().count() => {
            *cursor += 1;
        }
        KeyCode::Home => *cursor = 0,
        KeyCode::End => *cursor = input.chars().count(),
        KeyCode::Backspace if *cursor > 0 => {
            let byte = input
                .char_indices()
                .nth(*cursor - 1)
                .map(|(b, _)| b)
                .unwrap_or(0);
            input.remove(byte);
            *cursor -= 1;
        }
        KeyCode::Delete if *cursor < input.chars().count() => {
            let byte = input
                .char_indices()
                .nth(*cursor)
                .map(|(b, _)| b)
                .unwrap_or(0);
            input.remove(byte);
        }
        KeyCode::Char(c) => {
            let byte = input
                .char_indices()
                .nth(*cursor)
                .map(|(b, _)| b)
                .unwrap_or(input.len());
            input.insert(byte, c);
            *cursor += 1;
        }
        _ => {}
    }
}
