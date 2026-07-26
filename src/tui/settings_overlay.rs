//! Settings overlay (`F10`) state.
//!
//! The preferences overlay's transient state, split out of the
//! [`crate::tui::app::App`] god-struct into its own screen-local
//! container (the same move the login form and item forms already
//! made). The overlay's focus/section enums live here too, beside the
//! state they describe.

use crate::tui::screens::Screen;
use crate::tui::theme::Theme;

/// Which pane of the Settings overlay currently holds focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsFocus {
    /// The left-hand list of sections.
    Sidebar,
    /// The right-hand panel showing the active section's options.
    Panel,
}

/// A section of the Settings overlay. The Theme section is a live preset
/// picker; the others are lists of value rows edited in place with
/// `←/→`, each mapped to a key in `config.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Theme,
    Security,
    Advanced,
}

impl SettingsSection {
    /// Every section, in sidebar order.
    pub const ALL: [SettingsSection; 3] = [
        SettingsSection::Theme,
        SettingsSection::Security,
        SettingsSection::Advanced,
    ];

    /// The sidebar label.
    pub fn label(self) -> &'static str {
        match self {
            SettingsSection::Theme => "Theme",
            SettingsSection::Security => "Security",
            SettingsSection::Advanced => "Advanced",
        }
    }

    /// The editable value rows of a non-Theme section (empty for Theme,
    /// which has its own picker panel).
    pub fn rows(self) -> &'static [SettingRow] {
        match self {
            SettingsSection::Theme => &[],
            SettingsSection::Security => &[
                SettingRow::AutoLock,
                SettingRow::LockAfter,
                SettingRow::KeepSession,
                SettingRow::RememberEmail,
            ],
            SettingsSection::Advanced => &[
                SettingRow::ClipboardClear,
                SettingRow::ListTimeout,
                SettingRow::IconStyle,
            ],
        }
    }
}

/// One editable preference row, backing a single `config.toml` key. The
/// current value + the change/persist logic live on
/// [`crate::tui::app::App`] (which holds both the state and the settings
/// port); this enum only names the rows and their static chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingRow {
    /// `auto_lock` — lock the vault after inactivity (toggle).
    AutoLock,
    /// `lock_after_minutes` — the inactivity window.
    LockAfter,
    /// `keep_session` — persist the session key per-terminal (toggle).
    KeepSession,
    /// `save_email` — remember the login e-mail (toggle).
    RememberEmail,
    /// `clipboard_clear_secs` — auto-clear a copied secret.
    ClipboardClear,
    /// `list_items_timeout_secs` — `bw list items` wall-clock budget.
    ListTimeout,
    /// `icon_style` — which glyph set the UI draws icons from.
    IconStyle,
}

impl SettingRow {
    /// The row's user-visible label.
    pub fn label(self) -> &'static str {
        match self {
            SettingRow::AutoLock => "Auto-lock",
            SettingRow::LockAfter => "Lock after",
            SettingRow::KeepSession => "Keep session",
            SettingRow::RememberEmail => "Remember email",
            SettingRow::ClipboardClear => "Clipboard clear",
            SettingRow::ListTimeout => "List timeout",
            SettingRow::IconStyle => "Icons",
        }
    }

    /// A one-line hint for the focused row, pinned under the panel.
    pub fn hint(self) -> &'static str {
        match self {
            SettingRow::AutoLock => "←/→ toggle locking the vault after inactivity",
            SettingRow::LockAfter => "←/→ adjust the inactivity window (minutes)",
            SettingRow::KeepSession => "←/→ toggle reusing the session in this terminal",
            SettingRow::RememberEmail => "←/→ toggle remembering the login e-mail",
            SettingRow::ClipboardClear => "←/→ adjust auto-clear seconds (0 = off)",
            SettingRow::ListTimeout => "←/→ adjust the bw list-items timeout (seconds)",
            SettingRow::IconStyle => {
                "←/→ switch icon set (Unicode is font-safe; Nerd needs a patched font)"
            }
        }
    }
}

/// Transient state of the Settings overlay while it is open.
pub struct SettingsOverlay {
    /// Which pane holds focus.
    pub focus: SettingsFocus,
    /// Highlighted section in the sidebar (index into
    /// [`SettingsSection::ALL`]).
    pub section: usize,
    /// Highlighted preset in the Theme panel (index into
    /// [`crate::tui::theme::Preset::ALL`]). Previews live as it moves.
    pub theme_idx: usize,
    /// Highlighted row in a value-list section (index into the section's
    /// [`SettingsSection::rows`]). Reset to 0 when the section changes.
    pub row: usize,
    /// Theme active when the overlay opened — restored if the user
    /// cancels (`Esc`/`F10`) instead of confirming.
    pub theme_before: Theme,
    /// Screen the overlay was opened from (returned to on close).
    pub from: Screen,
}
