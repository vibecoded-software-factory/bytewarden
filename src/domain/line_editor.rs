//! [`LineEditor`] — the one single-line text-input model.
//!
//! Every text input in the app (popup paths/names, the reprompt master
//! password, …) edits a `LineEditor` instead of hand-rolling a
//! `String` + cursor pair with its own `char_indices().nth(…)` dance.
//! Centralising it means the cursor arithmetic and the readline word
//! ops (`Ctrl+W`, `Ctrl+U`, `Ctrl+←/→`, `Ctrl+A`, `Ctrl+E`) are written
//! once and every input inherits them identically — see
//! [`crate::tui::input::common::route_line_editor`].
//!
//! The cursor is a **char index** (always on a char boundary, so
//! multi-byte input is safe) into [`Self::text`]; all byte offsets are
//! derived on demand via `char_indices`. Single-line only: newlines are
//! never inserted.
//!
//! It derives [`Zeroize`] / [`ZeroizeOnDrop`] because any input can hold
//! sensitive content (a master password, a hidden custom field), and
//! [`Self::clear`] / [`Self::set`] scrub the previous bytes explicitly —
//! `String::clear` / re-assignment would otherwise leave the old
//! characters in the backing capacity until the whole struct drops.

use zeroize::{Zeroize, ZeroizeOnDrop};

/// A single-line text buffer with a char-index cursor.
#[derive(Debug, Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct LineEditor {
    text: String,
    cursor: usize,
}

impl LineEditor {
    /// An empty editor with the cursor at the start.
    pub fn new() -> Self {
        Self::default()
    }

    /// An editor pre-filled with `text`, cursor at the end.
    pub fn with_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.chars().count();
        Self { text, cursor }
    }

    /// The current text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The current text (alias for `text`, reads naturally at call sites
    /// that treat it as a value).
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The cursor position, as a char index in `[0, len_chars]`.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Number of characters (not bytes).
    pub fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Byte offset of char index `i` (or `text.len()` at/after the end).
    fn byte_at(&self, i: usize) -> usize {
        self.text
            .char_indices()
            .nth(i)
            .map(|(b, _)| b)
            .unwrap_or(self.text.len())
    }

    // ── Editing ───────────────────────────────────────────────────────────

    /// Inserts `c` at the cursor and advances past it.
    pub fn insert(&mut self, c: char) {
        let byte = self.byte_at(self.cursor);
        self.text.insert(byte, c);
        self.cursor += 1;
    }

    /// Inserts a whole string at the cursor (single-line: newlines are
    /// dropped so a paste can't smuggle a line break in).
    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars().filter(|c| *c != '\n' && *c != '\r') {
            self.insert(c);
        }
    }

    /// Deletes the char before the cursor (Backspace). No-op at the start.
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let byte = self.byte_at(self.cursor - 1);
        self.text.remove(byte);
        self.cursor -= 1;
    }

    /// Deletes the char at the cursor (Delete). No-op at the end.
    pub fn delete(&mut self) {
        if self.cursor >= self.len_chars() {
            return;
        }
        let byte = self.byte_at(self.cursor);
        self.text.remove(byte);
    }

    // ── Cursor moves ──────────────────────────────────────────────────────

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }
    pub fn right(&mut self) {
        if self.cursor < self.len_chars() {
            self.cursor += 1;
        }
    }
    pub fn home(&mut self) {
        self.cursor = 0;
    }
    pub fn end(&mut self) {
        self.cursor = self.len_chars();
    }

    // ── Word ops (readline / vim-insert) ──────────────────────────────────

    /// Char index of the previous word boundary from the cursor: skips
    /// any run of whitespace, then the run of non-whitespace before it.
    fn prev_word(&self) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let mut i = self.cursor;
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        i
    }

    /// Char index of the next word boundary from the cursor.
    fn next_word(&self) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let n = chars.len();
        let mut i = self.cursor;
        while i < n && chars[i].is_whitespace() {
            i += 1;
        }
        while i < n && !chars[i].is_whitespace() {
            i += 1;
        }
        i
    }

    /// Moves the cursor left one word (`Ctrl+←`).
    pub fn word_left(&mut self) {
        self.cursor = self.prev_word();
    }

    /// Moves the cursor right one word (`Ctrl+→`).
    pub fn word_right(&mut self) {
        self.cursor = self.next_word();
    }

    /// Deletes the word before the cursor (`Ctrl+W`).
    pub fn delete_word_back(&mut self) {
        let start = self.prev_word();
        if start == self.cursor {
            return;
        }
        let from = self.byte_at(start);
        let to = self.byte_at(self.cursor);
        self.text.replace_range(from..to, "");
        self.cursor = start;
    }

    /// Deletes from the line start to the cursor (`Ctrl+U`).
    pub fn kill_to_start(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let to = self.byte_at(self.cursor);
        self.text.replace_range(0..to, "");
        self.cursor = 0;
    }

    // ── Bulk set / clear (scrubbing) ──────────────────────────────────────

    /// Replaces the whole buffer, scrubbing the previous contents first
    /// so a secret can't linger in the freed capacity.
    pub fn set(&mut self, text: impl Into<String>) {
        self.text.zeroize();
        self.text = text.into();
        self.cursor = self.len_chars();
    }

    /// Empties the buffer, scrubbing the previous contents first.
    pub fn clear(&mut self) {
        self.text.zeroize();
        self.text.clear();
        self.cursor = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_advances_cursor() {
        let mut e = LineEditor::new();
        e.insert('a');
        e.insert('b');
        assert_eq!(e.text(), "ab");
        assert_eq!(e.cursor(), 2);
    }

    #[test]
    fn insert_in_the_middle() {
        let mut e = LineEditor::with_text("ac");
        e.home();
        e.right(); // between a and c
        e.insert('b');
        assert_eq!(e.text(), "abc");
        assert_eq!(e.cursor(), 2);
    }

    #[test]
    fn backspace_and_delete() {
        let mut e = LineEditor::with_text("abc");
        e.backspace(); // "ab"
        assert_eq!(e.text(), "ab");
        e.home();
        e.delete(); // "b"
        assert_eq!(e.text(), "b");
        assert_eq!(e.cursor(), 0);
    }

    #[test]
    fn backspace_at_start_is_noop() {
        let mut e = LineEditor::with_text("x");
        e.home();
        e.backspace();
        assert_eq!(e.text(), "x");
        assert_eq!(e.cursor(), 0);
    }

    #[test]
    fn multibyte_is_char_safe() {
        let mut e = LineEditor::with_text("áé");
        assert_eq!(e.cursor(), 2);
        e.backspace();
        assert_eq!(e.text(), "á");
        e.insert('ñ');
        assert_eq!(e.text(), "áñ");
    }

    #[test]
    fn insert_str_flattens_newlines() {
        let mut e = LineEditor::new();
        e.insert_str("a\nb\rc");
        assert_eq!(e.text(), "abc");
    }

    #[test]
    fn word_ops() {
        let mut e = LineEditor::with_text("foo bar baz");
        e.delete_word_back(); // removes "baz"
        assert_eq!(e.text(), "foo bar ");
        e.word_left(); // to start of "bar"
        assert_eq!(e.cursor(), 4);
        e.kill_to_start(); // removes "foo "
        assert_eq!(e.text(), "bar ");
        assert_eq!(e.cursor(), 0);
    }

    #[test]
    fn clear_scrubs_and_resets() {
        let mut e = LineEditor::with_text("secret");
        e.clear();
        assert!(e.is_empty());
        assert_eq!(e.cursor(), 0);
    }
}
