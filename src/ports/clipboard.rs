use super::BwError;

pub trait ClipboardPort {
    fn write(&self, text: &str) -> Result<(), BwError>;

    fn write_with_clear(&self, text: &str, clear_after_secs: u64) -> Result<(), BwError> {
        let _ = clear_after_secs;
        self.write(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

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
        let c = FakeClipboard::default();
        c.write_with_clear("a", 0).unwrap();
        assert_eq!(c.last.borrow().as_deref(), Some("a"));
        c.write_with_clear("b", 9999).unwrap();
        assert_eq!(c.last.borrow().as_deref(), Some("b"));
    }
}
