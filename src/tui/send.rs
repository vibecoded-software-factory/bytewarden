//! Send-create popup state.

/// Bw enforces 1–31 days; we clamp at the UI layer too so the
/// adjuster keys never produce out-of-range values.
pub const SEND_MIN_DAYS: u8 = 1;
pub const SEND_MAX_DAYS: u8 = 31;

/// Which control of the send-create popup currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendFocus {
    Name,
    Days,
    Content,
}

/// Buffer for the in-flight send-create popup.
#[derive(Debug, Clone)]
pub struct SendCreateState {
    pub name: String,
    pub name_cursor: usize,
    pub days: u8,
    pub content: String,
    pub content_cursor: usize,
    pub focus: SendFocus,
}

impl SendCreateState {
    /// Builds a fresh popup with sensible defaults: empty name + 7-day
    /// expiration (the bw default), focus parked on Name so the user
    /// can start typing.
    pub fn new() -> Self {
        Self {
            name: String::new(),
            name_cursor: 0,
            days: 7,
            content: String::new(),
            content_cursor: 0,
            focus: SendFocus::Name,
        }
    }
}

impl Default for SendCreateState {
    fn default() -> Self {
        Self::new()
    }
}
