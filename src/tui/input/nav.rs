//! Shared navigation primitives used across screens.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::edit_field::EditField;

/// Wrapping navigation — Tab/BackTab. Wraps from last to first and
/// vice versa. No-op if `len == 0`.
pub fn nav_wrap(idx: &mut usize, len: usize, dir: i8) {
    if len == 0 {
        return;
    }
    if dir > 0 {
        *idx = (*idx + 1) % len;
    } else {
        *idx = (*idx + len - 1) % len;
    }
}

/// Clamping navigation — j/k/arrows. Stops at `0` and `len - 1`.
pub fn nav_clamp(idx: &mut usize, len: usize, dir: i8) {
    if len == 0 {
        return;
    }
    if dir > 0 {
        if *idx + 1 < len {
            *idx += 1;
        }
    } else if *idx > 0 {
        *idx -= 1;
    }
}

/// Cursor + typing keys forwarded to a single [`EditField`]. Used by
/// both the create and edit forms.
pub fn text_input(field: Option<&mut EditField>, key: KeyEvent) {
    let Some(f) = field else {
        return;
    };
    match key.code {
        KeyCode::Left => f.cursor_left(),
        KeyCode::Right => f.cursor_right(),
        KeyCode::Home => f.cursor_home(),
        KeyCode::End => f.cursor_end(),
        KeyCode::Backspace => f.delete_before(),
        KeyCode::Delete => f.delete_at(),
        KeyCode::Char(c) => f.insert(c),
        _ => {}
    }
}
