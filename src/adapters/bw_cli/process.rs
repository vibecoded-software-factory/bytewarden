//! Thin wrappers around `std::process::Command` for `bw` invocations.

use crate::ports::BwError;
use std::io::{Read, Write};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Name of the environment variable used to feed master / unlock
/// passwords to `bw` via `--passwordenv`. The variable lives only in the
/// child process so the secret never appears on a command line and is
/// invisible to `ps` / process-accounting tools.
pub const BW_PASSWORD_ENV: &str = "BW_PASS_INPUT";

/// Standard env var name `bw` reads the unlocked-vault session key
/// from. We feed every session-bearing call through this var instead
/// of the equivalent `--session <key>` flag, so the key never lands
/// in `argv` and stays out of `ps aux` / `/proc/PID/cmdline`. Same
/// rationale as [`BW_PASSWORD_ENV`].
pub const BW_SESSION_ENV: &str = "BW_SESSION";

/// Global `bw` flag automatically prepended to every invocation.
///
/// `--nointeraction` makes `bw` fail fast instead of waiting on stdin
/// when a required argument is missing. Without it, a future change to
/// the CLI's prompt logic could deadlock the TUI on a hidden read.
const NOINTERACTION: &str = "--nointeraction";

/// Defense-in-depth timeout for *local-only* `bw` invocations
/// (unlock, list cached items, get TOTP from local store, list
/// folders/orgs/collections, fingerprint, lock). These should
/// finish in milliseconds, but a wedged bw process — or a
/// future bug that adds an unexpected hidden prompt — would
/// otherwise freeze the TUI indefinitely. 10 s is many orders of
/// magnitude above the expected runtime, so it never fires in
/// practice; it's purely a panic-prevention floor.
const LOCAL_OP_FALLBACK_TIMEOUT: u64 = 10;

/// Builds the full argv passed to the child process — always prefixed
/// with `--nointeraction` for defense-in-depth.
fn full_args<'a>(args: &'a [&'a str]) -> Vec<&'a str> {
    let mut v = Vec::with_capacity(args.len() + 1);
    v.push(NOINTERACTION);
    v.extend_from_slice(args);
    v
}

/// Polls a spawned child until it exits or the wall-clock deadline is
/// reached. On timeout the child is killed and an error is returned.
///
/// Extracted so every `bw_run_*_timeout` variant shares the same
/// polling code path — keeping the timeout semantics in lock-step.
///
/// ## Why two reader threads
///
/// stdout and stderr are drained concurrently in dedicated threads
/// **while** the child is still running, not after it exits. The
/// post-exit drain pattern (read_to_end after try_wait returns Some)
/// deadlocks on any output larger than the pipe buffer (typically
/// 64 KB on Linux): the child blocks on `write(2)` waiting for the
/// pipe to be drained, we block in `try_wait` waiting for the child
/// to exit, neither side makes progress and the only escape is the
/// timeout. `bw list items` on a vault with a few thousand entries
/// easily exceeds that threshold, which is exactly what was hitting
/// the 3-minute timeout in the field.
///
/// `std::process::Child::wait_with_output` does this internally; we
/// reimplement the same pattern here because we also need a wall-clock
/// deadline, which `wait_with_output` does not expose.
fn wait_with_timeout(mut child: Child, secs: u64, label: &str) -> Result<Output, BwError> {
    let deadline = Instant::now() + Duration::from_secs(secs);

    // Take ownership of the pipe handles up front so the reader
    // threads can be spawned before we start polling. Any pipe the
    // caller didn't request (e.g. stderr not piped) is simply absent
    // and the corresponding thread is not spawned.
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
                    .and_then(|t| t.join().ok())
                    .unwrap_or_default();
                let stderr = stderr_thread
                    .and_then(|t| t.join().ok())
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
                    // Reader threads return as soon as the pipe closes
                    // (kill drops the write end), so joining here is
                    // bounded — no risk of compounding the user's wait.
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

/// Runs `bw <args>` and returns the raw [`Output`].
///
/// Spawn errors are mapped to a `String` so callers can use `?` against
/// the same `Result<_, BwError>` shape used throughout the adapter.
///
/// `--nointeraction` is prepended automatically; callers do not need
/// (and should not) pass it themselves.
///
/// Used for local-only operations (no network round-trip) — but a
/// defensive [`LOCAL_OP_FALLBACK_TIMEOUT`] still applies so a hung
/// `bw` process can never freeze the TUI permanently. The timeout
/// is two orders of magnitude above the expected runtime so it
/// only ever fires if something is genuinely broken.
pub fn bw_run(args: &[&str]) -> Result<Output, BwError> {
    bw_run_timeout(args, LOCAL_OP_FALLBACK_TIMEOUT)
}

/// Runs `bw <args>` with a wall-clock timeout.
///
/// Used for every operation that might touch the network (login, sync,
/// item CRUD, HIBP check, attachment up/download, …). Local-only ops
/// (unlock, list cached items, get TOTP from the local store) keep
/// using [`bw_run`] without a deadline — they are fast and uninterruptible.
pub fn bw_run_timeout(args: &[&str], secs: u64) -> Result<Output, BwError> {
    let child = Command::new("bw")
        .args(full_args(args))
        // Null out stdin explicitly. With `--nointeraction` bw should
        // never prompt, but the parent process is a TUI in raw mode —
        // an inherited terminal fd would let any stray bw read steal
        // the user's keystrokes (or block waiting for them). Closing
        // it at spawn time is the cheapest possible defense.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BwError::Spawn(format!("Could not run bw: {e}")))?;
    wait_with_timeout(child, secs, "bw")
}

/// Runs `bw <args>` after exporting `password` to the child's environment
/// under [`BW_PASSWORD_ENV`].
///
/// Callers are expected to include `--passwordenv BW_PASS_INPUT` in `args`
/// so `bw` reads the secret from the env var instead of `argv`.
/// `--nointeraction` is prepended automatically.
///
/// Carries the same defensive [`LOCAL_OP_FALLBACK_TIMEOUT`] as
/// [`bw_run`] — used by `unlock`, which is local-only crypto and
/// should never need more than a fraction of a second, but a wedged
/// child must not freeze the TUI.
///
/// # Why this exists
///
/// Passing a password as a positional argument leaks it into `ps aux`
/// for the lifetime of the child process. The env-var path keeps the
/// secret out of the process command line entirely.
pub fn bw_run_with_password(args: &[&str], password: &str) -> Result<Output, BwError> {
    bw_run_with_password_timeout(args, password, LOCAL_OP_FALLBACK_TIMEOUT)
}

/// Like [`bw_run_with_password`] but with a wall-clock timeout. Used by
/// auth paths (login / unlock variants) that hit the Bitwarden server.
pub fn bw_run_with_password_timeout(
    args: &[&str],
    password: &str,
    secs: u64,
) -> Result<Output, BwError> {
    let child = Command::new("bw")
        .args(full_args(args))
        .env(BW_PASSWORD_ENV, password)
        // See `bw_run_timeout` for the stdin-null rationale.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BwError::Spawn(format!("Could not run bw: {e}")))?;
    wait_with_timeout(child, secs, "bw")
}

/// Runs `bw <args>` after exporting `session` to the child's
/// environment under [`BW_SESSION_ENV`].
///
/// **Do not** add `--session <key>` to `args`: that's the whole point
/// of this helper — keeping the unlocked-vault key out of `ps aux`
/// for the duration of every vault operation. The env-var path
/// mirrors what [`bw_run_with_password`] does for the master
/// password.
///
/// Used for local-only session-bearing reads (`bw list items`,
/// `bw list folders`, `bw get totp`, …) — same
/// [`LOCAL_OP_FALLBACK_TIMEOUT`] as [`bw_run`].
pub fn bw_run_with_session(args: &[&str], session: &str) -> Result<Output, BwError> {
    bw_run_with_session_timeout(args, session, LOCAL_OP_FALLBACK_TIMEOUT)
}

/// Like [`bw_run_with_session`] but with a wall-clock timeout. Used
/// by every session-bearing call that has a known upper bound on
/// runtime (item CRUD, sync, attachments, exports, …).
pub fn bw_run_with_session_timeout(
    args: &[&str],
    session: &str,
    secs: u64,
) -> Result<Output, BwError> {
    let child = Command::new("bw")
        .args(full_args(args))
        .env(BW_SESSION_ENV, session)
        // See `bw_run_timeout` for the stdin-null rationale.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BwError::Spawn(format!("Could not run bw: {e}")))?;
    wait_with_timeout(child, secs, "bw")
}

/// Runs `bw <args>` after exporting `password` to the child env under
/// [`BW_PASSWORD_ENV`] **and** writing `stdin_input` to the child's
/// stdin.
///
/// Used by the OTP path: `bw` does not expose a `--codeenv`-style flag
/// for the device-verification code, so the only way to keep it out of
/// `ps` is to drop `--nointeraction` for this single call and let
/// `bw`'s interactive prompt read the code off stdin.
///
/// `--nointeraction` is **not** prepended automatically — the caller is
/// expected to know what they are doing and to provide enough stdin to
/// answer every prompt the call may raise.
pub fn bw_run_with_password_and_stdin(
    args: &[&str],
    password: &str,
    stdin_input: &str,
) -> Result<Output, BwError> {
    let mut child = Command::new("bw")
        .args(args)
        .env(BW_PASSWORD_ENV, password)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BwError::Spawn(format!("Could not run bw: {e}")))?;

    if let Some(mut sin) = child.stdin.take() {
        // Errors writing to stdin (broken pipe if bw exits early) are
        // intentionally ignored — the wait below will surface the real
        // failure via the child's exit status / stderr.
        let _ = sin.write_all(stdin_input.as_bytes());
        // Dropping `sin` here closes the write end of the pipe, which
        // signals EOF to bw so it stops waiting for more input.
    }

    child
        .wait_with_output()
        .map_err(|e| BwError::Internal(format!("bw wait error: {e}")))
}

/// Like [`bw_run_with_password_and_stdin`] but with a wall-clock
/// timeout. Used by `login_with_otp` so a stalled MFA roundtrip can
/// be unstuck with `Esc` instead of freezing the TUI.
pub fn bw_run_with_password_and_stdin_timeout(
    args: &[&str],
    password: &str,
    stdin_input: &str,
    secs: u64,
) -> Result<Output, BwError> {
    let mut child = Command::new("bw")
        .args(args)
        .env(BW_PASSWORD_ENV, password)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BwError::Spawn(format!("Could not run bw: {e}")))?;

    if let Some(mut sin) = child.stdin.take() {
        let _ = sin.write_all(stdin_input.as_bytes());
    }
    wait_with_timeout(child, secs, "bw")
}

/// Returns the trimmed stdout of an [`Output`] as a [`String`].
pub fn stdout_str(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Returns the trimmed stderr of an [`Output`] as a [`String`].
pub fn stderr_str(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).trim().to_string()
}

// ── Interactive login: a child kept alive across a user interaction ──────
//
// Every other `bw` call in this adapter is one-shot: spawn, feed
// everything up front, collect the output, done. Device verification
// cannot work that way.
//
// When the backend doesn't recognise the device, it e-mails a code
// **as part of the login request itself** and `bw` then prompts for it.
// Each fresh `bw login` therefore triggers a *new* e-mail and
// invalidates the previous code, so submitting a code that was obtained
// before the process started can never succeed — the code the user is
// holding always belongs to the previous attempt. `bw` also offers no
// non-interactive flag for it (`--code` is documented, and implemented,
// as the *two-step* login code only; the new-device token is read
// exclusively from the prompt).
//
// The only workable shape is to keep **one** `bw login` alive across the
// user's code entry: spawn it, wait until it prints its prompt, hand
// control back to the UI, and later write the code to that same child's
// stdin. `inquirer` (the prompt library `bw` uses) reads whatever
// arrives on stdin whenever it arrives, so the pause costs nothing.

/// Outcome of waiting for a parked child to reach its prompt.
pub enum PromptWait {
    /// One of the caller's markers appeared on stderr — the child is
    /// parked at its prompt, waiting for a line on stdin.
    Reached,
    /// The child exited before prompting (success, or a plain failure).
    Exited(Box<Output>),
    /// Neither happened inside the budget; the caller should give up
    /// and kill the child.
    TimedOut,
}

/// A `bw` child held open across a user interaction.
///
/// Both pipes are drained by reader threads from the moment it spawns —
/// same rationale as [`wait_with_timeout`], and doubly so here: the
/// child lives for as long as a human takes to read an e-mail, which is
/// ample time to fill a pipe buffer and deadlock.
///
/// Dropping kills the child. A parked login is an authentication
/// attempt in progress; leaving one running after the UI has moved on
/// would silently hold a `bw` process (and its in-flight auth state)
/// for the lifetime of the app.
pub struct InteractiveChild {
    child: Child,
    stdout: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    stderr: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    readers: Vec<std::thread::JoinHandle<()>>,
}

/// Drains `r` into `buf` until EOF. Any read error ends the pump — the
/// child exiting is the normal way out.
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

/// Spawns `bw <args>` **without** `--nointeraction`, with the password
/// in the environment and all three pipes open, ready to be driven
/// interactively.
///
/// `--nointeraction` is deliberately omitted: it is exactly what
/// suppresses the prompt this whole mechanism waits for.
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
    /// Everything the child has written to stderr so far, lossily
    /// decoded. `bw` prompts on stderr, so this is where the markers
    /// show up.
    pub fn stderr_so_far(&self) -> String {
        self.stderr
            .lock()
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_default()
    }

    /// Polls until the child's stderr contains one of `markers`
    /// (case-insensitive), the child exits, or `secs` elapses.
    ///
    /// Markers are checked **before** the exit test on each pass so a
    /// prompt that lands in the same tick as an exit is not missed.
    pub fn wait_for_prompt(&mut self, markers: &[&str], secs: u64) -> Result<PromptWait, BwError> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            let seen = self.stderr_so_far().to_lowercase();
            if markers.iter().any(|m| seen.contains(m)) {
                return Ok(PromptWait::Reached);
            }
            match self.child.try_wait() {
                Err(e) => return Err(BwError::Internal(format!("bw wait error: {e}"))),
                Ok(Some(status)) => return Ok(PromptWait::Exited(Box::new(self.collect(status)))),
                Ok(None) => {}
            }
            if Instant::now() >= deadline {
                return Ok(PromptWait::TimedOut);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Writes one line to the child's stdin and closes it, which is what
    /// releases `inquirer`'s read. Closing also guarantees the child can
    /// never block waiting for a second line we are not going to send.
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

    /// Waits for the child to exit within `secs` and returns its output,
    /// killing it on timeout.
    pub fn finish(mut self, secs: u64, label: &str) -> Result<Output, BwError> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            match self.child.try_wait() {
                Err(e) => return Err(BwError::Internal(format!("bw wait error: {e}"))),
                Ok(Some(status)) => return Ok(self.collect(status)),
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

    /// Joins the reader threads (bounded — the child has exited, so both
    /// pipes are at EOF) and packages the drained buffers as an
    /// [`Output`].
    fn collect(&mut self, status: std::process::ExitStatus) -> Output {
        for r in self.readers.drain(..) {
            let _ = r.join();
        }
        let take = |b: &std::sync::Arc<std::sync::Mutex<Vec<u8>>>| {
            b.lock()
                .map(|mut v| std::mem::take(&mut *v))
                .unwrap_or_default()
        };
        Output {
            status,
            stdout: take(&self.stdout),
            stderr: take(&self.stderr),
        }
    }
}

impl Drop for InteractiveChild {
    fn drop(&mut self) {
        // Best-effort: if it already exited, both calls are no-ops.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spawns `sh -c <script>` with stdout/stderr piped and runs the
    /// shared timeout poller against it. Lets us exercise the polling
    /// path without depending on `bw` being installed.
    /// Builds an [`InteractiveChild`] around `sh -c <script>` instead of
    /// `bw`, so the parking mechanics can be tested without the CLI (or
    /// a Bitwarden account). Mirrors `spawn_interactive` exactly apart
    /// from the program name.
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

    /// The core of the device-verification fix: a child that prints a
    /// prompt and then blocks on stdin must be reported as *parked*, not
    /// as exited and not as a timeout — that is what lets the UI collect
    /// the code while this exact process stays alive.
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

    /// A child that exits before prompting must surface its output, so
    /// the caller can classify a plain credential failure.
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

    /// Neither prompting nor exiting inside the budget must be reported
    /// as a timeout rather than hanging the caller forever.
    #[test]
    fn interactive_child_times_out_when_nothing_happens() {
        let mut c = interactive_sh("exec sleep 5");
        assert!(matches!(
            c.wait_for_prompt(&["never appears"], 1),
            Ok(PromptWait::TimedOut)
        ));
    }

    /// Dropping a parked child must kill it — an abandoned login is an
    /// authentication attempt in flight, not something to leave running.
    #[test]
    fn dropping_a_parked_child_kills_the_process() {
        let c = interactive_sh("exec sleep 30");
        let pid = c.child.id();
        drop(c);
        // `kill -0` fails once the process is gone (reaped by `wait` in
        // the Drop impl).
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
        // 1-second budget against a 5-second sleep — must fire.
        // `exec sleep` replaces the shell with sleep so the child PID
        // and the sleep PID are the same; killing the child closes the
        // pipe immediately. Without `exec`, sh forks/execs sleep and
        // a SIGKILL on sh leaves sleep holding the inherited pipe fd
        // open until it naturally exits, which would block our reader
        // threads for the full 5 s and make the test useless.
        let child = spawn_sh("exec sleep 5");
        let started = Instant::now();
        let res = wait_with_timeout(child, 1, "test");
        let elapsed = started.elapsed();
        assert!(res.is_err());
        let msg = res.unwrap_err().to_string();
        assert!(msg.contains("test"));
        assert!(msg.contains("timed out"));
        // The polling loop sleeps 50 ms between checks, so the actual
        // kill happens within a couple of ticks of the deadline.
        assert!(elapsed < Duration::from_secs(3), "took {elapsed:?}");
    }

    /// Regression test for the pipe-buffer deadlock: a child that
    /// emits more than 64 KB to stdout and then exits must complete
    /// well within the timeout. The pre-fix `wait_with_timeout`
    /// drained stdout only after `try_wait` returned `Some`, which
    /// deadlocked any output above the pipe capacity (~64 KB on
    /// Linux). The reader-thread version handles this correctly.
    #[test]
    fn timeout_drains_large_stdout_without_deadlocking() {
        // 256 KB of output — comfortably above the 64 KB pipe buffer
        // so the bug, if reintroduced, manifests as a hard timeout
        // rather than a flaky pass.
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

    #[test]
    fn timeout_captures_stderr() {
        let child = spawn_sh(">&2 echo boom; exit 1");
        let out = wait_with_timeout(child, 5, "test").expect("not timeout");
        assert!(!out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stderr).trim(), "boom");
    }
}
