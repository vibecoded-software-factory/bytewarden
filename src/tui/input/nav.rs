use crossterm::event::KeyEvent;

use crate::tui::edit_field::EditField;

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

pub fn text_input(field: Option<&mut EditField>, key: KeyEvent) {
    let Some(f) = field else {
        return;
    };
    if f.read_only {
        return;
    }
    let _ = crate::tui::input::common::route_line_editor(&mut f.editor, key);
}
