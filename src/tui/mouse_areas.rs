use ratatui::layout::Rect;

use crate::tui::screens::Focus;

#[derive(Debug, Clone, Default)]
pub struct MouseAreas {
    pub status: Option<Rect>,
    pub search: Option<Rect>,
    pub folders: Option<Rect>,
    pub items: Option<Rect>,
    pub list: Option<Rect>,
    pub cmdlog: Option<Rect>,
}

impl MouseAreas {
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

    pub fn list_row(&self, row: u16) -> Option<usize> {
        let r = self.list?;
        if row < r.y + 1 || row >= r.y + r.height.saturating_sub(1) {
            return None;
        }
        Some((row - r.y - 1) as usize)
    }

    pub fn items_row(&self, row: u16) -> Option<usize> {
        let r = self.items?;
        if row < r.y + 1 || row >= r.y + r.height.saturating_sub(1) {
            return None;
        }
        Some((row - r.y - 1) as usize)
    }

    pub fn folders_row(&self, row: u16) -> Option<usize> {
        let r = self.folders?;
        if row < r.y + 1 || row >= r.y + r.height.saturating_sub(1) {
            return None;
        }
        Some((row - r.y - 1) as usize)
    }
}

pub fn rect_contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}
