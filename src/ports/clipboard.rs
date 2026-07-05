//! System clipboard port.

use super::BwError;
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
    fn write(&self, text: &str) -> Result<(), BwError>;

    /// Writes `text` and arranges for it to be cleared after
    /// `clear_after_secs` seconds, *if and only if* the clipboard
    /// content still matches `text` at that time. A value of `0`
    /// disables the auto-clear and is equivalent to [`Self::write`].
    ///
    /// The compare-before-clear is what keeps the clipboard from
    /// stomping on whatever the user copied in the meantime — if they
    /// already moved on to something else, we leave their selection
    /// alone.
    ///
    /// The default impl forwards to [`Self::write`] so adapters that
    /// don't support auto-clear (e.g. test fakes) keep working without
    /// having to opt in.
    fn write_with_clear(&self, text: &str, clear_after_secs: u64) -> Result<(), BwError> {
        let _ = clear_after_secs;
        self.write(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Minimal in-memory fake — captures the last `write` call so we can
    /// assert that the trait's default `write_with_clear` impl forwards
    /// to it (and ignores the TTL, which a fake can't honour anyway).
    #[derive(Default)]
    struct FakeClipboard {
        last: RefCell<Option<String>>,
    }
    impl ClipboardPort for FakeClipboard {
        fn write(&self, text: &str) -> Result<(), BwError> {
            *self.last.borrow_mut() = Some(text.to_string());
            Ok(())
        }
    }

    #[test]
    fn default_write_with_clear_forwards_to_write() {
        let c = FakeClipboard::default();
        c.write_with_clear("hello", 30).unwrap();
        assert_eq!(c.last.borrow().as_deref(), Some("hello"));
    }

    #[test]
    fn default_write_with_clear_ignores_ttl_argument() {
        // The default impl must not treat 0 specially — both branches
        // are expected to forward to `write` unmodified.
        let c = FakeClipboard::default();
        c.write_with_clear("a", 0).unwrap();
        assert_eq!(c.last.borrow().as_deref(), Some("a"));
        c.write_with_clear("b", 9999).unwrap();
        assert_eq!(c.last.borrow().as_deref(), Some("b"));
    }
}
