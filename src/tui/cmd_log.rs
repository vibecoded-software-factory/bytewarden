//! Command-log panel state.
//!
//! The bounded backlog of redacted `bw` invocations shown in the vault's
//! `─[4]-Command log` panel, plus its scroll position — split out of the
//! [`crate::tui::app::App`] god-struct into its own small state+behaviour
//! unit. Entries arrive **already redacted**: [`crate::tui::app::App::push_cmd`]
//! owns the redaction (it needs the cached session marker) and hands a
//! finished [`CmdEntry`] here.

use crate::tui::action::CmdEntry;

/// Maximum number of entries kept in the panel.
const LIMIT: usize = 50;

/// The command-log backlog + its scroll offset.
#[derive(Default)]
pub struct CmdLog {
    /// Newest-last list of redacted command entries.
    pub entries: Vec<CmdEntry>,
    /// Scroll offset from the bottom (0 = pinned to the latest entry).
    pub scroll: usize,
}

impl CmdLog {
    /// Appends an already-redacted entry, capping the backlog at
    /// [`LIMIT`] and re-pinning the view to the latest line.
    pub fn push(&mut self, entry: CmdEntry) {
        self.entries.push(entry);
        if self.entries.len() > LIMIT {
            self.entries.remove(0);
        }
        self.scroll = 0;
    }

    /// Empties the log (on logout).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.scroll = 0;
    }

    /// Scrolls toward older entries, clamped to the backlog length.
    pub fn scroll_up(&mut self, n: usize) {
        let max = self.entries.len().saturating_sub(1);
        self.scroll = (self.scroll + n).min(max);
    }

    /// Scrolls back toward the latest entry.
    pub fn scroll_down(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
    }
}
