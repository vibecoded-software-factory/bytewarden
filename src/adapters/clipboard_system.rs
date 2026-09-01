use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use zeroize::Zeroizing;

use crate::ports::{BwError, ClipboardPort};

#[derive(Debug, Default)]
pub struct SystemClipboardAdapter;

struct Backend {
    write_argv: Vec<&'static str>,
    read_argv: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Session {
    Wayland,
    X11,
    MacOs,
    Headless,
}

enum BackendChoice {
    Use(Backend),

    MissingTool { hint: &'static str },

    Headless,
}

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
    pub fn new() -> Self {
        Self
    }

    fn choose() -> BackendChoice {
        select_backend(detect_session(), &tool_available)
    }

    fn missing_tool_err(hint: &str) -> BwError {
        BwError::Spawn(format!(
            "no clipboard tool found for this session — install {hint}"
        ))
    }

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

            BackendChoice::Headless => {
                Self::write_osc52(text);
                Ok(())
            }

            BackendChoice::MissingTool { hint } => Err(Self::missing_tool_err(hint)),
        }
    }

    fn write_with_clear(&self, text: &str, clear_after_secs: u64) -> Result<(), BwError> {
        let backend = match Self::choose() {
            BackendChoice::Use(backend) => backend,
            BackendChoice::Headless => {
                Self::write_osc52(text);
                return Ok(());
            }
            BackendChoice::MissingTool { hint } => return Err(Self::missing_tool_err(hint)),
        };
        Self::write_via(&backend.write_argv, text)?;

        if clear_after_secs == 0 {
            return Ok(());
        }

        let payload = Zeroizing::new(text.to_string());
        let write_argv = backend.write_argv.clone();
        let read_argv = backend.read_argv.clone();

        thread::spawn(move || {
            thread::sleep(Duration::from_secs(clear_after_secs));

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
        assert!(matches!(
            select_backend(Session::Headless, &probe(&["wl-copy", "xclip"])),
            BackendChoice::Headless
        ));
    }

    #[test]
    fn write_with_clear_zero_disables_timer() {
        if !matches!(SystemClipboardAdapter::choose(), BackendChoice::Use(_)) {
            return;
        }
        let a = SystemClipboardAdapter::new();
        let started = std::time::Instant::now();
        let _ = a.write_with_clear("ignored", 0);

        assert!(
            started.elapsed() < Duration::from_secs(1),
            "write_with_clear(0) should not block on a timer"
        );
    }
}
