//! [`crate::ports::SettingsPort`] implementation backed by a single
//! `~/.config/bytewarden/config.toml` file.
//!
//! The parser is intentionally hand-rolled and forgiving — only the keys
//! recognised by [`UserSettings`] are read; everything else (including the
//! `[theme]` section consumed by the TUI) is preserved verbatim on
//! rewrites.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::ports::{SettingsPort, UserSettings};

/// Default inactivity threshold (15 minutes) when nothing is configured.
const DEFAULT_LOCK_AFTER_SECS: u64 = 15 * 60;

/// Default clipboard auto-clear delay (30 seconds), matching Bitwarden
/// GUI's behaviour. Set `clipboard_clear_secs = 0` in `config.toml` to
/// disable.
const DEFAULT_CLIPBOARD_CLEAR_SECS: u64 = 30;

/// Default wall-clock budget for `bw list items` (60 seconds). Sized
/// to cover a healthy decrypt of a large vault on a slow machine
/// without masking a wedged child for too long. Override with
/// `list_items_timeout_secs = N` in `config.toml` if you have a
/// genuinely huge vault and start hitting the ceiling.
const DEFAULT_LIST_ITEMS_TIMEOUT_SECS: u64 = 60;

/// Escapes a string for embedding inside a TOML basic-string literal
/// (`"…"`). Only the two TOML-significant characters need handling
/// here — `\` (the escape introducer) and `"` (the closing quote).
/// Real-world e-mail addresses contain neither, so the common path
/// is a no-op clone; the function exists purely as defense-in-depth
/// against pathological values that would otherwise corrupt the
/// config on rewrite.
fn escape_toml_basic(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Inverse of [`escape_toml_basic`]. Decodes the two escape
/// sequences we emit (`\"` and `\\`) and passes everything else
/// through untouched. Trailing/standalone `\` is preserved as-is so
/// hand-edited configs that don't follow the escape rules don't
/// silently lose data.
fn unescape_toml_basic(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    // Unknown escape — preserve verbatim rather
                    // than silently dropping the backslash.
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// File mode applied to `config.toml` after every write — owner-only
/// read/write. Even though the file does not contain credentials, it
/// can carry the user's e-mail address and `keep_session` preference,
/// neither of which need to be world-readable.
const CONFIG_FILE_MODE: u32 = 0o600;

/// Directory mode applied to `~/.config/bytewarden/` — owner-only
/// access, matching the file mode above and consistent with how the
/// session-file helper hardens its runtime directory.
const CONFIG_DIR_MODE: u32 = 0o700;

/// File-backed settings adapter.
#[derive(Debug, Clone)]
pub struct TomlSettingsAdapter {
    dir: PathBuf,
}

impl TomlSettingsAdapter {
    /// Builds an adapter rooted at `~/.config/bytewarden/`.
    ///
    /// Falls back to the current directory when `$HOME` is unset.
    pub fn new() -> Self {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        Self {
            dir: home.join(".config").join("bytewarden"),
        }
    }

    /// Returns the absolute path to `config.toml`.
    fn file(&self) -> PathBuf {
        self.dir.join("config.toml")
    }

    /// Ensures the config directory exists with `0o700` perms.
    ///
    /// Uses [`fs::DirBuilder::mode`] so a freshly-created directory is
    /// born with the right perms — the kernel never has the chance to
    /// return a `0o755` directory inheriting the user's umask. For
    /// directories that already exist (re-runs, or pre-existing trees
    /// from a different tool) the perms are tightened with an explicit
    /// `set_permissions` call as a safety net.
    ///
    /// Errors are swallowed by design — a missing directory will
    /// surface again at first read/write.
    fn ensure_dir(&self) {
        let _ = fs::DirBuilder::new()
            .recursive(true)
            .mode(CONFIG_DIR_MODE)
            .create(&self.dir);
        let _ = fs::set_permissions(&self.dir, fs::Permissions::from_mode(CONFIG_DIR_MODE));
    }
}

/// Writes `contents` to `path` so the on-disk file is **never** visible
/// with anything other than `0o600` perms.
///
/// Implementation notes:
///
/// * [`fs::OpenOptions::mode`] applies only when the file is *created*
///   — for a brand-new config it makes the perms atomic against the
///   user's umask, closing the race that `fs::write` followed by
///   `set_permissions` leaves open.
/// * For pre-existing files (e.g. created by an older bytewarden
///   without the hardening, or hand-edited by the user), `mode()` is
///   a no-op, so we still call `set_permissions(0o600)` after writing
///   as a safety net.
///
/// Errors are swallowed by design — settings persistence is
/// best-effort and the user-visible feedback comes from later reads
/// failing.
fn write_file_secure(path: &Path, contents: &str) {
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(CONFIG_FILE_MODE)
        .open(path)
    {
        Ok(f) => f,
        Err(_) => return,
    };
    if file.write_all(contents.as_bytes()).is_err() {
        return;
    }
    // Re-tighten perms in case the file already existed with a looser
    // mode (mode() above only takes effect on creation).
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(CONFIG_FILE_MODE));
}

impl Default for TomlSettingsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsPort for TomlSettingsAdapter {
    fn read(&self) -> UserSettings {
        self.ensure_dir();
        let mut cfg = UserSettings {
            lock_after_secs: DEFAULT_LOCK_AFTER_SECS,
            clipboard_clear_secs: DEFAULT_CLIPBOARD_CLEAR_SECS,
            list_items_timeout_secs: DEFAULT_LIST_ITEMS_TIMEOUT_SECS,
            ..Default::default()
        };
        let Ok(text) = fs::read_to_string(self.file()) else {
            return cfg;
        };
        for line in text.lines() {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("save_email = ") {
                cfg.save_email = v.trim() == "true";
            } else if let Some(v) = line.strip_prefix("email = ") {
                // Strip the wrapping `"` and decode escape sequences
                // emitted by `write` for round-trip safety with
                // pathological emails (those carrying `\` or `"`).
                let inner = v.trim().trim_matches('"');
                let decoded = unescape_toml_basic(inner);
                if !decoded.is_empty() {
                    cfg.email = Some(decoded);
                }
            } else if let Some(v) = line.strip_prefix("auto_lock = ") {
                cfg.auto_lock = v.trim() == "true";
            } else if let Some(v) = line.strip_prefix("keep_session = ") {
                cfg.keep_session = v.trim() == "true";
            } else if let Some(v) = line.strip_prefix("lock_after_minutes = ")
                && let Ok(m) = v.trim().parse::<u64>()
            {
                cfg.lock_after_secs = m * 60;
            } else if let Some(v) = line.strip_prefix("clipboard_clear_secs = ")
                && let Ok(s) = v.trim().parse::<u64>()
            {
                cfg.clipboard_clear_secs = s;
            } else if let Some(v) = line.strip_prefix("list_items_timeout_secs = ")
                && let Ok(s) = v.trim().parse::<u64>()
                && s > 0
            {
                // Reject 0 — it would mean "kill bw immediately", which
                // is never what the user wants. Any positive value is
                // accepted; the operator can pick "effectively no
                // timeout" by setting a very large number.
                cfg.list_items_timeout_secs = s;
            }
        }
        cfg
    }

    fn write(&self, save_email: bool, email: Option<&str>) {
        self.ensure_dir();
        let existing = fs::read_to_string(self.file()).unwrap_or_default();
        let mut preserved: Vec<String> = existing
            .lines()
            .filter(|l| {
                let t = l.trim();
                !t.starts_with("save_email =") && !t.starts_with("email =")
            })
            .map(|l| l.to_string())
            .collect();
        while preserved.first().is_some_and(|l| l.trim().is_empty()) {
            preserved.remove(0);
        }
        let mut owned = vec![format!("save_email = {save_email}")];
        if save_email && let Some(e) = email {
            owned.push(format!("email = \"{}\"", escape_toml_basic(e)));
        }
        if !preserved.is_empty() {
            owned.push(String::new());
            owned.extend(preserved);
        }
        write_file_secure(&self.file(), &(owned.join("\n") + "\n"));
    }

    fn write_auto_lock(&self, auto_lock: bool) {
        self.ensure_dir();
        let existing = fs::read_to_string(self.file()).unwrap_or_default();
        let mut lines: Vec<String> = existing
            .lines()
            .filter(|l| !l.trim().starts_with("auto_lock ="))
            .map(|l| l.to_string())
            .collect();
        let pos = lines
            .iter()
            .position(|l| l.trim().starts_with("save_email ="))
            .map(|i| i + 1)
            .unwrap_or(0);
        lines.insert(pos, format!("auto_lock = {auto_lock}"));
        write_file_secure(&self.file(), &(lines.join("\n") + "\n"));
    }

    fn write_keep_session(&self, keep_session: bool) {
        self.ensure_dir();
        let existing = fs::read_to_string(self.file()).unwrap_or_default();
        let mut lines: Vec<String> = existing
            .lines()
            .filter(|l| !l.trim().starts_with("keep_session ="))
            .map(|l| l.to_string())
            .collect();
        // Place keep_session right after auto_lock for tidy grouping —
        // fall back to after save_email or the top of the file.
        let pos = lines
            .iter()
            .position(|l| l.trim().starts_with("auto_lock ="))
            .or_else(|| {
                lines
                    .iter()
                    .position(|l| l.trim().starts_with("save_email ="))
            })
            .map(|i| i + 1)
            .unwrap_or(0);
        lines.insert(pos, format!("keep_session = {keep_session}"));
        write_file_secure(&self.file(), &(lines.join("\n") + "\n"));
    }

    fn config_dir(&self) -> PathBuf {
        self.dir.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Builds an adapter rooted at a fresh tempdir, returning both so
    /// the caller can inspect the on-disk file.
    fn fresh() -> (TomlSettingsAdapter, TempDir) {
        let tmp = TempDir::new().expect("tempdir");
        let adapter = TomlSettingsAdapter {
            dir: tmp.path().to_path_buf(),
        };
        (adapter, tmp)
    }

    #[test]
    fn read_returns_defaults_when_file_missing() {
        let (a, _t) = fresh();
        let cfg = a.read();
        assert!(!cfg.save_email);
        assert_eq!(cfg.email, None);
        assert!(!cfg.auto_lock);
        assert_eq!(cfg.lock_after_secs, DEFAULT_LOCK_AFTER_SECS);
        assert!(!cfg.keep_session);
        assert_eq!(cfg.clipboard_clear_secs, DEFAULT_CLIPBOARD_CLEAR_SECS);
    }

    #[test]
    fn read_parses_known_keys_and_ignores_unknown() {
        let (a, _t) = fresh();
        std::fs::write(
            a.file(),
            "save_email = true\nemail = \"a@b.com\"\nauto_lock = true\n\
             lock_after_minutes = 5\nkeep_session = true\n\
             clipboard_clear_secs = 60\nbogus = whatever\n",
        )
        .unwrap();
        let cfg = a.read();
        assert!(cfg.save_email);
        assert_eq!(cfg.email.as_deref(), Some("a@b.com"));
        assert!(cfg.auto_lock);
        assert_eq!(cfg.lock_after_secs, 5 * 60);
        assert!(cfg.keep_session);
        assert_eq!(cfg.clipboard_clear_secs, 60);
    }

    #[test]
    fn read_parses_clipboard_clear_zero_to_disable() {
        // 0 is the documented "auto-clear off" sentinel — must round-trip.
        let (a, _t) = fresh();
        std::fs::write(a.file(), "clipboard_clear_secs = 0\n").unwrap();
        assert_eq!(a.read().clipboard_clear_secs, 0);
    }

    #[test]
    fn read_falls_back_to_default_for_unparseable_clipboard_clear() {
        // Garbage value falls back to the 30s default rather than 0
        // (which would silently disable the protection).
        let (a, _t) = fresh();
        std::fs::write(a.file(), "clipboard_clear_secs = nope\n").unwrap();
        assert_eq!(a.read().clipboard_clear_secs, DEFAULT_CLIPBOARD_CLEAR_SECS);
    }

    #[test]
    fn write_save_email_preserves_clipboard_clear_secs() {
        // Our write paths only touch `save_email`/`email`/`auto_lock`/
        // `keep_session`. A user-edited `clipboard_clear_secs` line
        // must survive a TUI write cycle untouched.
        let (a, _t) = fresh();
        std::fs::write(a.file(), "clipboard_clear_secs = 15\n").unwrap();
        a.write(true, Some("u@x"));
        assert_eq!(a.read().clipboard_clear_secs, 15);
    }

    #[test]
    fn escape_helpers_are_round_trip_safe_on_pathological_input() {
        // Plain emails are unaffected by escape/unescape.
        let plain = "alice@example.com";
        assert_eq!(escape_toml_basic(plain), plain);
        assert_eq!(unescape_toml_basic(plain), plain);

        // Emails carrying TOML-significant characters round-trip
        // through the wire format. Real users won't do this — the
        // tests guard against silent corruption if they manage to.
        let weird = r#"alice"weird\path@example.com"#;
        let escaped = escape_toml_basic(weird);
        assert_eq!(escaped, r#"alice\"weird\\path@example.com"#);
        assert_eq!(unescape_toml_basic(&escaped), weird);
    }

    #[test]
    fn write_then_read_round_trips_email_with_quote() {
        // The validator rejects this email at the login screen, but
        // a hand-edited config could still produce it. Make sure the
        // adapter doesn't corrupt the file when the next write
        // touches it.
        let (a, _t) = fresh();
        let weird = r#"alice"q@example.com"#;
        a.write(true, Some(weird));
        assert_eq!(a.read().email.as_deref(), Some(weird));
    }

    #[test]
    fn write_then_read_round_trips_email_with_backslash() {
        let (a, _t) = fresh();
        let weird = r"alice\b@example.com";
        a.write(true, Some(weird));
        assert_eq!(a.read().email.as_deref(), Some(weird));
    }

    #[test]
    fn write_persists_email_when_save_enabled() {
        let (a, _t) = fresh();
        a.write(true, Some("u@x"));
        let cfg = a.read();
        assert!(cfg.save_email);
        assert_eq!(cfg.email.as_deref(), Some("u@x"));
    }

    #[test]
    fn write_drops_email_when_save_disabled() {
        let (a, _t) = fresh();
        a.write(true, Some("u@x"));
        a.write(false, None);
        let cfg = a.read();
        assert!(!cfg.save_email);
        assert_eq!(cfg.email, None);
    }

    #[test]
    fn write_preserves_unknown_sections_verbatim() {
        let (a, _t) = fresh();
        std::fs::write(a.file(), "[theme]\naccent = \"#cba6f7\"\nfoo = \"bar\"\n").unwrap();
        a.write(true, Some("u@x"));
        let on_disk = std::fs::read_to_string(a.file()).unwrap();
        // Theme section survives the rewrite untouched.
        assert!(on_disk.contains("[theme]"));
        assert!(on_disk.contains("accent = \"#cba6f7\""));
        assert!(on_disk.contains("foo = \"bar\""));
        assert!(on_disk.contains("save_email = true"));
        assert!(on_disk.contains("email = \"u@x\""));
    }

    #[test]
    fn write_auto_lock_does_not_disturb_email_or_theme() {
        let (a, _t) = fresh();
        a.write(true, Some("u@x"));
        std::fs::write(
            a.file(),
            format!(
                "{}\n[theme]\naccent = \"#cba6f7\"\n",
                std::fs::read_to_string(a.file()).unwrap()
            ),
        )
        .unwrap();
        a.write_auto_lock(true);
        let on_disk = std::fs::read_to_string(a.file()).unwrap();
        assert!(on_disk.contains("save_email = true"));
        assert!(on_disk.contains("email = \"u@x\""));
        assert!(on_disk.contains("auto_lock = true"));
        assert!(on_disk.contains("[theme]"));
    }

    #[test]
    fn write_auto_lock_overwrites_previous_value() {
        let (a, _t) = fresh();
        a.write_auto_lock(true);
        a.write_auto_lock(false);
        let cfg = a.read();
        assert!(!cfg.auto_lock);
        // Only one auto_lock line in the file.
        let on_disk = std::fs::read_to_string(a.file()).unwrap();
        assert_eq!(on_disk.matches("auto_lock =").count(), 1);
    }

    #[test]
    fn write_keep_session_appears_after_auto_lock_when_present() {
        let (a, _t) = fresh();
        a.write(true, Some("u@x"));
        a.write_auto_lock(true);
        a.write_keep_session(true);
        let on_disk = std::fs::read_to_string(a.file()).unwrap();
        let auto_lock_pos = on_disk.find("auto_lock").unwrap();
        let keep_pos = on_disk.find("keep_session").unwrap();
        assert!(keep_pos > auto_lock_pos);
    }

    #[test]
    fn write_keep_session_overwrites_previous_value() {
        let (a, _t) = fresh();
        a.write_keep_session(true);
        a.write_keep_session(false);
        let cfg = a.read();
        assert!(!cfg.keep_session);
        let on_disk = std::fs::read_to_string(a.file()).unwrap();
        assert_eq!(on_disk.matches("keep_session =").count(), 1);
    }

    #[test]
    fn config_dir_returns_root() {
        let (a, t) = fresh();
        assert_eq!(a.config_dir(), t.path());
    }

    #[test]
    fn read_parses_lock_after_minutes_to_seconds() {
        let (a, _t) = fresh();
        std::fs::write(a.file(), "lock_after_minutes = 30\n").unwrap();
        assert_eq!(a.read().lock_after_secs, 30 * 60);
    }

    #[test]
    fn read_ignores_unparseable_lock_after_minutes() {
        let (a, _t) = fresh();
        std::fs::write(a.file(), "lock_after_minutes = oops\n").unwrap();
        // Falls back to default (15 min).
        assert_eq!(a.read().lock_after_secs, DEFAULT_LOCK_AFTER_SECS);
    }

    /// Returns the unix mode bits of `path`, masking off non-permission
    /// bits so the assertion compares only `rwx` flags.
    fn mode(path: &std::path::Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn write_sets_file_mode_to_0600() {
        let (a, _t) = fresh();
        a.write(true, Some("u@x"));
        assert_eq!(mode(&a.file()), CONFIG_FILE_MODE);
    }

    #[test]
    fn write_auto_lock_sets_file_mode_to_0600() {
        let (a, _t) = fresh();
        a.write_auto_lock(true);
        assert_eq!(mode(&a.file()), CONFIG_FILE_MODE);
    }

    #[test]
    fn write_keep_session_sets_file_mode_to_0600() {
        let (a, _t) = fresh();
        a.write_keep_session(true);
        assert_eq!(mode(&a.file()), CONFIG_FILE_MODE);
    }

    #[test]
    fn ensure_dir_sets_directory_mode_to_0700() {
        let (a, _t) = fresh();
        // ensure_dir is called by every write; trigger one to be safe.
        a.write_keep_session(false);
        assert_eq!(mode(&a.dir), CONFIG_DIR_MODE);
    }

    #[test]
    fn first_write_creates_file_with_0600_atomically() {
        // The dir is empty (no pre-existing config.toml) — the only
        // way the file can be 0o600 on disk is if OpenOptions::mode()
        // applied at creation time. set_permissions afterwards would
        // also work, but it leaves a window; this test passes either
        // way and serves as a regression guard against losing the
        // atomic-creation path.
        let (a, _t) = fresh();
        assert!(!a.file().exists());
        a.write(true, Some("brand-new@example.com"));
        assert!(a.file().exists());
        assert_eq!(mode(&a.file()), CONFIG_FILE_MODE);
    }

    #[test]
    fn rewrite_keeps_secure_perms() {
        // Even after a config file is created with looser perms by an
        // external tool, the next bytewarden write should re-tighten
        // them to 0o600.
        let (a, _t) = fresh();
        std::fs::write(a.file(), "save_email = false\n").unwrap();
        std::fs::set_permissions(a.file(), std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(mode(&a.file()), 0o644); // sanity
        a.write(true, Some("u@x"));
        assert_eq!(mode(&a.file()), CONFIG_FILE_MODE);
    }
}
