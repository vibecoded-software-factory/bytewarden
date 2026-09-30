use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const ENV_VAR: &str = "BYTEWARDEN_DEBUG";

pub fn log_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".bytewarden.log")
}

pub fn is_enabled() -> bool {
    std::env::var(ENV_VAR)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

pub fn enabled_path() -> Option<PathBuf> {
    is_enabled().then(log_path)
}

pub fn append(path: &Path, cmd: &str, ok: bool, detail: &str) -> std::io::Result<()> {
    let icon = if ok { "✓" } else { "✕" };
    let line = format!("{}  {icon}  {cmd:<60}  → {detail}\n", iso_utc_now());
    write_line(path, &line)
}

fn write_line(path: &Path, line: &str) -> std::io::Result<()> {
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)?;
    let mode = f.metadata()?.permissions().mode();
    if mode & 0o077 != 0 {
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    f.write_all(line.as_bytes())
}

fn iso_utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    crate::domain::timefmt::unix_to_iso_utc(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_format_has_expected_shape() {
        let s = iso_utc_now();
        assert_eq!(s.len(), 20);
        assert_eq!(s.chars().nth(4), Some('-'));
        assert_eq!(s.chars().nth(7), Some('-'));
        assert_eq!(s.chars().nth(10), Some('T'));
        assert_eq!(s.chars().nth(13), Some(':'));
        assert_eq!(s.chars().nth(16), Some(':'));
        assert!(s.ends_with('Z'));
    }

    #[test]
    fn is_enabled_respects_env_var() {
        let was_set = std::env::var(ENV_VAR).is_ok();
        if !was_set {
            assert!(!is_enabled());
        }
    }

    #[test]
    fn log_path_uses_home_when_set() {
        if let Ok(home) = std::env::var("HOME") {
            let p = log_path();
            assert!(p.starts_with(home));
            assert!(p.ends_with(".bytewarden.log"));
        }
    }

    #[test]
    fn append_to_a_directory_reports_the_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(append(dir.path(), "bw sync", true, "ok").is_err());
    }

    #[test]
    fn append_under_a_missing_parent_reports_the_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join("bytewarden.log");
        assert!(append(&path, "bw sync", true, "ok").is_err());
    }

    #[test]
    fn append_creates_the_log_owner_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bytewarden.log");
        append(&path, "bw sync", true, "ok").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(std::fs::read_to_string(&path).unwrap().contains("bw sync"));
    }

    #[test]
    fn append_tightens_an_existing_looser_log() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bytewarden.log");
        std::fs::write(&path, "").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        append(&path, "bw sync", true, "ok").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
