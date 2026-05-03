//! Theme system.
//!
//! Reads the optional `[theme]` section of `config.toml`. Every key is
//! optional — only the entries present override the built-in defaults,
//! so partial configs are valid.
//!
//! ```toml
//! [theme]
//! accent          = "#cba6f7"   # active borders, cursor, highlights
//! inactive        = "#6c7086"   # inactive panel borders
//! selected_bg     = "#313244"   # selected row background
//! success         = "#a6e3a1"   # success messages
//! error           = "#f38ba8"   # error messages
//! dim             = "#585b70"   # secondary text, hints, counters
//! foreground      = "#cdd6f4"   # main text (omit to inherit terminal fg)
//! placeholder     = "#505578"   # empty-input "type here…" hints
//! muted           = "#3c3e50"   # decorative separators / barely-visible borders
//! star_dim        = "#262248"   # dimmest decorative star
//! star_mid        = "#5a5494"   # mid-brightness star
//! star_bright     = "#b9b2f8"   # rare bright star
//! item_login      = "#89b4fa"
//! item_card       = "#cba6f7"
//! item_identity   = "#f9e2af"
//! item_note       = "#a6e3a1"
//! item_ssh        = "#b4befe"
//! item_favorite   = "#f9e2af"
//! ```

use std::path::Path;

use ratatui::style::Color;

/// Resolved color palette.
#[derive(Debug, Clone)]
pub struct Theme {
    pub accent: Color,
    pub inactive: Color,
    pub selected_bg: Color,
    pub success: Color,
    pub error: Color,
    pub dim: Color,
    /// Main body-text color. Defaults to [`Color::Reset`] so the TUI
    /// inherits the terminal's foreground — the most portable choice.
    /// Override to a hex value to lock a specific tone.
    pub foreground: Color,
    /// "Type here…" placeholder hint inside empty input boxes.
    pub placeholder: Color,
    /// Decorative separators and barely-visible borders (e.g. the bar
    /// between command-log and content).
    pub muted: Color,
    /// Dimmest decorative star in the splash / login background.
    pub star_dim: Color,
    /// Mid-brightness decorative star.
    pub star_mid: Color,
    /// Rare bright decorative star.
    pub star_bright: Color,
    pub item_login: Color,
    pub item_card: Color,
    pub item_identity: Color,
    pub item_note: Color,
    pub item_ssh: Color,
    pub item_favorite: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: Color::Cyan,
            inactive: Color::Rgb(140, 140, 160),
            selected_bg: Color::Rgb(30, 60, 80),
            success: Color::Green,
            error: Color::Red,
            dim: Color::DarkGray,
            foreground: Color::Reset,
            placeholder: Color::Rgb(80, 85, 120),
            muted: Color::Rgb(60, 62, 80),
            star_dim: Color::Rgb(38, 34, 72),
            star_mid: Color::Rgb(90, 84, 148),
            star_bright: Color::Rgb(185, 178, 248),
            item_login: Color::Rgb(91, 143, 255),
            item_card: Color::Rgb(192, 96, 224),
            item_identity: Color::Rgb(224, 184, 64),
            item_note: Color::Rgb(0, 200, 150),
            item_ssh: Color::Rgb(160, 96, 224),
            item_favorite: Color::Rgb(255, 200, 0),
        }
    }
}

/// Loads the theme from the `[theme]` section of `<config_dir>/config.toml`.
///
/// Returns [`Theme::default`] when the file or section is missing.
pub fn load(config_dir: &Path) -> Theme {
    let file = config_dir.join("config.toml");
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Theme::default();
    };
    parse_theme_section(&text)
}

/// Parses individual color overrides from the `[theme]` section.
fn parse_theme_section(text: &str) -> Theme {
    let mut t = Theme::default();
    let mut in_theme = false;

    for line in text.lines() {
        let line = line.trim();
        if line == "[theme]" {
            in_theme = true;
            continue;
        }
        if line.starts_with('[') {
            in_theme = false;
            continue;
        }
        if !in_theme {
            continue;
        }
        let Some((key, rest)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let val = rest.trim();
        // Support both quoted ("#rrggbb") and unquoted values; ignore
        // inline comments after the value.
        let val = if val.starts_with('"') {
            val.trim_start_matches('"')
                .split('"')
                .next()
                .unwrap_or("")
                .trim()
        } else {
            val.split(' ').next().unwrap_or("").trim()
        };
        if val.len() != 7 || !val.starts_with('#') {
            continue;
        }
        let color = parse_hex(val);
        match key {
            "accent" => t.accent = color,
            "inactive" => t.inactive = color,
            "selected_bg" => t.selected_bg = color,
            "success" => t.success = color,
            "error" => t.error = color,
            "dim" => t.dim = color,
            "foreground" => t.foreground = color,
            "placeholder" => t.placeholder = color,
            "muted" => t.muted = color,
            "star_dim" => t.star_dim = color,
            "star_mid" => t.star_mid = color,
            "star_bright" => t.star_bright = color,
            "item_login" => t.item_login = color,
            "item_card" => t.item_card = color,
            "item_identity" => t.item_identity = color,
            "item_note" => t.item_note = color,
            "item_ssh" => t.item_ssh = color,
            "item_favorite" => t.item_favorite = color,
            _ => {}
        }
    }
    t
}

/// Parses a hex color string like `"#cba6f7"` into [`Color::Rgb`].
///
/// Returns [`Color::Reset`] on parse error.
fn parse_hex(s: &str) -> Color {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return Color::Reset;
    }
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
    Color::Rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parse_hex_roundtrips_known_value() {
        assert_eq!(parse_hex("#cba6f7"), Color::Rgb(0xcb, 0xa6, 0xf7));
    }

    #[test]
    fn parse_hex_accepts_unprefixed_values() {
        assert_eq!(parse_hex("000000"), Color::Rgb(0, 0, 0));
        assert_eq!(parse_hex("ffffff"), Color::Rgb(255, 255, 255));
    }

    #[test]
    fn parse_hex_rejects_wrong_length() {
        assert_eq!(parse_hex("#abc"), Color::Reset);
        assert_eq!(parse_hex("#1234567"), Color::Reset);
    }

    #[test]
    fn theme_default_uses_reset_for_foreground() {
        // Foreground default inherits the terminal — important for
        // light-bg terminals that the user might use.
        assert_eq!(Theme::default().foreground, Color::Reset);
    }

    #[test]
    fn parse_section_overrides_only_listed_keys() {
        let toml = "\
            save_email = true\n\
            [theme]\n\
            accent = \"#112233\"\n\
            foreground = \"#445566\"\n\
            unknown_key = \"#aabbcc\"\n";
        let t = parse_theme_section(toml);
        assert_eq!(t.accent, Color::Rgb(0x11, 0x22, 0x33));
        assert_eq!(t.foreground, Color::Rgb(0x44, 0x55, 0x66));
        // Unlisted keys keep the defaults.
        assert_eq!(t.success, Theme::default().success);
    }

    #[test]
    fn parse_section_ignores_keys_outside_theme_block() {
        // The same key name outside [theme] should be ignored.
        let toml = "\
            accent = \"#112233\"\n\
            [other]\n\
            accent = \"#445566\"\n";
        let t = parse_theme_section(toml);
        assert_eq!(t.accent, Theme::default().accent);
    }

    #[test]
    fn parse_section_accepts_unquoted_values() {
        let toml = "[theme]\naccent = #112233\n";
        let t = parse_theme_section(toml);
        assert_eq!(t.accent, Color::Rgb(0x11, 0x22, 0x33));
    }

    #[test]
    fn parse_section_drops_malformed_values() {
        let toml = "[theme]\naccent = \"not a color\"\nfoo\n";
        let t = parse_theme_section(toml);
        assert_eq!(t.accent, Theme::default().accent);
    }

    #[test]
    fn load_returns_default_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let theme = load(tmp.path());
        assert_eq!(theme.accent, Theme::default().accent);
    }

    #[test]
    fn load_reads_existing_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("config.toml"),
            "[theme]\naccent = \"#abcdef\"\n",
        )
        .unwrap();
        let theme = load(tmp.path());
        assert_eq!(theme.accent, Color::Rgb(0xab, 0xcd, 0xef));
    }
}
