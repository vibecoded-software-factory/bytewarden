//! [`crate::ports::ClipboardPort`] implementation that shells out to the
//! native clipboard tool of the running session.
//!
//! Backend selection priority:
//! 1. Wayland (`$WAYLAND_DISPLAY` set) → `wl-copy`.
//! 2. X11 (`$DISPLAY` set) → `xclip`, falling back to `xsel`.
//! 3. macOS → `pbcopy`.
//!
//! When none of the above can be detected the call returns an error
//! describing the missing tool.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::ports::ClipboardPort;

/// Default clipboard adapter — picks the right tool at call time and
/// pipes the payload into it via stdin (the payload never appears on a
/// command line, so it stays out of `ps`).
#[derive(Debug, Default)]
pub struct SystemClipboardAdapter;

impl SystemClipboardAdapter {
    /// Constructs a new adapter. Cheap — selection happens at write-time.
    pub fn new() -> Self {
        Self
    }

    /// Picks the clipboard command + arguments for the current session.
    fn choose_backend() -> Option<Vec<&'static str>> {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            return Some(vec!["wl-copy"]);
        }
        if std::env::var("DISPLAY").is_ok() {
            if Path::new("/usr/bin/xclip").exists() || Path::new("/usr/local/bin/xclip").exists() {
                return Some(vec!["xclip", "-selection", "clipboard"]);
            }
            return Some(vec!["xsel", "--clipboard", "--input"]);
        }
        if cfg!(target_os = "macos") {
            return Some(vec!["pbcopy"]);
        }
        None
    }
}

impl ClipboardPort for SystemClipboardAdapter {
    fn write(&self, text: &str) -> Result<(), String> {
        let args = Self::choose_backend()
            .ok_or_else(|| "No clipboard tool found (install wl-copy or xclip)".to_string())?;

        let mut cmd = Command::new(args[0]);
        for a in &args[1..] {
            cmd.arg(a);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        drop(child);
        Ok(())
    }
}
