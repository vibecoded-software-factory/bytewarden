//! Mouse hit-test areas — populated every frame by [`crate::tui::view`]
//! and consumed by [`crate::tui::input::mouse`].

use ratatui::layout::Rect;

use crate::tui::screens::Focus;

/// Last frame's bounding rectangles for each mouse-clickable region.
///
/// The view layer overwrites these every render, the input layer reads
/// them when a click arrives. `None` means "this region was not visible
/// in the last frame".
#[derive(Debug, Clone, Default)]
pub struct MouseAreas {
    pub status: Option<Rect>,
    pub search: Option<Rect>,
    pub folders: Option<Rect>,
    pub items: Option<Rect>,
    pub list: Option<Rect>,
    pub cmdlog: Option<Rect>,
    pub detail: Option<Rect>,
}

impl MouseAreas {
    /// Returns the [`Focus`] panel under the given screen coordinates,
    /// or `None` if the click landed outside any tracked region.
    pub fn focus_for(&self, col: u16, row: u16) -> Option<Focus> {
        let hit = |r: Option<Rect>| r.is_some_and(|r| rect_contains(r, col, row));
        if hit(self.status) {
            return Some(Focus::Status);
        }
        if hit(self.search) {
            return Some(Focus::Search);
        }
        if hit(self.folders) {
            return Some(Focus::Folders);
        }
        if hit(self.items) {
            return Some(Focus::Items);
        }
        if hit(self.list) {
            return Some(Focus::List);
        }
        if hit(self.cmdlog) {
            return Some(Focus::CmdLog);
        }
        None
    }

    /// Translates a click row into the *visible* row index inside the
    /// vault list, accounting for the panel's top border.
    pub fn list_row(&self, row: u16) -> Option<usize> {
        let r = self.list?;
        if row < r.y + 1 || row >= r.y + r.height.saturating_sub(1) {
            return None;
        }
        Some((row - r.y - 1) as usize)
    }

    /// Translates a click row into the visible row index inside the
    /// items-filter sidebar.
    pub fn items_row(&self, row: u16) -> Option<usize> {
        let r = self.items?;
        if row < r.y + 1 || row >= r.y + r.height.saturating_sub(1) {
            return None;
        }
        Some((row - r.y - 1) as usize)
    }
}

/// Returns `true` if `(col, row)` lies inside `r`.
pub fn rect_contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}
