use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct LineEditor {
    text: String,
    cursor: usize,
}

impl LineEditor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.chars().count();
        Self { text, cursor }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn byte_at(&self, i: usize) -> usize {
        self.text
            .char_indices()
            .nth(i)
            .map(|(b, _)| b)
            .unwrap_or(self.text.len())
    }

    pub fn insert(&mut self, c: char) {
        let byte = self.byte_at(self.cursor);
        self.text.insert(byte, c);
        self.cursor += 1;
    }

    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars().filter(|c| *c != '\n' && *c != '\r') {
            self.insert(c);
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let byte = self.byte_at(self.cursor - 1);
        self.text.remove(byte);
        self.cursor -= 1;
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.len_chars() {
            return;
        }
        let byte = self.byte_at(self.cursor);
        self.text.remove(byte);
    }

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

    pub fn word_left(&mut self) {
        self.cursor = self.prev_word();
    }

    pub fn word_right(&mut self) {
        self.cursor = self.next_word();
    }

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

    pub fn kill_to_start(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let to = self.byte_at(self.cursor);
        self.text.replace_range(0..to, "");
        self.cursor = 0;
    }

    pub fn set(&mut self, text: impl Into<String>) {
        self.text.zeroize();
        self.text = text.into();
        self.cursor = self.len_chars();
    }

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
        e.right();
        e.insert('b');
        assert_eq!(e.text(), "abc");
        assert_eq!(e.cursor(), 2);
    }

    #[test]
    fn backspace_and_delete() {
        let mut e = LineEditor::with_text("abc");
        e.backspace();
        assert_eq!(e.text(), "ab");
        e.home();
        e.delete();
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
        e.delete_word_back();
        assert_eq!(e.text(), "foo bar ");
        e.word_left();
        assert_eq!(e.cursor(), 4);
        e.kill_to_start();
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
