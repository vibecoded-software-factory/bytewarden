use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::parent_id;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use zeroize::Zeroizing;

const SESSION_MAX_AGE_SECS: u64 = 24 * 60 * 60;

fn dir_inner(xdg_runtime: Option<&str>, user: Option<&str>) -> PathBuf {
    if let Some(d) = xdg_runtime
        && !d.is_empty()
    {
        return PathBuf::from(d).join("bytewarden");
    }
    let user = user.unwrap_or("default");
    PathBuf::from("/tmp").join(format!("bytewarden-{user}"))
}

fn dir() -> PathBuf {
    let xdg = std::env::var("XDG_RUNTIME_DIR").ok();
    let user = std::env::var("USER").ok();
    dir_inner(xdg.as_deref(), user.as_deref())
}

fn current_path() -> PathBuf {
    dir().join(format!("session-{}", parent_id()))
}

pub fn save(session_key: &str) {
    if session_key.is_empty() {
        return;
    }
    let d = dir();
    if fs::create_dir_all(&d).is_err() {
        return;
    }

    let _ = fs::set_permissions(&d, fs::Permissions::from_mode(0o700));

    let path = current_path();
    if fs::write(&path, session_key).is_err() {
        return;
    }
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
}

pub fn load() -> Option<Zeroizing<String>> {
    let path = current_path();

    if is_too_old(&path) {
        let _ = fs::remove_file(&path);
        return None;
    }

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

pub fn clear() {
    let _ = fs::remove_file(current_path());
}

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
        assert!(!pid_alive(2_147_483_646));
    }

    #[test]
    fn load_return_type_is_zeroizing() {
        let _: fn() -> Option<Zeroizing<String>> = load;
    }

    #[test]
    fn save_load_clear_round_trip_via_xdg() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();

        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", tmp.path());
        }

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

        assert!(!is_too_old(&path));

        let stale = SystemTime::now() - Duration::from_secs(SESSION_MAX_AGE_SECS + 1);
        set_mtime(&path, stale);
        assert!(is_too_old(&path));
    }

    #[test]
    fn is_too_old_returns_true_for_missing_file() {
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

        let live = dir.join(format!("session-{}", std::process::id()));
        std::fs::write(&live, "x").unwrap();

        let dead = dir.join("session-2147483646");
        std::fs::write(&dead, "x").unwrap();

        let other = dir.join("not-a-session");
        std::fs::write(&other, "x").unwrap();

        cleanup_orphans();

        assert!(live.exists(), "live PID file must survive");
        assert!(!dead.exists(), "dead PID file must be removed");
        assert!(other.exists(), "non-matching name must be ignored");
    }
}
