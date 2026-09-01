use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::domain::LineEditor;

#[inline]
pub fn types_a_char(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char(_))
        && !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

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
        KeyCode::Char(c) if types_a_char(&key) => {
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

pub fn search_key(editor: &mut LineEditor, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Up
        | KeyCode::Down
        | KeyCode::PageUp
        | KeyCode::PageDown
        | KeyCode::Enter
        | KeyCode::Tab
        | KeyCode::BackTab
        | KeyCode::Esc => false,
        _ => route_line_editor(editor, key),
    }
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
    fn types_a_char_rejects_ctrl_and_alt_chords() {
        assert!(!types_a_char(&ctrl(KeyCode::Char('w'))));
        assert!(!types_a_char(&ctrl(KeyCode::Char('u'))));
        assert!(!types_a_char(&KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::ALT
        )));
    }

    #[test]
    fn types_a_char_accepts_plain_and_shifted_characters() {
        assert!(types_a_char(&key(KeyCode::Char('-'))));
        assert!(types_a_char(&key(KeyCode::Char('w'))));

        assert!(types_a_char(&KeyEvent::new(
            KeyCode::Char('_'),
            KeyModifiers::SHIFT
        )));
    }

    #[test]
    fn types_a_char_is_false_for_non_character_keys() {
        for code in [
            KeyCode::Backspace,
            KeyCode::Enter,
            KeyCode::Left,
            KeyCode::Esc,
        ] {
            assert!(!types_a_char(&key(code)), "{code:?} is not typed text");
        }
    }

    #[test]
    fn ctrl_chars_do_not_insert_letters() {
        let mut e = LineEditor::new();

        assert!(!route_line_editor(&mut e, ctrl(KeyCode::Char('a'))));
        assert_eq!(e.text(), "");
    }

    #[test]
    fn search_key_types_bare_letters_including_j_and_k() {
        let mut e = LineEditor::new();
        for c in ['j', 'k', 'x'] {
            assert!(search_key(&mut e, key(KeyCode::Char(c))));
        }
        assert_eq!(e.text(), "jkx");
    }

    #[test]
    fn search_key_declines_the_keys_the_list_owns() {
        let mut e = LineEditor::with_text("q");
        for code in [
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::PageUp,
            KeyCode::PageDown,
            KeyCode::Enter,
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Esc,
        ] {
            assert!(
                !search_key(&mut e, key(code)),
                "{code:?} must reach the list"
            );
        }
        assert_eq!(e.text(), "q", "navigation keys never touch the buffer");
    }

    #[test]
    fn search_key_inherits_the_word_ops() {
        let mut e = LineEditor::with_text("foo bar");
        assert!(search_key(&mut e, ctrl(KeyCode::Char('w'))));
        assert_eq!(e.text(), "foo ");
    }

    #[test]
    fn search_key_moves_the_cursor_without_reporting_change() {
        let mut e = LineEditor::with_text("hi");
        assert!(!search_key(&mut e, key(KeyCode::Left)));
        assert_eq!(e.cursor(), 1);
    }
}
