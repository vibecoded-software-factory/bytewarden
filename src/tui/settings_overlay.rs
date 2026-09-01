use crate::tui::screens::Screen;
use crate::tui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsFocus {
    Sidebar,

    Panel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Theme,
    Security,
    Advanced,
}

impl SettingsSection {
    pub const ALL: [SettingsSection; 3] = [
        SettingsSection::Theme,
        SettingsSection::Security,
        SettingsSection::Advanced,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SettingsSection::Theme => "Theme",
            SettingsSection::Security => "Security",
            SettingsSection::Advanced => "Advanced",
        }
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingRow {
    AutoLock,

    LockAfter,

    KeepSession,

    RememberEmail,

    ClipboardClear,

    ListTimeout,

    IconStyle,
}

impl SettingRow {
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

    pub fn hint(self) -> &'static str {
        match self {
            SettingRow::AutoLock => "Lock the vault after a period of inactivity",
            SettingRow::LockAfter => "How long the vault may sit idle before it locks",
            SettingRow::KeepSession => "Reuse this terminal's session on the next launch",
            SettingRow::RememberEmail => "Pre-fill the login e-mail next time",
            SettingRow::ClipboardClear => "Wipe a copied secret after this long (0 = never)",
            SettingRow::ListTimeout => "Wall-clock budget for listing a large vault",
            SettingRow::IconStyle => "Unicode renders on any font; Nerd needs a patched one",
        }
    }
}

pub struct SettingsOverlay {
    pub focus: SettingsFocus,

    pub section: usize,

    pub theme_idx: usize,

    pub row: usize,

    pub theme_before: Theme,

    pub from: Screen,
}
