//! Import-popup state.

use crate::domain::LineEditor;

/// Which control of the import popup currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFocus {
    Format,
    Path,
}

/// Buffer for the in-flight import popup. `None` outside the popup.
///
/// The format is no longer a free-form text input — the user picks
/// from the list `bw import --formats` advertised at login (cached
/// in [`crate::tui::App::import_formats`]). When the cached list
/// is empty (bw failed to enumerate, or the user is offline before
/// the first sync) we fall back to a single hard-coded entry
/// (`"bitwardenjson"`) so the popup still works for the common case.
#[derive(Debug, Clone)]
pub struct ImportState {
    /// Available format identifiers, copied from
    /// [`crate::tui::App::import_formats`] at popup-open time.
    pub formats: Vec<String>,
    /// Index of the currently-selected format inside `formats`.
    pub format_idx: usize,
    /// Filesystem path to the file `bw` will import.
    pub path: LineEditor,
    pub focus: ImportFocus,
}

impl ImportState {
    /// Builds a fresh popup. `available` is the cached format list
    /// from the App; if empty we fall back to `bitwardenjson` so the
    /// popup is never useless.
    pub fn new(available: &[String]) -> Self {
        let formats: Vec<String> = if available.is_empty() {
            vec!["bitwardenjson".to_string()]
        } else {
            available.to_vec()
        };
        // Default to bitwardenjson when present (the most common
        // case — re-importing a bytewarden export). Otherwise pick
        // whatever's first in the list.
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

    /// Returns the currently-selected format identifier, used by
    /// the flow's commit path to call `bw import <format> <path>`.
    pub fn current_format(&self) -> &str {
        self.formats
            .get(self.format_idx)
            .map(String::as_str)
            .unwrap_or("bitwardenjson")
    }

    /// Cycles the selection by `dir` (+1 down, -1 up), clamped at
    /// list bounds.
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
        assert_eq!(s.format_idx, 2); // wrap to end
        s.cycle_format(1);
        assert_eq!(s.format_idx, 0); // wrap to start
    }
}
