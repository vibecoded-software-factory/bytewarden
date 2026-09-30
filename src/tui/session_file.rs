use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
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
    if fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&d)
        .is_err()
    {
        return;
    }
    if !trusted_dir(&d) {
        return;
    }

    write_atomic(&current_path(), session_key);
}

fn trusted_dir(d: &Path) -> bool {
    let owner_only = |d: &Path| {
        fs::symlink_metadata(d).is_ok_and(|m| m.file_type().is_dir() && m.mode() & 0o077 == 0)
    };
    if owner_only(d) {
        return true;
    }
    if fs::symlink_metadata(d).is_ok_and(|m| m.file_type().is_dir()) {
        let _ = fs::set_permissions(d, fs::Permissions::from_mode(0o700));
    }
    owner_only(d)
}

fn write_atomic(path: &Path, contents: &str) {
    let tmp = {
        let mut p = path.as_os_str().to_owned();
        p.push(".tmp");
        PathBuf::from(p)
    };
    let _ = fs::remove_file(&tmp);
    let write = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if write.is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

pub fn load() -> Option<Zeroizing<String>> {
    if !trusted_dir(&dir()) {
        return None;
    }
    let path = current_path();

    if is_too_old(&path) {
        let _ = fs::remove_file(&path);
        return None;
    }
    let meta = fs::symlink_metadata(&path).ok()?;
    if !meta.file_type().is_file() || meta.mode() & 0o077 != 0 {
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
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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

    fn use_xdg(tmp: &TempDir) -> PathBuf {
        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", tmp.path());
        }
        tmp.path().join("bytewarden")
    }

    fn session_path_in(dir: &Path) -> PathBuf {
        dir.join(format!("session-{}", std::os::unix::process::parent_id()))
    }

    #[test]
    fn save_writes_owner_only_file_and_dir() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        let dir = use_xdg(&tmp);
        save("KEY-MODE");
        let path = session_path_in(&dir);
        let file_mode = fs::metadata(&path).unwrap().mode() & 0o777;
        let dir_mode = fs::metadata(&dir).unwrap().mode() & 0o777;
        assert_eq!(file_mode, 0o600);
        assert_eq!(dir_mode, 0o700);
        let tmp_path = {
            let mut p = path.as_os_str().to_owned();
            p.push(".tmp");
            PathBuf::from(p)
        };
        assert!(!tmp_path.exists(), "temp file must not linger");
    }

    #[test]
    fn save_tightens_own_world_writable_dir() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        let dir = use_xdg(&tmp);
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o777)).unwrap();
        save("KEY-LOOSE");
        assert_eq!(fs::metadata(&dir).unwrap().mode() & 0o777, 0o700);
        assert_eq!(load().as_ref().map(|z| z.as_str()), Some("KEY-LOOSE"));
    }

    #[test]
    fn save_and_load_refuse_symlinked_dir() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        let dir = use_xdg(&tmp);
        let target = tmp.path().join("elsewhere");
        fs::create_dir(&target).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
        std::os::unix::fs::symlink(&target, &dir).unwrap();

        save("KEY-SYMLINK");
        assert!(
            !session_path_in(&target).exists(),
            "must not write through a symlinked dir"
        );

        fs::write(session_path_in(&target), "PLANTED").unwrap();
        fs::set_permissions(session_path_in(&target), fs::Permissions::from_mode(0o600)).unwrap();
        assert!(load().is_none(), "must not trust a symlinked dir");
    }

    #[test]
    fn save_replaces_planted_symlink_instead_of_following_it() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        let dir = use_xdg(&tmp);
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let victim = tmp.path().join("victim");
        fs::write(&victim, "ORIGINAL").unwrap();
        std::os::unix::fs::symlink(&victim, session_path_in(&dir)).unwrap();

        save("KEY-ATOMIC");

        assert_eq!(fs::read_to_string(&victim).unwrap(), "ORIGINAL");
        let meta = fs::symlink_metadata(session_path_in(&dir)).unwrap();
        assert!(meta.file_type().is_file());
        assert_eq!(meta.mode() & 0o777, 0o600);
    }

    #[test]
    fn load_refuses_symlinked_or_loose_session_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        let dir = use_xdg(&tmp);
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let path = session_path_in(&dir);

        let planted = tmp.path().join("planted");
        fs::write(&planted, "PLANTED").unwrap();
        fs::set_permissions(&planted, fs::Permissions::from_mode(0o600)).unwrap();
        std::os::unix::fs::symlink(&planted, &path).unwrap();
        assert!(load().is_none(), "symlinked session file must be refused");

        fs::remove_file(&path).unwrap();
        fs::write(&path, "LOOSE").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            load().is_none(),
            "group/world-readable file must be refused"
        );
    }
}
