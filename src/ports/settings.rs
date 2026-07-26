//! User-settings port.

use std::path::PathBuf;

/// Persisted user preferences.
#[derive(Debug, Clone, Default)]
pub struct UserSettings {
    /// Whether the e-mail entered on the login form should be remembered.
    pub save_email: bool,

    /// Last remembered e-mail (only populated when `save_email == true`).
    pub email: Option<String>,

    /// Whether the vault should auto-lock after a period of inactivity.
    pub auto_lock: bool,

    /// Inactivity threshold (in seconds) before an auto-lock fires.
    pub lock_after_secs: u64,

    /// Whether the unlocked `bw` session key should be persisted to a
    /// per-PPID runtime file so subsequent bytewarden launches under
    /// the **same** terminal skip the master-password prompt. Cleaned
    /// up the moment the parent shell dies (see
    /// [`crate::tui::session_file`]).
    pub keep_session: bool,

    /// Seconds after which a copied secret is auto-cleared from the
    /// system clipboard. `0` disables the feature. Default `30`,
    /// matching the Bitwarden GUI's clipboard timeout.
    ///
    /// The clear is contingent on the clipboard still holding the
    /// originally-copied value: if the user copied something else in
    /// the meantime, we leave their selection alone.
    pub clipboard_clear_secs: u64,

    /// Wall-clock budget (in seconds) for `bw list items` /
    /// `bw list items --trash`. These are local-only operations but
    /// decrypt every record and serialize to JSON, so very large vaults
    /// can take well over a minute. Default `180` (3 min).
    pub list_items_timeout_secs: u64,

    /// Which glyph set the UI draws its font-dependent icons from:
    /// `"unicode"` (the font-safe default) or `"nerd"` (needs a patched
    /// Nerd font). Any other value is treated as `"unicode"`.
    pub icon_style: String,
}

/// Abstraction over a settings store.
///
/// The current concrete implementation is a TOML file at
/// `~/.config/bytewarden/config.toml`, but a future GUI version could plug
/// in a system-keychain or registry-based adapter without touching the
/// rest of the code.
pub trait SettingsPort {
    /// Loads the settings, returning sensible defaults if the file is
    /// missing or malformed.
    fn read(&self) -> UserSettings;

    /// Persists the e-mail-related settings.
    ///
    /// `email` is only stored when `save_email == true`; passing
    /// `save_email == false` clears any stored e-mail.
    fn write(&self, save_email: bool, email: Option<&str>);

    /// Persists just the auto-lock toggle.
    fn write_auto_lock(&self, auto_lock: bool);

    /// Persists just the keep-session toggle.
    fn write_keep_session(&self, keep_session: bool);

    /// Persists the auto-lock inactivity threshold. Stored in the TOML as
    /// whole minutes (`lock_after_minutes`), matching what [`Self::read`]
    /// parses back.
    fn write_lock_after_secs(&self, secs: u64);

    /// Persists the clipboard auto-clear window (`clipboard_clear_secs`).
    /// `0` disables the feature.
    fn write_clipboard_clear_secs(&self, secs: u64);

    /// Persists the `bw list items` wall-clock budget
    /// (`list_items_timeout_secs`).
    fn write_list_items_timeout_secs(&self, secs: u64);

    /// Persists the chosen icon set (`icon_style = "unicode" | "nerd"`).
    fn write_icon_style(&self, style: &str);

    /// Persists the chosen theme preset as `name = "<preset>"` inside the
    /// `[theme]` section, preserving every other key (incl. per-color
    /// overrides). Best-effort, like the other writers.
    fn write_theme_name(&self, name: &str);

    /// Returns the directory that backs this settings store. The TUI uses
    /// this to discover the optional `[theme]` overrides file living next
    /// to `config.toml`.
    fn config_dir(&self) -> PathBuf;
}
