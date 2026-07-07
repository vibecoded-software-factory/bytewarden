//! Shared navigation primitives used across screens.

use crossterm::event::KeyEvent;

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

/// Cursor + typing keys forwarded to a single [`EditField`], through
/// the one text-input router (`route_line_editor`) — so form fields
/// inherit the readline word ops like every other input. Read-only
/// rows ignore keys entirely. Used by both the create and edit forms.
pub fn text_input(field: Option<&mut EditField>, key: KeyEvent) {
    let Some(f) = field else {
        return;
    };
    if f.read_only {
        return;
    }
    let _ = crate::tui::input::common::route_line_editor(&mut f.editor, key);
}
