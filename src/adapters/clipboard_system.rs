use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use zeroize::Zeroizing;

use crate::ports::{AutoClear, BwError, ClipboardPort};

const TOOL_TIMEOUT_SECS: u64 = 3;

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

    fn wait_deadline(child: &mut Child, secs: u64, tool: &str) -> Result<ExitStatus, BwError> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            match child
                .try_wait()
                .map_err(|e| BwError::Internal(format!("{tool} wait error: {e}")))?
            {
                Some(status) => return Ok(status),
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(BwError::Timeout {
                        label: tool.to_string(),
                        secs,
                    });
                }
                None => thread::sleep(Duration::from_millis(20)),
            }
        }
    }

    fn exit_err(tool: &str, status: ExitStatus) -> BwError {
        BwError::exit(format!("{tool} failed ({status})"), status.code())
    }

    fn write_via(argv: &[&str], text: &str, secs: u64) -> Result<(), BwError> {
        let tool = argv[0];
        let mut child = Command::new(tool)
            .args(&argv[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| BwError::Spawn(format!("spawn failed: {e}")))?;

        let Some(mut stdin) = child.stdin.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(BwError::Internal(format!("{tool} stdin was not captured")));
        };

        let payload = Zeroizing::new(text.to_string());
        let writer = thread::spawn(move || {
            let res = stdin
                .write_all(payload.as_bytes())
                .and_then(|()| stdin.flush());
            drop(stdin);
            res
        });

        let status = Self::wait_deadline(&mut child, secs, tool)?;
        let written = writer
            .join()
            .map_err(|_| BwError::Internal(format!("{tool} stdin writer panicked")))?;
        if !status.success() {
            return Err(Self::exit_err(tool, status));
        }
        written.map_err(|e| BwError::Internal(format!("could not send the text to {tool}: {e}")))
    }

    fn read_via(argv: &[&str], secs: u64) -> Result<Zeroizing<Vec<u8>>, BwError> {
        let tool = argv[0];
        let mut child = Command::new(tool)
            .args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| BwError::Spawn(format!("spawn failed: {e}")))?;

        let Some(mut stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(BwError::Internal(format!("{tool} stdout was not captured")));
        };

        let reader = thread::spawn(move || {
            let mut buf = Zeroizing::new(Vec::new());
            stdout.read_to_end(&mut buf).map(|_| buf)
        });

        let status = Self::wait_deadline(&mut child, secs, tool)?;
        let read = reader
            .join()
            .map_err(|_| BwError::Internal(format!("{tool} stdout reader panicked")))?;
        if !status.success() {
            return Err(Self::exit_err(tool, status));
        }
        read.map_err(|e| {
            BwError::Internal(format!("could not read the clipboard from {tool}: {e}"))
        })
    }
}

impl SystemClipboardAdapter {
    fn write_osc52(text: &str) -> Result<(), BwError> {
        use std::io::Write;
        let encoded = Zeroizing::new(crate::adapters::bw_cli::codec::base64_encode(text));
        let seq = Zeroizing::new(format!("\x1b]52;c;{}\x07", encoded.as_str()));
        let mut out = std::io::stdout();
        out.write_all(seq.as_bytes())
            .and_then(|()| out.flush())
            .map_err(|e| BwError::Internal(format!("could not write the OSC 52 sequence: {e}")))
    }
}

impl ClipboardPort for SystemClipboardAdapter {
    fn write(&self, text: &str) -> Result<(), BwError> {
        match Self::choose() {
            BackendChoice::Use(backend) => {
                Self::write_via(&backend.write_argv, text, TOOL_TIMEOUT_SECS)
            }

            BackendChoice::Headless => Self::write_osc52(text),

            BackendChoice::MissingTool { hint } => Err(Self::missing_tool_err(hint)),
        }
    }

    fn write_with_clear(&self, text: &str, clear_after_secs: u64) -> Result<AutoClear, BwError> {
        let backend = match Self::choose() {
            BackendChoice::Use(backend) => backend,
            BackendChoice::Headless => {
                Self::write_osc52(text)?;
                return Ok(if clear_after_secs == 0 {
                    AutoClear::Off
                } else {
                    AutoClear::Unsupported
                });
            }
            BackendChoice::MissingTool { hint } => return Err(Self::missing_tool_err(hint)),
        };
        Self::write_via(&backend.write_argv, text, TOOL_TIMEOUT_SECS)?;

        if clear_after_secs == 0 {
            return Ok(AutoClear::Off);
        }

        let payload = Zeroizing::new(text.to_string());
        let write_argv = backend.write_argv.clone();
        let read_argv = backend.read_argv.clone();

        thread::spawn(move || {
            thread::sleep(Duration::from_secs(clear_after_secs));

            let Ok(current) = Self::read_via(&read_argv, TOOL_TIMEOUT_SECS) else {
                return;
            };
            if current.as_slice() == payload.as_bytes() {
                let _ = Self::write_via(&write_argv, "", TOOL_TIMEOUT_SECS);
            }
        });

        Ok(AutoClear::Scheduled)
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

    fn alive(pid: &str) -> bool {
        Command::new("kill")
            .args(["-0", pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[test]
    fn write_via_delivers_the_text_and_reaps_the_tool() {
        let dir = tempfile::TempDir::new().unwrap();
        let out = dir.path().join("out");
        let pid = dir.path().join("pid");
        let script = format!("echo $$ > '{}'; cat > '{}'", pid.display(), out.display());
        SystemClipboardAdapter::write_via(&["sh", "-c", &script], "s3cret", 5)
            .expect("a tool that reads stdin and exits 0 is a success");
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "s3cret");
        let pid = std::fs::read_to_string(&pid).unwrap();
        assert!(
            !alive(pid.trim()),
            "the tool was left running or as a zombie"
        );
    }

    #[test]
    fn write_via_reports_a_tool_that_exits_nonzero() {
        let res =
            SystemClipboardAdapter::write_via(&["sh", "-c", "cat > /dev/null; exit 3"], "x", 5);
        assert!(
            matches!(
                res,
                Err(BwError::Exit {
                    status: Some(3),
                    ..
                })
            ),
            "a failed clipboard tool must not pass as a copy"
        );
    }

    #[test]
    fn write_via_reports_text_the_tool_never_read() {
        let payload = "A".repeat(256 * 1024);
        let res = SystemClipboardAdapter::write_via(&["sh", "-c", "exit 0"], &payload, 5);
        match res {
            Err(BwError::Internal(msg)) => assert!(msg.contains("could not send"), "{msg}"),
            Err(e) => panic!("expected a write error, got {e}"),
            Ok(()) => panic!("a truncated copy must not pass as success"),
        }
    }

    #[test]
    fn write_via_times_out_on_a_tool_that_hangs() {
        let started = Instant::now();
        let res = SystemClipboardAdapter::write_via(&["sh", "-c", "exec sleep 5"], "x", 1);
        assert!(matches!(res, Err(BwError::Timeout { .. })));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn write_via_returns_while_a_forked_server_keeps_running() {
        let started = Instant::now();
        SystemClipboardAdapter::write_via(
            &["sh", "-c", "cat > /dev/null; (sleep 5 &) ; exit 0"],
            "x",
            5,
        )
        .expect("the foreground tool exited 0");
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "waiting must stop at the foreground process, as with wl-copy/xclip/xsel"
        );
    }

    #[test]
    fn read_via_returns_the_tool_output() {
        let out = SystemClipboardAdapter::read_via(&["sh", "-c", "printf s3cret"], 5).unwrap();
        assert_eq!(out.as_slice(), b"s3cret");
    }

    #[test]
    fn read_via_reports_a_tool_that_exits_nonzero() {
        let res = SystemClipboardAdapter::read_via(&["sh", "-c", "printf x; exit 1"], 5);
        assert!(matches!(
            res,
            Err(BwError::Exit {
                status: Some(1),
                ..
            })
        ));
    }

    #[test]
    fn read_via_times_out_on_a_tool_that_hangs() {
        let started = Instant::now();
        let res = SystemClipboardAdapter::read_via(&["sh", "-c", "exec sleep 5"], 1);
        assert!(matches!(res, Err(BwError::Timeout { .. })));
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
