//! Settings overlay (`F9`) state.
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

/// A section of the Settings overlay. Sectioned so the preferences
/// surface can grow (Security, Clipboard…) without changing the layout.
/// Today only [`SettingsSection::Theme`] exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Theme,
}

impl SettingsSection {
    /// Every section, in sidebar order.
    pub const ALL: [SettingsSection; 1] = [SettingsSection::Theme];

    /// The sidebar label.
    pub fn label(self) -> &'static str {
        match self {
            SettingsSection::Theme => "Theme",
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
    /// Theme active when the overlay opened — restored if the user
    /// cancels (`Esc`/`F9`) instead of confirming.
    pub theme_before: Theme,
    /// Screen the overlay was opened from (returned to on close).
    pub from: Screen,
}
