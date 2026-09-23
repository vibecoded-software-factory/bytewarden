use crate::domain::LineEditor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFocus {
    Format,
    Path,
}

#[derive(Debug, Clone)]
pub struct ImportState {
    pub formats: Vec<String>,

    pub format_idx: usize,

    pub path: LineEditor,
    pub focus: ImportFocus,
}

impl ImportState {
    pub fn new(available: &[String]) -> Self {
        let formats: Vec<String> = if available.is_empty() {
            vec!["bitwardenjson".to_string()]
        } else {
            available.to_vec()
        };

        let format_idx = formats
            .iter()
            .position(|f| f == "bitwardenjson")
            .unwrap_or(0);
        Self {
            formats,
            format_idx,
            path: LineEditor::new(),
            focus: ImportFocus::Path,
        }
    }

    pub fn current_format(&self) -> &str {
        self.formats
            .get(self.format_idx)
            .map(String::as_str)
            .unwrap_or("bitwardenjson")
    }

    pub fn cycle_format(&mut self, dir: i32) {
        let n = self.formats.len();
        if n == 0 {
            return;
        }
        let cur = self.format_idx as i32;
        let next = ((cur + dir) % n as i32 + n as i32) % n as i32;
        self.format_idx = next as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_falls_back_to_bitwardenjson_when_list_empty() {
        let s = ImportState::new(&[]);
        assert_eq!(s.formats, vec!["bitwardenjson".to_string()]);
        assert_eq!(s.format_idx, 0);
        assert_eq!(s.current_format(), "bitwardenjson");
    }

    #[test]
    fn new_defaults_to_bitwardenjson_when_present() {
        let avail = vec![
            "1password1pif".into(),
            "bitwardenjson".into(),
            "lastpasscsv".into(),
        ];
        let s = ImportState::new(&avail);
        assert_eq!(s.current_format(), "bitwardenjson");
    }

    #[test]
    fn new_picks_first_when_bitwardenjson_absent() {
        let avail = vec!["1password1pif".into(), "lastpasscsv".into()];
        let s = ImportState::new(&avail);
        assert_eq!(s.current_format(), "1password1pif");
    }

    #[test]
    fn cycle_format_wraps_around() {
        let avail = vec!["a".into(), "b".into(), "c".into()];
        let mut s = ImportState::new(&avail);
        s.format_idx = 0;
        s.cycle_format(-1);
        assert_eq!(s.format_idx, 2);
        s.cycle_format(1);
        assert_eq!(s.format_idx, 0);
    }
}
