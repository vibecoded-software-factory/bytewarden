use crate::tui::action::CmdEntry;

const LIMIT: usize = 50;

#[derive(Default)]
pub struct CmdLog {
    pub entries: Vec<CmdEntry>,

    pub scroll: usize,
}

impl CmdLog {
    pub fn push(&mut self, entry: CmdEntry) {
        self.entries.push(entry);
        if self.entries.len() > LIMIT {
            self.entries.remove(0);
        }
        self.scroll = 0;
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.scroll = 0;
    }

    pub fn scroll_up(&mut self, n: usize) {
        let max = self.entries.len().saturating_sub(1);
        self.scroll = (self.scroll + n).min(max);
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
    }
}
