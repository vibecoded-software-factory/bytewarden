use crate::tui::glyph::GlyphCaps;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconSet {
    Unicode,

    Nerd,
}

pub fn resolve(setting: &str) -> IconSet {
    match setting.trim().to_ascii_lowercase().as_str() {
        "nerd" => IconSet::Nerd,
        _ => IconSet::Unicode,
    }
}

pub fn resolve_icons(setting: &str, caps: GlyphCaps) -> IconSet {
    match caps {
        GlyphCaps::Console => IconSet::Unicode,
        GlyphCaps::Full => resolve(setting),
    }
}

impl IconSet {
    pub fn login(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰌋",
            IconSet::Unicode => "◆",
        }
    }

    pub fn card(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰿯",
            IconSet::Unicode => "■",
        }
    }

    pub fn identity(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰀉",
            IconSet::Unicode => "●",
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰎞",
            IconSet::Unicode => "≡",
        }
    }

    pub fn ssh(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰣀",
            IconSet::Unicode => "◇",
        }
    }

    pub fn trash(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰩺",
            IconSet::Unicode => "✗",
        }
    }

    pub fn search(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰍉",
            IconSet::Unicode => "»",
        }
    }

    pub fn folder(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰉋",
            IconSet::Unicode => "▤",
        }
    }

    pub fn collection(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰡉",
            IconSet::Unicode => "◫",
        }
    }

    pub fn org(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰦑",
            IconSet::Unicode => "⌂",
        }
    }

    pub fn locked(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰌾",
            IconSet::Unicode => "⊘",
        }
    }

    pub fn warning(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰀦",
            IconSet::Unicode => "!",
        }
    }

    pub fn email(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰇮",
            IconSet::Unicode => "@",
        }
    }

    pub fn two_factor(self) -> &'static str {
        match self {
            IconSet::Nerd => "󰦯",
            IconSet::Unicode => "#",
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

    fn all_icons(set: IconSet) -> [&'static str; 14] {
        [
            set.login(),
            set.card(),
            set.identity(),
            set.note(),
            set.ssh(),
            set.trash(),
            set.search(),
            set.folder(),
            set.collection(),
            set.org(),
            set.locked(),
            set.warning(),
            set.email(),
            set.two_factor(),
        ]
    }

    fn is_double_width_emoji(c: char) -> bool {
        matches!(c as u32,

            0x1F000..=0x1FAFF

            | 0x231A..=0x231B | 0x23E9..=0x23EC | 0x23F0 | 0x23F3
            | 0x25FD..=0x25FE | 0x2614..=0x2615 | 0x2648..=0x2653
            | 0x267F | 0x2693 | 0x26A1 | 0x26AA..=0x26AB
            | 0x26BD..=0x26BE | 0x26C4..=0x26C5 | 0x26CE | 0x26D4
            | 0x26EA | 0x26F2..=0x26F3 | 0x26F5 | 0x26FA | 0x26FD
            | 0x2705 | 0x270A..=0x270B | 0x2728 | 0x274C | 0x274E
            | 0x2753..=0x2755 | 0x2757 | 0x2795..=0x2797 | 0x27B0
            | 0x27BF | 0x2B1B..=0x2B1C | 0x2B50 | 0x2B55)
    }

    #[test]
    fn every_icon_is_a_single_char_in_both_sets() {
        for set in [IconSet::Unicode, IconSet::Nerd] {
            for glyph in all_icons(set) {
                assert_eq!(
                    glyph.chars().count(),
                    1,
                    "icon {glyph} is not a single char"
                );
            }
        }
    }

    #[test]
    fn no_icon_is_a_double_width_emoji() {
        for set in [IconSet::Unicode, IconSet::Nerd] {
            for glyph in all_icons(set) {
                let c = glyph.chars().next().expect("one char");
                assert!(
                    !is_double_width_emoji(c),
                    "icon {glyph} (U+{:04X}) is two cells wide",
                    c as u32
                );
            }
        }
    }

    #[test]
    fn the_width_guard_catches_the_emoji_that_were_removed() {
        for c in [
            '\u{1F4C1}',
            '\u{1F465}',
            '\u{1F3E2}',
            '\u{1F512}',
            '\u{1F510}',
        ] {
            assert!(
                is_double_width_emoji(c),
                "U+{:04X} should be caught",
                c as u32
            );
        }

        for c in ['\u{2605}', '\u{2717}', '\u{25C6}', '\u{00BB}'] {
            assert!(!is_double_width_emoji(c), "U+{:04X} is one cell", c as u32);
        }
    }

    #[test]
    fn unicode_set_uses_no_private_use_glyphs() {
        for glyph in all_icons(IconSet::Unicode) {
            let c = glyph.chars().next().unwrap();
            let cp = c as u32;
            let pua = matches!(cp, 0xE000..=0xF8FF | 0xF_0000..=0xF_FFFD | 0x10_0000..=0x10_FFFD);
            assert!(!pua, "unicode icon {glyph} is in a private-use block");
        }
    }

    #[test]
    fn nerd_set_stays_in_the_material_design_range() {
        for glyph in all_icons(IconSet::Nerd) {
            let cp = glyph.chars().next().expect("one char") as u32;

            if cp < 0x80 {
                continue;
            }
            assert!(
                (0xF0001..=0xF1AF0).contains(&cp),
                "nerd icon {glyph} (U+{cp:05X}) is outside the nf-md range"
            );
        }
    }
}
