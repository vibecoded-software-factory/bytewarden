use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::ports::{SettingsPort, UserSettings};

const DEFAULT_LOCK_AFTER_SECS: u64 = 15 * 60;

const DEFAULT_CLIPBOARD_CLEAR_SECS: u64 = 30;

const DEFAULT_LIST_ITEMS_TIMEOUT_SECS: u64 = 60;

const DEFAULT_ICON_STYLE: &str = "unicode";

fn escape_toml_basic(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn unescape_toml_basic(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
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

const CONFIG_FILE_MODE: u32 = 0o600;

const CONFIG_DIR_MODE: u32 = 0o700;

#[derive(Debug, Clone)]
pub struct TomlSettingsAdapter {
    dir: PathBuf,
}

impl TomlSettingsAdapter {
    pub fn new() -> Self {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        Self {
            dir: home.join(".config").join("bytewarden"),
        }
    }

    fn file(&self) -> PathBuf {
        self.dir.join("config.toml")
    }

    fn ensure_dir(&self) {
        let _ = fs::DirBuilder::new()
            .recursive(true)
            .mode(CONFIG_DIR_MODE)
            .create(&self.dir);
        let _ = fs::set_permissions(&self.dir, fs::Permissions::from_mode(CONFIG_DIR_MODE));
    }

    fn upsert_scalar(&self, key: &str, value: String, after: &[&str]) {
        self.ensure_dir();
        let existing = fs::read_to_string(self.file()).unwrap_or_default();
        let prefix = format!("{key} =");
        let mut lines: Vec<String> = existing
            .lines()
            .filter(|l| !l.trim().starts_with(&prefix))
            .map(|l| l.to_string())
            .collect();
        let pos = after
            .iter()
            .find_map(|k| {
                let p = format!("{k} =");
                lines.iter().position(|l| l.trim().starts_with(&p))
            })
            .map(|i| i + 1)
            .unwrap_or(0);
        lines.insert(pos, format!("{key} = {value}"));
        write_file_secure(&self.file(), &(lines.join("\n") + "\n"));
    }
}

fn write_file_secure(path: &Path, contents: &str) {
    let tmp = {
        let mut p = path.as_os_str().to_owned();
        p.push(".tmp");
        std::path::PathBuf::from(p)
    };
    let write = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(CONFIG_FILE_MODE)
            .open(&tmp)?;
        file.write_all(contents.as_bytes())?;

        file.sync_all()?;
        Ok(())
    })();
    if write.is_err() {
        let _ = fs::remove_file(&tmp);
        return;
    }
    if fs::rename(&tmp, path).is_err() {
        let _ = fs::remove_file(&tmp);
        return;
    }

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
            icon_style: DEFAULT_ICON_STYLE.to_string(),
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
                cfg.list_items_timeout_secs = s;
            } else if let Some(v) = line.strip_prefix("icon_style = ") {
                let inner = v.trim().trim_matches('"');
                if !inner.is_empty() {
                    cfg.icon_style = inner.to_string();
                }
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

    fn write_lock_after_secs(&self, secs: u64) {
        self.upsert_scalar(
            "lock_after_minutes",
            (secs / 60).to_string(),
            &["auto_lock", "keep_session", "save_email"],
        );
    }

    fn write_clipboard_clear_secs(&self, secs: u64) {
        self.upsert_scalar(
            "clipboard_clear_secs",
            secs.to_string(),
            &["keep_session", "auto_lock", "save_email"],
        );
    }

    fn write_list_items_timeout_secs(&self, secs: u64) {
        self.upsert_scalar(
            "list_items_timeout_secs",
            secs.to_string(),
            &["clipboard_clear_secs", "keep_session", "save_email"],
        );
    }

    fn write_icon_style(&self, style: &str) {
        self.upsert_scalar(
            "icon_style",
            format!("\"{style}\""),
            &[
                "list_items_timeout_secs",
                "clipboard_clear_secs",
                "save_email",
            ],
        );
    }

    fn write_theme_name(&self, name: &str) {
        self.ensure_dir();
        let existing = fs::read_to_string(self.file()).unwrap_or_default();
        write_file_secure(&self.file(), &upsert_theme_name(&existing, name));
    }

    fn config_dir(&self) -> PathBuf {
        self.dir.clone()
    }
}

fn upsert_theme_name(raw: &str, name: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_theme = false;
    let mut found_theme = false;
    for line in raw.lines() {
        let t = line.trim();
        let is_header = t.starts_with('[') && t.ends_with(']');
        if is_header {
            in_theme = t == "[theme]";
            out.push(line.to_string());
            if in_theme {
                found_theme = true;
                out.push(format!("name = \"{name}\""));
            }
            continue;
        }

        if in_theme && t.split_once('=').is_some_and(|(k, _)| k.trim() == "name") {
            continue;
        }
        out.push(line.to_string());
    }
    let mut text = out.join("\n");
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    if !found_theme {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&format!("[theme]\nname = \"{name}\"\n"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fresh() -> (TomlSettingsAdapter, TempDir) {
        let tmp = TempDir::new().expect("tempdir");
        let adapter = TomlSettingsAdapter {
            dir: tmp.path().to_path_buf(),
        };
        (adapter, tmp)
    }

    #[test]
    fn write_file_secure_is_atomic_and_owner_only() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("config.toml");
        write_file_secure(&path, "a = 1\n");
        assert_eq!(fs::read_to_string(&path).unwrap(), "a = 1\n");

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        assert!(!tmp.path().join("config.toml.tmp").exists());

        write_file_secure(&path, "b = 2\n");
        assert_eq!(fs::read_to_string(&path).unwrap(), "b = 2\n");
        assert!(!tmp.path().join("config.toml.tmp").exists());
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
        let (a, _t) = fresh();
        std::fs::write(a.file(), "clipboard_clear_secs = 0\n").unwrap();
        assert_eq!(a.read().clipboard_clear_secs, 0);
    }

    #[test]
    fn read_falls_back_to_default_for_unparseable_clipboard_clear() {
        let (a, _t) = fresh();
        std::fs::write(a.file(), "clipboard_clear_secs = nope\n").unwrap();
        assert_eq!(a.read().clipboard_clear_secs, DEFAULT_CLIPBOARD_CLEAR_SECS);
    }

    #[test]
    fn read_defaults_icon_style_to_font_safe_unicode() {
        let (a, _t) = fresh();
        assert_eq!(a.read().icon_style, DEFAULT_ICON_STYLE);
    }

    #[test]
    fn write_icon_style_round_trips_through_the_quoted_value() {
        let (a, _t) = fresh();
        a.write_icon_style("nerd");
        assert_eq!(a.read().icon_style, "nerd");
        a.write_icon_style("unicode");
        assert_eq!(a.read().icon_style, "unicode");
    }

    #[test]
    fn write_icon_style_preserves_other_keys() {
        let (a, _t) = fresh();
        std::fs::write(a.file(), "clipboard_clear_secs = 45\n").unwrap();
        a.write_icon_style("nerd");
        let cfg = a.read();
        assert_eq!(cfg.icon_style, "nerd");
        assert_eq!(cfg.clipboard_clear_secs, 45);
    }

    #[test]
    fn write_save_email_preserves_clipboard_clear_secs() {
        let (a, _t) = fresh();
        std::fs::write(a.file(), "clipboard_clear_secs = 15\n").unwrap();
        a.write(true, Some("u@x"));
        assert_eq!(a.read().clipboard_clear_secs, 15);
    }

    #[test]
    fn escape_helpers_are_round_trip_safe_on_pathological_input() {
        let plain = "alice@example.com";
        assert_eq!(escape_toml_basic(plain), plain);
        assert_eq!(unescape_toml_basic(plain), plain);

        let weird = r#"alice"weird\path@example.com"#;
        let escaped = escape_toml_basic(weird);
        assert_eq!(escaped, r#"alice\"weird\\path@example.com"#);
        assert_eq!(unescape_toml_basic(&escaped), weird);
    }

    #[test]
    fn write_then_read_round_trips_email_with_quote() {
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

        assert!(on_disk.contains("[theme]"));
        assert!(on_disk.contains("accent = \"#cba6f7\""));
        assert!(on_disk.contains("foo = \"bar\""));
        assert!(on_disk.contains("save_email = true"));
        assert!(on_disk.contains("email = \"u@x\""));
    }

    #[test]
    fn write_theme_name_inserts_into_section_preserving_overrides() {
        let (a, _t) = fresh();
        std::fs::write(
            a.file(),
            "save_email = true\n[theme]\naccent = \"#ff0000\"\n",
        )
        .unwrap();
        a.write_theme_name("dracula");
        let out = std::fs::read_to_string(a.file()).unwrap();
        assert!(out.contains("name = \"dracula\""));
        assert!(out.contains("accent = \"#ff0000\""));
        assert!(out.contains("save_email = true"));
        let theme_at = out.find("[theme]").unwrap();
        let name_at = out.find("name = \"dracula\"").unwrap();
        assert!(name_at > theme_at, "name must sit inside [theme]");

        a.write_theme_name("nord");
        let out2 = std::fs::read_to_string(a.file()).unwrap();
        assert_eq!(out2.matches("name = ").count(), 1);
        assert!(out2.contains("name = \"nord\""));
    }

    #[test]
    fn write_theme_name_creates_section_when_absent() {
        let (a, _t) = fresh();
        std::fs::write(a.file(), "save_email = true\n").unwrap();
        a.write_theme_name("nord");
        let out = std::fs::read_to_string(a.file()).unwrap();
        assert!(out.contains("[theme]"));
        assert!(out.contains("name = \"nord\""));
        assert!(out.contains("save_email = true"));
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

        assert_eq!(a.read().lock_after_secs, DEFAULT_LOCK_AFTER_SECS);
    }

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

        a.write_keep_session(false);
        assert_eq!(mode(&a.dir), CONFIG_DIR_MODE);
    }

    #[test]
    fn first_write_creates_file_with_0600_atomically() {
        let (a, _t) = fresh();
        assert!(!a.file().exists());
        a.write(true, Some("brand-new@example.com"));
        assert!(a.file().exists());
        assert_eq!(mode(&a.file()), CONFIG_FILE_MODE);
    }

    #[test]
    fn rewrite_keeps_secure_perms() {
        let (a, _t) = fresh();
        std::fs::write(a.file(), "save_email = false\n").unwrap();
        std::fs::set_permissions(a.file(), std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(mode(&a.file()), 0o644);
        a.write(true, Some("u@x"));
        assert_eq!(mode(&a.file()), CONFIG_FILE_MODE);
    }

    #[test]
    fn write_lock_after_secs_round_trips_as_minutes() {
        let (a, _t) = fresh();
        a.write_lock_after_secs(15 * 60);
        let on_disk = std::fs::read_to_string(a.file()).unwrap();
        assert!(on_disk.contains("lock_after_minutes = 15"));
        assert_eq!(a.read().lock_after_secs, 15 * 60);
    }

    #[test]
    fn write_clipboard_clear_secs_round_trips_and_overwrites() {
        let (a, _t) = fresh();
        a.write_clipboard_clear_secs(45);
        a.write_clipboard_clear_secs(0);
        let on_disk = std::fs::read_to_string(a.file()).unwrap();
        assert_eq!(on_disk.matches("clipboard_clear_secs =").count(), 1);
        assert_eq!(a.read().clipboard_clear_secs, 0);
    }

    #[test]
    fn write_list_items_timeout_secs_round_trips() {
        let (a, _t) = fresh();
        a.write_list_items_timeout_secs(240);
        assert_eq!(a.read().list_items_timeout_secs, 240);
    }

    #[test]
    fn scalar_writers_preserve_email_and_theme() {
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
        a.write_clipboard_clear_secs(10);
        a.write_list_items_timeout_secs(300);
        let on_disk = std::fs::read_to_string(a.file()).unwrap();
        assert!(on_disk.contains("save_email = true"));
        assert!(on_disk.contains("email = \"u@x\""));
        assert!(on_disk.contains("clipboard_clear_secs = 10"));
        assert!(on_disk.contains("list_items_timeout_secs = 300"));
        assert!(on_disk.contains("[theme]"));
        assert!(on_disk.contains("accent = \"#cba6f7\""));
    }
}
