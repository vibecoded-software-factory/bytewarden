//! Per-terminal `BW_SESSION` persistence — keeps the unlocked session
//! alive across launches **as long as the parent shell is still alive**.
//!
//! Approach: when the user enables "Keep session while terminal is
//! open" on the login screen, we write the freshly-issued `bw` session
//! key to a private file named after the parent shell's PID. On the
//! next launch we look that file up *before* `BwCliAdapter::new()` runs
//! and export `$BW_SESSION` from it — but only after verifying the
//! parent PID is still alive, so the moment the terminal closes the
//! key becomes effectively unreadable.
//!
//! Why not just print `export BW_SESSION=…` for the user to `eval`?
//! Because that is opt-in noise on every exit; this approach is
//! invisible to the shell and reverses cleanly when the terminal dies.
//!
//! ## Storage
//!
//! - Preferred: `${XDG_RUNTIME_DIR}/bytewarden/session-{PPID}` —
//!   `XDG_RUNTIME_DIR` is per-user, `0700`, and wiped at logout, so we
//!   inherit those guarantees for free.
//! - Fallback: `/tmp/bytewarden-{$USER}/session-{PPID}` — last resort
//!   when `XDG_RUNTIME_DIR` is unset (rare; some minimal containers).
//!
//! Files are written `0600`. Stale files (PPIDs that no longer point at
//! a live process) are cleaned up at startup so `/tmp` doesn't drift.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::parent_id;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use zeroize::Zeroizing;

/// Maximum age of a session file before it is considered stale and
/// deleted, regardless of whether its PPID still maps to a live
/// process.
///
/// This is a defence-in-depth check on top of `pid_alive`: if the OS
/// recycles the parent shell's PID quickly enough, a freshly-spawned
/// process could inherit the dead shell's PID and `kill -0` would
/// (correctly) say it is alive — but the saved session key belongs to
/// the *previous* shell. Capping the file age means a recycled PID
/// can keep an orphan file readable for at most this long.
///
/// 24 hours strikes a balance between "still skip the unlock prompt
/// for a workday" and "don't trust a key for an entire weekend."
const SESSION_MAX_AGE_SECS: u64 = 24 * 60 * 60;

/// Pure version of [`dir`] — explicit env inputs make this easy to
/// unit-test without touching the global environment.
fn dir_inner(xdg_runtime: Option<&str>, user: Option<&str>) -> PathBuf {
    if let Some(d) = xdg_runtime
        && !d.is_empty()
    {
        return PathBuf::from(d).join("bytewarden");
    }
    let user = user.unwrap_or("default");
    PathBuf::from("/tmp").join(format!("bytewarden-{user}"))
}

/// Returns the directory where per-PID session files live. Created
/// lazily by [`save`].
fn dir() -> PathBuf {
    let xdg = std::env::var("XDG_RUNTIME_DIR").ok();
    let user = std::env::var("USER").ok();
    dir_inner(xdg.as_deref(), user.as_deref())
}

/// Path of the session file for the *current* parent process.
fn current_path() -> PathBuf {
    dir().join(format!("session-{}", parent_id()))
}

/// Writes `session_key` to disk with `0600` perms. Errors are
/// swallowed: failing to persist the key is not fatal — the user just
/// has to log in again next launch.
pub fn save(session_key: &str) {
    if session_key.is_empty() {
        return;
    }
    let d = dir();
    if fs::create_dir_all(&d).is_err() {
        return;
    }
    // Tighten the directory perms to 0700 in case the runtime path
    // (e.g. `/tmp/bytewarden-$USER`) was just created.
    let _ = fs::set_permissions(&d, fs::Permissions::from_mode(0o700));

    let path = current_path();
    if fs::write(&path, session_key).is_err() {
        return;
    }
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
}

/// Returns the saved session key for the current parent process if the
/// parent is still alive, otherwise `None`. Stale files are removed.
///
/// The returned `String` is wrapped in [`Zeroizing`] so the in-memory
/// copy of the session key gets overwritten with zeroes when the value
/// is dropped — the caller exports it to `BW_SESSION` and lets the
/// wrapper drop, leaving no plaintext copy in our heap.
pub fn load() -> Option<Zeroizing<String>> {
    let path = current_path();
    // A file older than the cap is dropped before we even read it —
    // closes the PID-reuse window where a recycled PPID could still
    // pass `pid_alive` for a stale orphan key.
    if is_too_old(&path) {
        let _ = fs::remove_file(&path);
        return None;
    }
    // Read into a Zeroizing buffer from the start: even the
    // pre-trimmed string would otherwise live unzeroed in our heap
    // until the allocator reclaims its memory.
    let content = Zeroizing::new(fs::read_to_string(&path).ok()?);
    let trimmed = content.trim();
    if trimmed.is_empty() {
        let _ = fs::remove_file(&path);
        return None;
    }
    if !pid_alive(parent_id()) {
        let _ = fs::remove_file(&path);
        return None;
    }
    Some(Zeroizing::new(trimmed.to_string()))
}

/// Removes the session file for the current parent process. Idempotent.
pub fn clear() {
    let _ = fs::remove_file(current_path());
}

/// Walks [`dir()`] and removes every `session-*` file that is either
///
/// * older than [`SESSION_MAX_AGE_SECS`], or
/// * named after a PID that no longer points at a live process.
///
/// Cheap to call at startup. The age cap is the defence against
/// PID-reuse: even if the OS hands the dead parent's PID to a fresh
/// shell, the orphan file is evicted as soon as it crosses the age
/// threshold.
pub fn cleanup_orphans() {
    let Ok(entries) = fs::read_dir(dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(s) = name.to_str() else {
            continue;
        };
        let Some(pid_str) = s.strip_prefix("session-") else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        let path = entry.path();
        if !pid_alive(pid) || is_too_old(&path) {
            let _ = fs::remove_file(&path);
        }
    }
}

/// Returns `true` when the file at `path` is older than
/// [`SESSION_MAX_AGE_SECS`] (or its mtime cannot be read at all — we
/// err on the side of "treat as stale" so suspicious files are
/// removed rather than trusted).
///
/// A future-dated mtime (clock skew) is treated as fresh.
fn is_too_old(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return true;
    };
    let Ok(mtime) = meta.modified() else {
        return true;
    };
    match SystemTime::now().duration_since(mtime) {
        Ok(age) => age >= Duration::from_secs(SESSION_MAX_AGE_SECS),
        Err(_) => false,
    }
}

/// Returns `true` when a process with `pid` exists and the current
/// user can signal it. Implemented via `kill -0` to avoid pulling in
/// `libc` as a dependency.
fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// Serialises tests that mutate `XDG_RUNTIME_DIR` / `USER` so they
    /// don't race when `cargo test` runs them in parallel threads.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn dir_inner_prefers_xdg_runtime_dir() {
        let p = dir_inner(Some("/run/user/1000"), Some("alice"));
        assert_eq!(p, PathBuf::from("/run/user/1000/bytewarden"));
    }

    #[test]
    fn dir_inner_falls_back_to_tmp_with_user() {
        let p = dir_inner(None, Some("alice"));
        assert_eq!(p, PathBuf::from("/tmp/bytewarden-alice"));
    }

    #[test]
    fn dir_inner_empty_xdg_falls_back() {
        let p = dir_inner(Some(""), Some("alice"));
        assert_eq!(p, PathBuf::from("/tmp/bytewarden-alice"));
    }

    #[test]
    fn dir_inner_handles_missing_user() {
        let p = dir_inner(None, None);
        assert_eq!(p, PathBuf::from("/tmp/bytewarden-default"));
    }

    #[test]
    fn pid_alive_self_is_true() {
        assert!(pid_alive(std::process::id()));
    }

    #[test]
    fn pid_alive_zero_is_false() {
        assert!(!pid_alive(0));
    }

    #[test]
    fn pid_alive_unlikely_high_pid_is_false() {
        // 2^31 - 1 is way past any realistic PID on Linux/macOS.
        assert!(!pid_alive(2_147_483_646));
    }

    /// Compile-time guard that `load` returns the zeroizing wrapper.
    /// If a future refactor ever swaps the return type to a plain
    /// `String` this fails to compile, signalling that the in-memory
    /// secret is no longer being scrubbed on drop.
    #[test]
    fn load_return_type_is_zeroizing() {
        let _: fn() -> Option<Zeroizing<String>> = load;
    }

    /// End-to-end save → load → clear cycle. Uses the global
    /// `XDG_RUNTIME_DIR` lock to keep concurrent test threads honest.
    #[test]
    fn save_load_clear_round_trip_via_xdg() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        // SAFETY: env mutation is guarded by the module-local lock and
        // only exercised inside this module (no other code reads
        // `XDG_RUNTIME_DIR` at runtime in tests).
        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", tmp.path());
        }

        // Clean slate in case a previous run left a file behind.
        clear();

        save("KEY-12345");
        let path = tmp
            .path()
            .join("bytewarden")
            .join(format!("session-{}", std::os::unix::process::parent_id()));
        assert!(path.exists(), "session file should have been written");

        let loaded = load();
        assert_eq!(loaded.as_ref().map(|z| z.as_str()), Some("KEY-12345"));

        clear();
        assert!(!path.exists(), "clear should remove the session file");
    }

    #[test]
    fn save_skips_empty_keys() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", tmp.path());
        }
        clear();
        save("");
        let path = tmp
            .path()
            .join("bytewarden")
            .join(format!("session-{}", std::os::unix::process::parent_id()));
        assert!(!path.exists(), "empty key must not create a file");
    }

    /// Sets the mtime of `path` to the given absolute `SystemTime`.
    /// Wraps `File::set_modified` so each test reads cleanly.
    fn set_mtime(path: &std::path::Path, when: SystemTime) {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .expect("open file to set mtime");
        f.set_modified(when).expect("set mtime");
    }

    #[test]
    fn is_too_old_classifies_by_age() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("probe");
        std::fs::write(&path, "x").unwrap();

        // Fresh — must be considered young.
        assert!(!is_too_old(&path));

        // Backdate to one second past the cap — must flip to "too old".
        let stale = SystemTime::now() - Duration::from_secs(SESSION_MAX_AGE_SECS + 1);
        set_mtime(&path, stale);
        assert!(is_too_old(&path));
    }

    #[test]
    fn is_too_old_returns_true_for_missing_file() {
        // No file at this path → can't read mtime → treat as stale.
        assert!(is_too_old(std::path::Path::new(
            "/nonexistent/bytewarden-test"
        )));
    }

    #[test]
    fn load_drops_files_older_than_the_cap() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", tmp.path());
        }
        clear();
        save("STALE-KEY");
        let path = tmp
            .path()
            .join("bytewarden")
            .join(format!("session-{}", std::os::unix::process::parent_id()));
        // Backdate past the cap so the file is treated as stale even
        // though our PPID is still alive.
        let stale = SystemTime::now() - Duration::from_secs(SESSION_MAX_AGE_SECS + 60);
        set_mtime(&path, stale);

        assert!(load().is_none(), "stale file must not be loaded");
        assert!(!path.exists(), "stale file must be removed by load");
    }

    #[test]
    fn cleanup_orphans_removes_files_older_than_the_cap_even_with_live_pid() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", tmp.path());
        }

        let dir = tmp.path().join("bytewarden");
        std::fs::create_dir_all(&dir).unwrap();
        // PID is the running test process — `pid_alive` returns true.
        // Without the age cap this file would survive cleanup; with
        // it, the stale mtime gets it removed.
        let path = dir.join(format!("session-{}", std::process::id()));
        std::fs::write(&path, "x").unwrap();
        let stale = SystemTime::now() - Duration::from_secs(SESSION_MAX_AGE_SECS + 60);
        set_mtime(&path, stale);

        cleanup_orphans();
        assert!(
            !path.exists(),
            "stale file must be removed despite live PID"
        );
    }

    #[test]
    fn cleanup_orphans_removes_dead_pids_and_keeps_live_ones() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", tmp.path());
        }

        let dir = tmp.path().join("bytewarden");
        std::fs::create_dir_all(&dir).unwrap();
        // Live: my own PID — pid_alive returns true → keep.
        let live = dir.join(format!("session-{}", std::process::id()));
        std::fs::write(&live, "x").unwrap();
        // Dead: an absurdly high PID nobody is using → removed.
        let dead = dir.join("session-2147483646");
        std::fs::write(&dead, "x").unwrap();
        // Garbage filename: not a session-* prefix → ignored, kept.
        let other = dir.join("not-a-session");
        std::fs::write(&other, "x").unwrap();

        cleanup_orphans();

        assert!(live.exists(), "live PID file must survive");
        assert!(!dead.exists(), "dead PID file must be removed");
        assert!(other.exists(), "non-matching name must be ignored");
    }
}
