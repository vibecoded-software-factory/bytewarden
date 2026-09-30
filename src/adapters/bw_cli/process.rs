use crate::ports::BwError;
use std::io::{Read, Write};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

pub const BW_PASSWORD_ENV: &str = "BW_PASS_INPUT";

pub const BW_SESSION_ENV: &str = "BW_SESSION";

const NOINTERACTION: &str = "--nointeraction";

const LOCAL_OP_FALLBACK_TIMEOUT: u64 = 10;

fn full_args<'a>(args: &'a [&'a str]) -> Vec<&'a str> {
    let mut v = Vec::with_capacity(args.len() + 1);
    v.push(NOINTERACTION);
    v.extend_from_slice(args);
    v
}

fn join_reader<T>(handle: std::thread::JoinHandle<T>, stream: &str) -> Result<T, BwError> {
    handle
        .join()
        .map_err(|_| BwError::Internal(format!("bw {stream} reader panicked")))
}

fn wait_with_timeout(mut child: Child, secs: u64, label: &str) -> Result<Output, BwError> {
    let deadline = Instant::now() + Duration::from_secs(secs);

    let stdout_thread = child.stdout.take().map(|mut s| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = s.read_to_end(&mut buf);
            buf
        })
    });
    let stderr_thread = child.stderr.take().map(|mut s| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = s.read_to_end(&mut buf);
            buf
        })
    });

    loop {
        match child
            .try_wait()
            .map_err(|e| BwError::Internal(format!("bw wait error: {e}")))?
        {
            Some(status) => {
                let stdout = stdout_thread
                    .map(|t| join_reader(t, "stdout"))
                    .transpose()?
                    .unwrap_or_default();
                let stderr = stderr_thread
                    .map(|t| join_reader(t, "stderr"))
                    .transpose()?
                    .unwrap_or_default();
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();

                    let _ = stdout_thread.and_then(|t| t.join().ok());
                    let _ = stderr_thread.and_then(|t| t.join().ok());
                    return Err(BwError::Timeout {
                        label: label.to_string(),
                        secs,
                    });
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

pub fn bw_run(args: &[&str]) -> Result<Output, BwError> {
    bw_run_timeout(args, LOCAL_OP_FALLBACK_TIMEOUT)
}

pub fn bw_run_timeout(args: &[&str], secs: u64) -> Result<Output, BwError> {
    let child = Command::new("bw")
        .args(full_args(args))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BwError::Spawn(format!("Could not run bw: {e}")))?;
    wait_with_timeout(child, secs, "bw")
}

pub fn bw_run_with_password(args: &[&str], password: &str) -> Result<Output, BwError> {
    bw_run_with_password_timeout(args, password, LOCAL_OP_FALLBACK_TIMEOUT)
}

pub fn bw_run_with_password_timeout(
    args: &[&str],
    password: &str,
    secs: u64,
) -> Result<Output, BwError> {
    let child = Command::new("bw")
        .args(full_args(args))
        .env(BW_PASSWORD_ENV, password)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BwError::Spawn(format!("Could not run bw: {e}")))?;
    wait_with_timeout(child, secs, "bw")
}

pub fn bw_run_with_session(args: &[&str], session: &str) -> Result<Output, BwError> {
    bw_run_with_session_timeout(args, session, LOCAL_OP_FALLBACK_TIMEOUT)
}

pub fn bw_run_with_session_timeout(
    args: &[&str],
    session: &str,
    secs: u64,
) -> Result<Output, BwError> {
    let child = Command::new("bw")
        .args(full_args(args))
        .env(BW_SESSION_ENV, session)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BwError::Spawn(format!("Could not run bw: {e}")))?;
    wait_with_timeout(child, secs, "bw")
}

pub fn bw_run_with_session_and_stdin_timeout(
    args: &[&str],
    session: &str,
    stdin_input: &str,
    secs: u64,
) -> Result<Output, BwError> {
    let mut cmd = Command::new("bw");
    cmd.args(full_args(args)).env(BW_SESSION_ENV, session);
    run_with_stdin_timeout(cmd, stdin_input, secs, "bw")
}

fn run_with_stdin_timeout(
    mut cmd: Command,
    stdin_input: &str,
    secs: u64,
    label: &str,
) -> Result<Output, BwError> {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BwError::Spawn(format!("Could not run bw: {e}")))?;

    let Some(mut sin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(BwError::Internal("bw stdin was not captured".into()));
    };

    let payload = Zeroizing::new(stdin_input.to_string());
    let writer = std::thread::spawn(move || {
        let res = sin.write_all(payload.as_bytes()).and_then(|()| sin.flush());
        drop(sin);
        res
    });

    let out = wait_with_timeout(child, secs, label)?;
    let written = writer
        .join()
        .map_err(|_| BwError::Internal("bw stdin writer panicked".into()))?;
    match written {
        Ok(()) => Ok(out),
        Err(_) if !out.status.success() => Ok(out),
        Err(e) => Err(BwError::Internal(format!(
            "could not send the payload to bw: {e}"
        ))),
    }
}

pub fn bw_run_with_password_and_stdin_timeout(
    args: &[&str],
    password: &str,
    stdin_input: &str,
    secs: u64,
) -> Result<Output, BwError> {
    let mut cmd = Command::new("bw");
    cmd.args(args).env(BW_PASSWORD_ENV, password);
    run_with_stdin_timeout(cmd, stdin_input, secs, "bw")
}

pub fn stdout_str(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

pub fn stderr_str(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).trim().to_string()
}

pub enum PromptWait {
    Reached,

    Exited(Box<Output>),

    TimedOut,
}

pub struct InteractiveChild {
    child: Child,
    stdout: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    stderr: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    readers: Vec<std::thread::JoinHandle<()>>,
}

fn pump<R: Read>(mut r: R, buf: &std::sync::Arc<std::sync::Mutex<Vec<u8>>>) {
    let mut chunk = [0u8; 4096];
    loop {
        match r.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if let Ok(mut b) = buf.lock() {
                    b.extend_from_slice(&chunk[..n]);
                }
            }
        }
    }
}

pub fn spawn_interactive(args: &[&str], password: &str) -> Result<InteractiveChild, BwError> {
    let mut child = Command::new("bw")
        .args(args)
        .env(BW_PASSWORD_ENV, password)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BwError::Spawn(format!("Could not run bw: {e}")))?;

    let stdout = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let stderr = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut readers = Vec::new();
    if let Some(out) = child.stdout.take() {
        let buf = std::sync::Arc::clone(&stdout);
        readers.push(std::thread::spawn(move || pump(out, &buf)));
    }
    if let Some(err) = child.stderr.take() {
        let buf = std::sync::Arc::clone(&stderr);
        readers.push(std::thread::spawn(move || pump(err, &buf)));
    }
    Ok(InteractiveChild {
        child,
        stdout,
        stderr,
        readers,
    })
}

impl InteractiveChild {
    pub fn stderr_so_far(&self) -> String {
        self.stderr
            .lock()
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_default()
    }

    pub fn wait_for_prompt(&mut self, markers: &[&str], secs: u64) -> Result<PromptWait, BwError> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            let seen = self.stderr_so_far().to_lowercase();
            if markers.iter().any(|m| seen.contains(m)) {
                return Ok(PromptWait::Reached);
            }
            match self.child.try_wait() {
                Err(e) => return Err(BwError::Internal(format!("bw wait error: {e}"))),
                Ok(Some(status)) => return Ok(PromptWait::Exited(Box::new(self.collect(status)?))),
                Ok(None) => {}
            }
            if Instant::now() >= deadline {
                return Ok(PromptWait::TimedOut);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn submit_line(&mut self, line: &str) -> Result<(), BwError> {
        let mut stdin = self
            .child
            .stdin
            .take()
            .ok_or_else(|| BwError::Internal("bw login stdin is already closed".into()))?;
        stdin
            .write_all(line.as_bytes())
            .and_then(|()| stdin.flush())
            .map_err(|e| BwError::Internal(format!("could not send the code to bw: {e}")))?;
        Ok(())
    }

    pub fn finish(mut self, secs: u64, label: &str) -> Result<Output, BwError> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            match self.child.try_wait() {
                Err(e) => return Err(BwError::Internal(format!("bw wait error: {e}"))),
                Ok(Some(status)) => return self.collect(status),
                Ok(None) => {}
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return Err(BwError::Timeout {
                    label: label.to_string(),
                    secs,
                });
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn collect(&mut self, status: std::process::ExitStatus) -> Result<Output, BwError> {
        for r in self.readers.drain(..) {
            join_reader(r, "output")?;
        }
        let take = |b: &std::sync::Arc<std::sync::Mutex<Vec<u8>>>| {
            b.lock()
                .map(|mut v| std::mem::take(&mut *v))
                .unwrap_or_default()
        };
        Ok(Output {
            status,
            stdout: take(&self.stdout),
            stderr: take(&self.stderr),
        })
    }
}

impl Drop for InteractiveChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interactive_sh(script: &str) -> InteractiveChild {
        let mut child = Command::new("sh")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn sh");
        let stdout = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let stderr = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut readers = Vec::new();
        if let Some(out) = child.stdout.take() {
            let buf = std::sync::Arc::clone(&stdout);
            readers.push(std::thread::spawn(move || pump(out, &buf)));
        }
        if let Some(err) = child.stderr.take() {
            let buf = std::sync::Arc::clone(&stderr);
            readers.push(std::thread::spawn(move || pump(err, &buf)));
        }
        InteractiveChild {
            child,
            stdout,
            stderr,
            readers,
        }
    }

    #[test]
    fn interactive_child_parks_at_its_prompt_and_completes_on_the_submitted_line() {
        let mut c = interactive_sh(
            "printf 'New device verification required. Enter OTP:' >&2; read code; \
             printf '%s' \"$code\"; test \"$code\" = 424242",
        );
        assert!(
            matches!(
                c.wait_for_prompt(&["new device verification required"], 5),
                Ok(PromptWait::Reached)
            ),
            "the child is waiting at its prompt, not exited"
        );
        c.submit_line("424242\n").expect("stdin accepted the code");
        let out = c.finish(5, "test").expect("child exited");
        assert!(out.status.success(), "the code reached the same process");
        assert_eq!(String::from_utf8_lossy(&out.stdout), "424242");
    }

    #[test]
    fn interactive_child_reports_an_early_exit_with_its_output() {
        let mut c = interactive_sh("printf 'Username or password is incorrect.' >&2; exit 1");
        match c.wait_for_prompt(&["new device verification required"], 5) {
            Ok(PromptWait::Exited(out)) => {
                assert!(!out.status.success());
                assert!(stderr_str(&out).contains("incorrect"));
            }
            other => panic!(
                "expected an early exit, got a different outcome: {}",
                matches!(other, Ok(PromptWait::Reached))
            ),
        }
    }

    #[test]
    fn interactive_child_times_out_when_nothing_happens() {
        let mut c = interactive_sh("exec sleep 5");
        assert!(matches!(
            c.wait_for_prompt(&["never appears"], 1),
            Ok(PromptWait::TimedOut)
        ));
    }

    #[test]
    fn dropping_a_parked_child_kills_the_process() {
        let c = interactive_sh("exec sleep 30");
        let pid = c.child.id();
        drop(c);

        let alive = Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(!alive, "the parked child outlived its owner");
    }

    fn spawn_sh(script: &str) -> Child {
        Command::new("sh")
            .args(["-c", script])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn sh")
    }

    #[test]
    fn timeout_returns_output_when_command_finishes_quickly() {
        let child = spawn_sh("printf hello; exit 0");
        let out = wait_with_timeout(child, 5, "test").expect("should not time out");
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout), "hello");
    }

    #[test]
    fn timeout_returns_err_when_command_runs_too_long() {
        let child = spawn_sh("exec sleep 5");
        let started = Instant::now();
        let res = wait_with_timeout(child, 1, "test");
        let elapsed = started.elapsed();
        assert!(res.is_err());
        let msg = res.unwrap_err().to_string();
        assert!(msg.contains("test"));
        assert!(msg.contains("timed out"));

        assert!(elapsed < Duration::from_secs(3), "took {elapsed:?}");
    }

    #[test]
    fn timeout_drains_large_stdout_without_deadlocking() {
        let child = spawn_sh("exec head -c 262144 /dev/zero");
        let started = Instant::now();
        let res = wait_with_timeout(child, 5, "test").expect("must not time out");
        let elapsed = started.elapsed();
        assert!(res.status.success());
        assert_eq!(res.stdout.len(), 262144);
        assert!(elapsed < Duration::from_secs(3), "took {elapsed:?}");
    }

    #[test]
    fn timeout_propagates_nonzero_exit_status() {
        let child = spawn_sh("exit 7");
        let out = wait_with_timeout(child, 5, "test").expect("not timeout");
        assert!(!out.status.success());
        assert_eq!(out.status.code(), Some(7));
    }

    fn sh_command(script: &str) -> Command {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", script]);
        cmd
    }

    #[test]
    fn stdin_runner_delivers_the_payload_and_closes_stdin() {
        let out = run_with_stdin_timeout(sh_command("exec cat"), "eyJuYW1lIjoieCJ9", 5, "test")
            .expect("must not time out");
        assert!(out.status.success(), "cat only exits once stdin hits EOF");
        assert_eq!(String::from_utf8_lossy(&out.stdout), "eyJuYW1lIjoieCJ9");
    }

    #[test]
    fn stdin_runner_echoes_a_large_payload_without_deadlocking() {
        let payload = "A".repeat(256 * 1024);
        let started = Instant::now();
        let out = run_with_stdin_timeout(sh_command("exec cat"), &payload, 5, "test")
            .expect("must not time out");
        let elapsed = started.elapsed();
        assert!(out.status.success());
        assert_eq!(out.stdout.len(), payload.len());
        assert!(out.stdout == payload.as_bytes());
        assert!(elapsed < Duration::from_secs(3), "took {elapsed:?}");
    }

    #[test]
    fn stdin_runner_reports_a_payload_the_child_never_read() {
        let payload = "A".repeat(256 * 1024);
        let res = run_with_stdin_timeout(sh_command("exit 0"), &payload, 5, "test");
        match res {
            Err(BwError::Internal(msg)) => assert!(msg.contains("payload"), "{msg}"),
            Err(e) => panic!("expected an internal write error, got {e}"),
            Ok(_) => panic!("a truncated payload must not pass as success"),
        }
    }

    #[test]
    fn stdin_runner_keeps_the_childs_own_error_when_it_fails_unread() {
        let payload = "A".repeat(256 * 1024);
        let out = run_with_stdin_timeout(
            sh_command(">&2 echo 'Vault is locked.'; exit 1"),
            &payload,
            5,
            "test",
        )
        .expect("the child's failure is returned as output");
        assert_eq!(out.status.code(), Some(1));
        assert_eq!(stderr_str(&out), "Vault is locked.");
    }

    #[test]
    fn stdin_runner_times_out_when_the_child_hangs() {
        let payload = "A".repeat(256 * 1024);
        let started = Instant::now();
        let res = run_with_stdin_timeout(sh_command("exec sleep 5"), &payload, 1, "test");
        let elapsed = started.elapsed();
        assert!(matches!(res, Err(BwError::Timeout { .. })));
        assert!(elapsed < Duration::from_secs(3), "took {elapsed:?}");
    }

    #[test]
    fn a_panicked_reader_surfaces_as_an_internal_error() {
        let reader = std::thread::spawn(|| -> Vec<u8> { panic!("reader blew up") });
        match join_reader(reader, "stdout") {
            Err(BwError::Internal(msg)) => assert_eq!(msg, "bw stdout reader panicked"),
            Err(e) => panic!("expected an internal error, got {e}"),
            Ok(_) => panic!("a panicked reader must not pass as empty output"),
        }
    }

    #[test]
    fn a_finished_reader_hands_back_its_output() {
        let reader = std::thread::spawn(|| b"hello".to_vec());
        assert_eq!(join_reader(reader, "stdout").unwrap(), b"hello");
    }

    #[test]
    fn timeout_captures_stderr() {
        let child = spawn_sh(">&2 echo boom; exit 1");
        let out = wait_with_timeout(child, 5, "test").expect("not timeout");
        assert!(!out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stderr).trim(), "boom");
    }
}
