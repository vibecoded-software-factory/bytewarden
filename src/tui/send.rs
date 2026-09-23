use crate::domain::LineEditor;

pub const SEND_MIN_DAYS: u8 = 1;
pub const SEND_MAX_DAYS: u8 = 31;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendFocus {
    Name,
    Days,
    Content,
}

#[derive(Debug, Clone)]
pub struct SendCreateState {
    pub name: LineEditor,
    pub days: u8,
    pub content: LineEditor,
    pub focus: SendFocus,
}

impl SendCreateState {
    pub fn new() -> Self {
        Self {
            name: LineEditor::new(),
            days: 7,
            content: LineEditor::new(),
            focus: SendFocus::Name,
        }
    }
}

impl Default for SendCreateState {
    fn default() -> Self {
        Self::new()
    }
}
