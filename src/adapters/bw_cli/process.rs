//! Thin wrappers around `std::process::Command` for `bw` invocations.

use std::io::{Read, Write};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Name of the environment variable used to feed master / unlock
/// passwords to `bw` via `--passwordenv`. The variable lives only in the
/// child process so the secret never appears on a command line and is
/// invisible to `ps` / process-accounting tools.
pub const BW_PASSWORD_ENV: &str = "BW_PASS_INPUT";

/// Global `bw` flag automatically prepended to every invocation.
///
/// `--nointeraction` makes `bw` fail fast instead of waiting on stdin
/// when a required argument is missing. Without it, a future change to
/// the CLI's prompt logic could deadlock the TUI on a hidden read.
const NOINTERACTION: &str = "--nointeraction";

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
fn wait_with_timeout(mut child: Child, secs: u64, label: &str) -> Result<Output, String> {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        match child
            .try_wait()
            .map_err(|e| format!("bw wait error: {e}"))?
        {
            Some(status) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut o) = child.stdout.take() {
                    let _ = o.read_to_end(&mut stdout);
                }
                if let Some(mut e) = child.stderr.take() {
                    let _ = e.read_to_end(&mut stderr);
                }
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
                    return Err(format!("{label} timed out after {secs}s"));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// Runs `bw <args>` and returns the raw [`Output`].
///
/// Spawn errors are mapped to a `String` so callers can use `?` against
/// the same `Result<_, String>` shape used throughout the adapter.
///
/// `--nointeraction` is prepended automatically; callers do not need
/// (and should not) pass it themselves.
pub fn bw_run(args: &[&str]) -> Result<Output, String> {
    Command::new("bw")
        .args(full_args(args))
        .output()
        .map_err(|e| format!("Could not run bw: {e}"))
}

/// Runs `bw <args>` with a wall-clock timeout.
///
/// Used for every operation that might touch the network (login, sync,
/// item CRUD, HIBP check, attachment up/download, …). Local-only ops
/// (unlock, list cached items, get TOTP from the local store) keep
/// using [`bw_run`] without a deadline — they are fast and uninterruptible.
pub fn bw_run_timeout(args: &[&str], secs: u64) -> Result<Output, String> {
    let child = Command::new("bw")
        .args(full_args(args))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not run bw: {e}"))?;
    wait_with_timeout(child, secs, "bw")
}

/// Runs `bw <args>` after exporting `password` to the child's environment
/// under [`BW_PASSWORD_ENV`].
///
/// Callers are expected to include `--passwordenv BW_PASS_INPUT` in `args`
/// so `bw` reads the secret from the env var instead of `argv`.
/// `--nointeraction` is prepended automatically.
///
/// # Why this exists
///
/// Passing a password as a positional argument leaks it into `ps aux`
/// for the lifetime of the child process. The env-var path keeps the
/// secret out of the process command line entirely.
pub fn bw_run_with_password(args: &[&str], password: &str) -> Result<Output, String> {
    Command::new("bw")
        .args(full_args(args))
        .env(BW_PASSWORD_ENV, password)
        .output()
        .map_err(|e| format!("Could not run bw: {e}"))
}

/// Like [`bw_run_with_password`] but with a wall-clock timeout. Used by
/// auth paths (login / unlock variants) that hit the Bitwarden server.
pub fn bw_run_with_password_timeout(
    args: &[&str],
    password: &str,
    secs: u64,
) -> Result<Output, String> {
    let child = Command::new("bw")
        .args(full_args(args))
        .env(BW_PASSWORD_ENV, password)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not run bw: {e}"))?;
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
) -> Result<Output, String> {
    let mut child = Command::new("bw")
        .args(args)
        .env(BW_PASSWORD_ENV, password)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not run bw: {e}"))?;

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
        .map_err(|e| format!("bw wait error: {e}"))
}

/// Like [`bw_run_with_password_and_stdin`] but with a wall-clock
/// timeout. Used by `login_with_otp` so a stalled MFA roundtrip can
/// be unstuck with `Esc` instead of freezing the TUI.
pub fn bw_run_with_password_and_stdin_timeout(
    args: &[&str],
    password: &str,
    stdin_input: &str,
    secs: u64,
) -> Result<Output, String> {
    let mut child = Command::new("bw")
        .args(args)
        .env(BW_PASSWORD_ENV, password)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not run bw: {e}"))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Spawns `sh -c <script>` with stdout/stderr piped and runs the
    /// shared timeout poller against it. Lets us exercise the polling
    /// path without depending on `bw` being installed.
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
        let child = spawn_sh("sleep 5");
        let started = Instant::now();
        let res = wait_with_timeout(child, 1, "test");
        let elapsed = started.elapsed();
        assert!(res.is_err());
        let msg = res.unwrap_err();
        assert!(msg.contains("test"));
        assert!(msg.contains("timed out"));
        // The polling loop sleeps 50 ms between checks, so the actual
        // kill happens within a couple of ticks of the deadline.
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
