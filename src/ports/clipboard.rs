//! System clipboard port.

/// Abstraction over the operating-system clipboard.
///
/// Implementations can shell out to `wl-copy`/`xclip`/`pbcopy`, link to a
/// platform API, or be a simple in-memory fake for tests.
pub trait ClipboardPort {
    /// Writes `text` to the system clipboard.
    ///
    /// # Errors
    ///
    /// Returns an error string when no clipboard backend is available
    /// or when the underlying command fails to start.
    fn write(&self, text: &str) -> Result<(), String>;
}
