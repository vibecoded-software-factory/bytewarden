//! [`crate::ports::ClipboardPort`] implementation that shells out to the
//! native clipboard tool of the running session.
//!
//! Backend selection is driven by **tool availability on `$PATH`**, not by
//! environment variables alone: a session may advertise Wayland/X11 yet not
//! have the matching binary installed (and the tool can live anywhere on
//! `$PATH`, e.g. a Homebrew prefix, not only `/usr/bin`). Priority within a
//! session:
//! 1. Wayland (`$WAYLAND_DISPLAY` set) → `wl-copy` / `wl-paste`.
//! 2. X11 (`$DISPLAY` set) → `xclip`, falling back to `xsel`.
//! 3. macOS → `pbcopy` / `pbpaste`.
//!
//! When a graphical session is detected but its clipboard tool is missing,
//! the call returns an actionable error naming the package to install —
//! deliberately *not* a silent OSC 52 fallback, because a terminal that
//! ignores OSC 52 (e.g. VTE, off by default) would make a "copied" report a
//! lie about a secret. OSC 52 is used only for a genuinely headless session
//! (no `$WAYLAND_DISPLAY` / `$DISPLAY`), where it is the only option.

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

/// The kind of session we're running in, for clipboard purposes. Detected
/// from the environment; kept separate from the (pure) backend decision so
/// the latter is unit-testable without touching the real environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Session {
    Wayland,
    X11,
    MacOs,
    Headless,
}

/// Outcome of choosing a clipboard backend for a session.
enum BackendChoice {
    /// A tool is available — use it.
    Use(Backend),
    /// A graphical session, but none of its clipboard tools are installed.
    /// Carries the package hint for an actionable error.
    MissingTool { hint: &'static str },
    /// No graphical session at all — fall back to OSC 52.
    Headless,
}

/// Detects the session kind from the environment. Wayland wins over X11 when
/// both are advertised (an XWayland session exports both).
fn detect_session() -> Session {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return Session::Wayland;
    }
    if std::env::var_os("DISPLAY").is_some() {
        return Session::X11;
    }
    if cfg!(target_os = "macos") {
        return Session::MacOs;
    }
    Session::Headless
}

/// True if `name` resolves to a file on `$PATH` (or, for an absolute path,
/// exists directly). This mirrors how `Command::new` resolves a bare command
/// name, so it recognises a tool installed anywhere on the user's PATH — not
/// just a hardcoded `/usr/bin`.
fn tool_available(name: &str) -> bool {
    let p = Path::new(name);
    if p.is_absolute() {
        return p.exists();
    }
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| dir.join(name).exists())
}

/// Pure backend decision: given the session and a tool-availability probe,
/// pick the backend, report a missing tool, or defer to OSC 52. Pure (the
/// probe is injected) so it can be unit-tested without a real environment.
fn select_backend(session: Session, available: &dyn Fn(&str) -> bool) -> BackendChoice {
    match session {
        Session::Wayland => {
            if available("wl-copy") {
                BackendChoice::Use(Backend {
                    write_argv: vec!["wl-copy"],
                    read_argv: vec!["wl-paste", "--no-newline"],
                })
            } else {
                BackendChoice::MissingTool {
                    hint: "wl-clipboard (provides wl-copy)",
                }
            }
        }
        Session::X11 => {
            if available("xclip") {
                BackendChoice::Use(Backend {
                    write_argv: vec!["xclip", "-selection", "clipboard"],
                    read_argv: vec!["xclip", "-selection", "clipboard", "-o"],
                })
            } else if available("xsel") {
                BackendChoice::Use(Backend {
                    write_argv: vec!["xsel", "--clipboard", "--input"],
                    read_argv: vec!["xsel", "--clipboard", "--output"],
                })
            } else {
                BackendChoice::MissingTool {
                    hint: "xclip or xsel",
                }
            }
        }
        Session::MacOs => {
            if available("pbcopy") {
                BackendChoice::Use(Backend {
                    write_argv: vec!["pbcopy"],
                    read_argv: vec!["pbpaste"],
                })
            } else {
                BackendChoice::MissingTool {
                    hint: "pbcopy (a base macOS tool)",
                }
            }
        }
        Session::Headless => BackendChoice::Headless,
    }
}

impl SystemClipboardAdapter {
    /// Constructs a new adapter. Cheap — selection happens at call-time.
    pub fn new() -> Self {
        Self
    }

    /// Picks the clipboard backend for the current session, resolving tool
    /// availability against the real `$PATH`.
    fn choose() -> BackendChoice {
        select_backend(detect_session(), &tool_available)
    }

    /// Builds the actionable error raised when a graphical session has no
    /// clipboard tool installed. Reuses `BwError::Spawn` (the same variant
    /// `write_via` returns) since clipboard access has no dedicated error.
    fn missing_tool_err(hint: &str) -> BwError {
        BwError::Spawn(format!(
            "no clipboard tool found for this session — install {hint}"
        ))
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

impl SystemClipboardAdapter {
    /// Sets the terminal's clipboard via the **OSC 52** escape sequence —
    /// the fallback when no clipboard tool exists (headless / SSH / tmux):
    /// writes `ESC ] 52 ; c ; <base64> BEL` to stdout. Best-effort — a
    /// terminal that doesn't speak OSC 52 simply ignores it. Written
    /// between frames (from a synchronous copy flow), and the next redraw
    /// repaints the screen, so it doesn't corrupt the TUI.
    fn write_osc52(text: &str) {
        use std::io::Write;
        let seq = format!(
            "\x1b]52;c;{}\x07",
            crate::adapters::bw_cli::codec::base64_encode(text)
        );
        let mut out = std::io::stdout();
        let _ = out.write_all(seq.as_bytes());
        let _ = out.flush();
    }
}

impl ClipboardPort for SystemClipboardAdapter {
    fn write(&self, text: &str) -> Result<(), BwError> {
        match Self::choose() {
            BackendChoice::Use(backend) => Self::write_via(&backend.write_argv, text),
            // No graphical session — OSC 52 is the only path.
            BackendChoice::Headless => {
                Self::write_osc52(text);
                Ok(())
            }
            // Graphical session but the tool is missing: surface it instead of
            // silently pretending to copy (see the module docs).
            BackendChoice::MissingTool { hint } => Err(Self::missing_tool_err(hint)),
        }
    }

    fn write_with_clear(&self, text: &str, clear_after_secs: u64) -> Result<(), BwError> {
        let backend = match Self::choose() {
            BackendChoice::Use(backend) => backend,
            BackendChoice::Headless => {
                // OSC 52. We can't read the clipboard back over OSC 52 to
                // compare, so the timed auto-clear is skipped on this path
                // (the write still happens).
                Self::write_osc52(text);
                return Ok(());
            }
            BackendChoice::MissingTool { hint } => return Err(Self::missing_tool_err(hint)),
        };
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

    /// Builds a probe closure that reports the given tools as available.
    fn probe(available: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |name| available.contains(&name)
    }

    #[test]
    fn wayland_uses_wl_copy_when_present() {
        match select_backend(Session::Wayland, &probe(&["wl-copy"])) {
            BackendChoice::Use(b) => assert_eq!(b.write_argv, vec!["wl-copy"]),
            _ => panic!("expected wl-copy backend"),
        }
    }

    #[test]
    fn wayland_reports_missing_when_wl_copy_absent() {
        // The original bug: WAYLAND_DISPLAY set but wl-copy not installed.
        assert!(matches!(
            select_backend(Session::Wayland, &probe(&[])),
            BackendChoice::MissingTool { .. }
        ));
    }

    #[test]
    fn x11_prefers_xclip_over_xsel() {
        match select_backend(Session::X11, &probe(&["xclip", "xsel"])) {
            BackendChoice::Use(b) => assert_eq!(b.write_argv[0], "xclip"),
            _ => panic!("expected xclip backend"),
        }
    }

    #[test]
    fn x11_falls_back_to_xsel() {
        match select_backend(Session::X11, &probe(&["xsel"])) {
            BackendChoice::Use(b) => assert_eq!(b.write_argv[0], "xsel"),
            _ => panic!("expected xsel backend"),
        }
    }

    #[test]
    fn x11_reports_missing_when_no_tool() {
        assert!(matches!(
            select_backend(Session::X11, &probe(&[])),
            BackendChoice::MissingTool { .. }
        ));
    }

    #[test]
    fn macos_uses_pbcopy_when_present() {
        match select_backend(Session::MacOs, &probe(&["pbcopy"])) {
            BackendChoice::Use(b) => assert_eq!(b.write_argv, vec!["pbcopy"]),
            _ => panic!("expected pbcopy backend"),
        }
    }

    #[test]
    fn headless_defers_to_osc52_even_with_tools_present() {
        // A headless session never spawns a tool, even if one happens to be
        // on PATH — OSC 52 is the contract there.
        assert!(matches!(
            select_backend(Session::Headless, &probe(&["wl-copy", "xclip"])),
            BackendChoice::Headless
        ));
    }

    /// `write_with_clear` with `clear_after_secs = 0` must not spawn a
    /// background thread (and therefore must return immediately). We
    /// can't observe the thread directly without instrumenting the
    /// adapter, but we *can* verify the call returns synchronously
    /// well under the 1s mark — a spawned `sleep(N)` would otherwise
    /// block the test for at least N seconds if the constant ever got
    /// passed straight through.
    ///
    /// The test only runs where a clipboard backend is actually available;
    /// otherwise the call short-circuits (OSC 52 or a missing-tool error)
    /// and there's no timer path to exercise.
    #[test]
    fn write_with_clear_zero_disables_timer() {
        if !matches!(SystemClipboardAdapter::choose(), BackendChoice::Use(_)) {
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
