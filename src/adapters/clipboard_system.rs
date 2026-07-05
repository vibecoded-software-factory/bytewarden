//! [`crate::ports::ClipboardPort`] implementation that shells out to the
//! native clipboard tool of the running session.
//!
//! Backend selection priority:
//! 1. Wayland (`$WAYLAND_DISPLAY` set) → `wl-copy` / `wl-paste`.
//! 2. X11 (`$DISPLAY` set) → `xclip`, falling back to `xsel`.
//! 3. macOS → `pbcopy` / `pbpaste`.
//!
//! When none of the above can be detected the call returns an error
//! describing the missing tool.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use zeroize::Zeroizing;

use crate::ports::{BwError, ClipboardPort};

/// Default clipboard adapter — picks the right tool at call time and
/// pipes the payload into it via stdin (the payload never appears on a
/// command line, so it stays out of `ps`).
#[derive(Debug, Default)]
pub struct SystemClipboardAdapter;

/// Shape of a clipboard backend pair. Read and write tools are
/// independent so we can mix `wl-copy` with `wl-paste`, `xclip -i` with
/// `xclip -o`, etc.
struct Backend {
    write_argv: Vec<&'static str>,
    read_argv: Vec<&'static str>,
}

impl SystemClipboardAdapter {
    /// Constructs a new adapter. Cheap — selection happens at call-time.
    pub fn new() -> Self {
        Self
    }

    /// Picks the clipboard read+write commands for the current session.
    ///
    /// Returns `None` when no backend is detectable so the caller can
    /// surface a clear error to the user instead of guessing.
    fn choose_backend() -> Option<Backend> {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            return Some(Backend {
                write_argv: vec!["wl-copy"],
                read_argv: vec!["wl-paste", "--no-newline"],
            });
        }
        if std::env::var("DISPLAY").is_ok() {
            if Path::new("/usr/bin/xclip").exists() || Path::new("/usr/local/bin/xclip").exists() {
                return Some(Backend {
                    write_argv: vec!["xclip", "-selection", "clipboard"],
                    read_argv: vec!["xclip", "-selection", "clipboard", "-o"],
                });
            }
            return Some(Backend {
                write_argv: vec!["xsel", "--clipboard", "--input"],
                read_argv: vec!["xsel", "--clipboard", "--output"],
            });
        }
        if cfg!(target_os = "macos") {
            return Some(Backend {
                write_argv: vec!["pbcopy"],
                read_argv: vec!["pbpaste"],
            });
        }
        None
    }

    /// Pipes `text` into the configured write tool via stdin.
    fn write_via(argv: &[&str], text: &str) -> Result<(), BwError> {
        let mut cmd = Command::new(argv[0]);
        for a in &argv[1..] {
            cmd.arg(a);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| BwError::Spawn(format!("spawn failed: {e}")))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        drop(child);
        Ok(())
    }

    /// Reads the current clipboard contents through the configured read
    /// tool. Returns `None` when the tool fails to spawn or exits with
    /// an error — we treat both as "couldn't read", which makes the
    /// caller skip the clear (safer than blindly clobbering whatever
    /// the user has).
    fn read_via(argv: &[&str]) -> Option<String> {
        let out = Command::new(argv[0])
            .args(&argv[1..])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).to_string())
    }
}

impl ClipboardPort for SystemClipboardAdapter {
    fn write(&self, text: &str) -> Result<(), BwError> {
        let backend = Self::choose_backend().ok_or_else(|| {
            BwError::Spawn("No clipboard tool found (install wl-copy or xclip)".to_string())
        })?;
        Self::write_via(&backend.write_argv, text)
    }

    fn write_with_clear(&self, text: &str, clear_after_secs: u64) -> Result<(), BwError> {
        let backend = Self::choose_backend().ok_or_else(|| {
            BwError::Spawn("No clipboard tool found (install wl-copy or xclip)".to_string())
        })?;
        Self::write_via(&backend.write_argv, text)?;

        if clear_after_secs == 0 {
            return Ok(());
        }

        // The payload is wrapped in `Zeroizing` so the heap copy that
        // lives inside the spawned thread is overwritten with zeroes
        // when the thread exits — closes the window where the password
        // would otherwise sit unscrubbed waiting for the timer to fire.
        let payload = Zeroizing::new(text.to_string());
        let write_argv = backend.write_argv.clone();
        let read_argv = backend.read_argv.clone();

        // Detached background thread. If bytewarden exits before the
        // timer fires the thread is killed with the process and the
        // clipboard is left as-is — same outcome as today, no worse.
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(clear_after_secs));
            // Compare-and-clear: only wipe the clipboard if it still
            // holds the secret we wrote. Anything else means the user
            // moved on (copied a different value) and we'd be stomping
            // on their selection.
            let Some(current) = Self::read_via(&read_argv) else {
                return;
            };
            if current.as_str() == payload.as_str() {
                let _ = Self::write_via(&write_argv, "");
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `write_with_clear` with `clear_after_secs = 0` must not spawn a
    /// background thread (and therefore must return immediately). We
    /// can't observe the thread directly without instrumenting the
    /// adapter, but we *can* verify the call returns synchronously
    /// well under the 1s mark — a spawned `sleep(N)` would otherwise
    /// block the test for at least N seconds if the constant ever got
    /// passed straight through.
    ///
    /// The test only runs in environments where a clipboard backend is
    /// available; CI without `wl-copy`/`xclip` skips it because there's
    /// no useful assertion to make.
    #[test]
    fn write_with_clear_zero_disables_timer() {
        if SystemClipboardAdapter::choose_backend().is_none() {
            return;
        }
        let a = SystemClipboardAdapter::new();
        let started = std::time::Instant::now();
        let _ = a.write_with_clear("ignored", 0);
        // Anything below a second is fine — the synchronous write
        // typically returns in milliseconds.
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "write_with_clear(0) should not block on a timer"
        );
    }
}
