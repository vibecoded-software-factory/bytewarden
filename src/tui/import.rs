//! Import-popup state.

/// Which control of the import popup currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFocus {
    Format,
    Path,
}

/// Buffer for the in-flight import popup. `None` outside the popup.
#[derive(Debug, Clone)]
pub struct ImportState {
    /// `bw import` format string (e.g. `"bitwardenjson"`,
    /// `"lastpasscsv"`, `"chromecsv"`). Run `bw import --formats` for
    /// the full list.
    pub format: String,
    pub format_cursor: usize,
    /// Filesystem path to the file `bw` will import.
    pub path: String,
    pub path_cursor: usize,
    pub focus: ImportFocus,
}

impl ImportState {
    /// Builds a fresh popup with the most useful defaults: native
    /// Bitwarden JSON format (matches our own export's default) and
    /// an empty path field for the user to fill in.
    pub fn new() -> Self {
        let format = "bitwardenjson".to_string();
        let format_cursor = format.chars().count();
        Self {
            format,
            format_cursor,
            path: String::new(),
            path_cursor: 0,
            focus: ImportFocus::Path,
        }
    }
}

impl Default for ImportState {
    fn default() -> Self {
        Self::new()
    }
}
