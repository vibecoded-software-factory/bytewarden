//! Shared input mechanics — the routines every per-screen handler
//! delegates to instead of re-implementing.
//!
//! Living here is what keeps behaviour identical across surfaces: the
//! cursor arithmetic and readline word ops are written once in
//! [`route_line_editor`], so a `Ctrl+W` deletes a word the same way in
//! every popup input.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::domain::LineEditor;

/// Routes one key event into a [`LineEditor`], returning `true` when the
/// text changed (so a caller behind a filter can rebuild it). Modifier
/// combinations are matched first so they never fall through to plain
/// insertion:
///
/// * `Ctrl+W` delete word · `Ctrl+U` kill to start
/// * `Ctrl+A` / `Ctrl+E` line start / end
/// * `Ctrl+←` / `Ctrl+→` word left / right
/// * Backspace / Delete, printable insert, plain arrows, Home / End
///
/// A printable char is inserted only when neither `Ctrl` nor `Alt` is
/// held (those tiers are reserved for actions), so this is safe to call
/// as the fallback arm of any handler.
pub fn route_line_editor(editor: &mut LineEditor, key: KeyEvent) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('w') if ctrl => {
            editor.delete_word_back();
            return true;
        }
        KeyCode::Char('u') if ctrl => {
            editor.kill_to_start();
            return true;
        }
        KeyCode::Char('a') if ctrl => editor.home(),
        KeyCode::Char('e') if ctrl => editor.end(),
        KeyCode::Left if ctrl => editor.word_left(),
        KeyCode::Right if ctrl => editor.word_right(),
        KeyCode::Backspace => {
            editor.backspace();
            return true;
        }
        KeyCode::Delete => {
            editor.delete();
            return true;
        }
        KeyCode::Char(c)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            editor.insert(c);
            return true;
        }
        KeyCode::Left => editor.left(),
        KeyCode::Right => editor.right(),
        KeyCode::Home => editor.home(),
        KeyCode::End => editor.end(),
        _ => {}
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn typing_inserts_and_reports_change() {
        let mut e = LineEditor::new();
        assert!(route_line_editor(&mut e, key(KeyCode::Char('h'))));
        assert!(route_line_editor(&mut e, key(KeyCode::Char('i'))));
        assert_eq!(e.text(), "hi");
    }

    #[test]
    fn arrows_move_without_reporting_change() {
        let mut e = LineEditor::with_text("hi");
        assert!(!route_line_editor(&mut e, key(KeyCode::Left)));
        assert_eq!(e.cursor(), 1);
        assert!(!route_line_editor(&mut e, key(KeyCode::Home)));
        assert_eq!(e.cursor(), 0);
    }

    #[test]
    fn ctrl_w_deletes_a_word() {
        let mut e = LineEditor::with_text("foo bar");
        assert!(route_line_editor(&mut e, ctrl(KeyCode::Char('w'))));
        assert_eq!(e.text(), "foo ");
    }

    #[test]
    fn ctrl_u_kills_to_start() {
        let mut e = LineEditor::with_text("foo bar");
        assert!(route_line_editor(&mut e, ctrl(KeyCode::Char('u'))));
        assert_eq!(e.text(), "");
    }

    #[test]
    fn ctrl_chars_do_not_insert_letters() {
        let mut e = LineEditor::new();
        // Ctrl+A is a cursor move, not an 'a' insertion.
        assert!(!route_line_editor(&mut e, ctrl(KeyCode::Char('a'))));
        assert_eq!(e.text(), "");
    }
}
