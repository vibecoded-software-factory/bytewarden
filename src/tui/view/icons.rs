//! UI icons with a font-independence fallback.
//!
//! The item-type markers and the search glass use **Nerd-font** private-use
//! glyphs, which render as tofu on a terminal without a patched font. A TUI
//! can't set the terminal's font, so the only knob it has is *which glyph* to
//! emit: [`IconSet`] lets the user pick the safe **Unicode** set (the default
//! — renders on any font) or the richer **Nerd** set, via the `icon_style`
//! setting. On a bare console the choice is overridden to Unicode; see
//! [`resolve_icons`] and [`crate::tui::glyph`].

use crate::tui::glyph::GlyphCaps;

/// Which glyph set the UI draws its font-dependent icons from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconSet {
    /// Widely-supported single-cell Unicode — renders on any font (the
    /// default).
    Unicode,
    /// Nerd-font glyphs — prettier, but need a patched (Nerd) font.
    Nerd,
}

/// Resolves the `icon_style` setting to an [`IconSet`]. Anything other than an
/// explicit `nerd` is the safe Unicode set, so an unset / typo'd value never
/// leaves the user staring at tofu.
pub fn resolve(setting: &str) -> IconSet {
    match setting.trim().to_ascii_lowercase().as_str() {
        "nerd" => IconSet::Nerd,
        _ => IconSet::Unicode,
    }
}

/// Resolves the effective icon set from the `icon_style` setting **and** the
/// terminal's glyph capability: a bare console is forced to Unicode whatever
/// the setting says, since it cannot draw Nerd glyphs at all.
pub fn resolve_icons(setting: &str, caps: GlyphCaps) -> IconSet {
    match caps {
        GlyphCaps::Console => IconSet::Unicode,
        GlyphCaps::Full => resolve(setting),
    }
}

impl IconSet {
    /// Sidebar marker for the Login item type.
    pub fn login(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰌋",
            IconSet::Unicode => "◆",
        }
    }

    /// Sidebar marker for the Card item type.
    pub fn card(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰻷",
            IconSet::Unicode => "■",
        }
    }

    /// Sidebar marker for the Identity item type.
    pub fn identity(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰀉",
            IconSet::Unicode => "●",
        }
    }

    /// Sidebar marker for the Secure Note item type.
    pub fn note(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰎞",
            IconSet::Unicode => "≡",
        }
    }

    /// Sidebar marker for the SSH Key item type.
    pub fn ssh(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰣀",
            IconSet::Unicode => "◇",
        }
    }

    /// Sidebar marker for the Trash view.
    pub fn trash(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰩺",
            IconSet::Unicode => "✗",
        }
    }

    /// Leading glass for a search / filter field.
    pub fn search(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰍉",
            IconSet::Unicode => "»",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_defaults_to_unicode_and_reads_nerd() {
        assert_eq!(resolve("nerd"), IconSet::Nerd);
        assert_eq!(resolve("NERD"), IconSet::Nerd);
        assert_eq!(resolve("unicode"), IconSet::Unicode);
        assert_eq!(resolve(""), IconSet::Unicode);
        assert_eq!(resolve("garbage"), IconSet::Unicode);
    }

    #[test]
    fn console_tier_forces_unicode_even_when_nerd_is_configured() {
        assert_eq!(resolve_icons("nerd", GlyphCaps::Console), IconSet::Unicode);
        assert_eq!(resolve_icons("nerd", GlyphCaps::Full), IconSet::Nerd);
        assert_eq!(resolve_icons("unicode", GlyphCaps::Full), IconSet::Unicode);
    }

    #[test]
    fn every_icon_is_a_single_column_in_both_sets() {
        for set in [IconSet::Unicode, IconSet::Nerd] {
            for glyph in [
                set.login(),
                set.card(),
                set.identity(),
                set.note(),
                set.ssh(),
                set.trash(),
                set.search(),
            ] {
                assert_eq!(
                    glyph.chars().count(),
                    1,
                    "icon {glyph} is not a single char"
                );
            }
        }
    }

    #[test]
    fn unicode_set_uses_no_private_use_glyphs() {
        // The whole point of the safe set: nothing in a Nerd private-use
        // block, which is exactly what tofus without a patched font.
        let set = IconSet::Unicode;
        for glyph in [
            set.login(),
            set.card(),
            set.identity(),
            set.note(),
            set.ssh(),
            set.trash(),
            set.search(),
        ] {
            let c = glyph.chars().next().unwrap();
            let cp = c as u32;
            let pua = matches!(cp, 0xE000..=0xF8FF | 0xF_0000..=0xF_FFFD | 0x10_0000..=0x10_FFFD);
            assert!(!pua, "unicode icon {glyph} is in a private-use block");
        }
    }
}
