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

/// A named base palette — the raw colors a [`Preset`] is built from.
///
/// The same 13 roles exist verbatim in all three sibling TUIs
/// (bytewarden, jewel, secretbase), so the shared/core `Theme` fields
/// map identically across them; each app maps the remaining roles to
/// its own domain colors in [`Theme::from_palette`]. This is what keeps
/// the palettes coherent across the three apps.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub base: Color,
    pub surface: Color,
    pub overlay: Color,
    pub muted: Color,
    pub text: Color,
    pub accent: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub orange: Color,
}

/// A bundled, named theme. The default (and the shared default across
/// the three TUIs) is [`Preset::CatppuccinMocha`]. Selected via
/// `name = "<preset>"` in the `[theme]` section of `config.toml`, or
/// live from the in-app theme picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    CatppuccinMocha,
    Dracula,
    Nord,
    CatppuccinLatte,
}

impl Preset {
    /// Every bundled preset, in picker order (dark first, light last).
    pub const ALL: [Preset; 4] = [
        Preset::CatppuccinMocha,
        Preset::Dracula,
        Preset::Nord,
        Preset::CatppuccinLatte,
    ];

    /// The shared default preset across the three TUIs — used when the
    /// config names no preset.
    pub const DEFAULT: Preset = Preset::Nord;

    /// The stable config key (lower-kebab) written to `config.toml`.
    pub fn name(self) -> &'static str {
        match self {
            Preset::CatppuccinMocha => "catppuccin-mocha",
            Preset::Dracula => "dracula",
            Preset::Nord => "nord",
            Preset::CatppuccinLatte => "catppuccin-latte",
        }
    }

    /// The human-readable label shown in the picker.
    pub fn label(self) -> &'static str {
        match self {
            Preset::CatppuccinMocha => "Catppuccin Mocha",
            Preset::Dracula => "Dracula",
            Preset::Nord => "Nord",
            Preset::CatppuccinLatte => "Catppuccin Latte (light)",
        }
    }

    /// Resolves a config `name` value (case-insensitive) to a preset.
    pub fn from_name(name: &str) -> Option<Preset> {
        let n = name.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|p| p.name() == n)
    }

    /// The next preset in [`Self::ALL`], wrapping — used by the picker.
    pub fn next(self) -> Preset {
        let i = Self::ALL.iter().position(|&p| p == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// The previous preset in [`Self::ALL`], wrapping.
    pub fn prev(self) -> Preset {
        let i = Self::ALL.iter().position(|&p| p == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    /// The raw base colors of this preset.
    pub fn palette(self) -> Palette {
        let h = parse_hex;
        match self {
            Preset::CatppuccinMocha => Palette {
                base: h("#1e1e2e"),
                surface: h("#313244"),
                overlay: h("#6c7086"),
                muted: h("#45475a"),
                text: h("#cdd6f4"),
                accent: h("#cba6f7"),
                red: h("#f38ba8"),
                green: h("#a6e3a1"),
                yellow: h("#f9e2af"),
                blue: h("#89b4fa"),
                magenta: h("#f5c2e7"),
                cyan: h("#94e2d5"),
                orange: h("#fab387"),
            },
            Preset::Dracula => Palette {
                base: h("#282a36"),
                surface: h("#44475a"),
                overlay: h("#6272a4"),
                muted: h("#3a3c4e"),
                text: h("#f8f8f2"),
                accent: h("#bd93f9"),
                red: h("#ff5555"),
                green: h("#50fa7b"),
                yellow: h("#f1fa8c"),
                blue: h("#8be9fd"),
                magenta: h("#ff79c6"),
                cyan: h("#8be9fd"),
                orange: h("#ffb86c"),
            },
            Preset::Nord => Palette {
                base: h("#2e3440"),
                surface: h("#3b4252"),
                overlay: h("#4c566a"),
                muted: h("#434c5e"),
                text: h("#d8dee9"),
                accent: h("#88c0d0"),
                red: h("#bf616a"),
                green: h("#a3be8c"),
                yellow: h("#ebcb8b"),
                blue: h("#81a1c1"),
                magenta: h("#b48ead"),
                cyan: h("#8fbcbb"),
                orange: h("#d08770"),
            },
            Preset::CatppuccinLatte => Palette {
                base: h("#eff1f5"),
                surface: h("#ccd0da"),
                overlay: h("#9ca0b0"),
                muted: h("#bcc0cc"),
                text: h("#4c4f69"),
                accent: h("#8839ef"),
                red: h("#d20f39"),
                green: h("#40a02b"),
                yellow: h("#df8e1d"),
                blue: h("#1e66f5"),
                magenta: h("#ea76cb"),
                cyan: h("#179299"),
                orange: h("#fe640b"),
            },
        }
    }
}

impl Theme {
    /// Builds a full theme from a base [`Palette`]. The core fields map
    /// identically across the three TUIs; the bytewarden-specific fields
    /// (the splash starfield + the per-item-type accent colors) are
    /// derived from the palette roles so every preset gets a coherent
    /// set for free.
    pub fn from_palette(p: &Palette) -> Theme {
        Theme {
            accent: p.accent,
            inactive: p.overlay,
            selected_bg: p.surface,
            success: p.green,
            error: p.red,
            dim: p.overlay,
            foreground: p.text,
            placeholder: p.overlay,
            muted: p.muted,
            // Starfield: a fade from the background up toward the accent.
            star_dim: mix(p.accent, p.base, 0.78),
            star_mid: mix(p.accent, p.base, 0.45),
            star_bright: mix(p.accent, p.text, 0.25),
            item_login: p.blue,
            item_card: p.magenta,
            item_identity: p.yellow,
            // Teal, kept distinct from the green `success` color.
            item_note: p.cyan,
            item_ssh: p.accent,
            item_favorite: p.orange,
        }
    }
}

/// Decomposes a `Color` into RGB, treating non-RGB colors as black.
fn rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

/// Linearly blends `a` toward `b` by `t` (0.0 = all `a`, 1.0 = all `b`).
/// Used to derive the starfield tints from palette roles.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let (ar, ag, ab) = rgb(a);
    let (br, bg, bb) = rgb(b);
    let f = |x: u8, y: u8| (x as f32 * (1.0 - t) + y as f32 * t).round() as u8;
    Color::Rgb(f(ar, br), f(ag, bg), f(ab, bb))
}

impl Default for Theme {
    fn default() -> Self {
        // The shared default across the three TUIs is Catppuccin Mocha,
        // but `foreground` stays `Reset` so text inherits the terminal
        // until the user opts into a full preset (via `name = …` or the
        // in-app picker).
        let mut t = Theme::from_palette(&Preset::DEFAULT.palette());
        t.foreground = Color::Reset;
        t
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

/// Returns the [`Preset`] named in the `[theme]` section of
/// `<config_dir>/config.toml`, if it resolves. Used to preselect the
/// in-app theme picker on the Settings screen.
pub fn configured_preset(config_dir: &Path) -> Option<Preset> {
    let text = std::fs::read_to_string(config_dir.join("config.toml")).ok()?;
    theme_name(&text).as_deref().and_then(Preset::from_name)
}

/// Extracts the raw `name = "<preset>"` value from the `[theme]`
/// section, if present. [`Preset::from_name`] validates it.
fn theme_name(text: &str) -> Option<String> {
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
        if key.trim() != "name" {
            continue;
        }
        let rest = rest.trim();
        let val = if rest.starts_with('"') {
            rest.trim_start_matches('"').split('"').next().unwrap_or("")
        } else {
            rest.split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
        };
        if !val.is_empty() {
            return Some(val.trim().to_string());
        }
    }
    None
}

/// Parses the `[theme]` section: a `name = "<preset>"` picks the base
/// palette, then individual color keys override it.
fn parse_theme_section(text: &str) -> Theme {
    // `name` picks the base palette; per-key hex entries below override
    // it. Two passes so an override wins regardless of line order.
    let mut t = match theme_name(text).as_deref().and_then(Preset::from_name) {
        Some(p) => Theme::from_palette(&p.palette()),
        None => Theme::default(),
    };
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

    // ── Named presets ───────────────────────────────────────────

    #[test]
    fn named_preset_sets_the_base_palette() {
        let t = parse_theme_section("[theme]\nname = \"dracula\"\n");
        assert_eq!(t.accent, parse_hex("#bd93f9"));
        assert_eq!(t.error, parse_hex("#ff5555"));
        // A preset sets an explicit foreground (unlike the bare default).
        assert_eq!(t.foreground, parse_hex("#f8f8f2"));
    }

    #[test]
    fn preset_name_is_case_insensitive_and_unquoted() {
        let t = parse_theme_section("[theme]\nname = NORD\n");
        assert_eq!(t.accent, parse_hex("#88c0d0"));
    }

    #[test]
    fn explicit_keys_override_the_preset() {
        // Override wins even though `name` is declared last.
        let toml = "[theme]\naccent = \"#000000\"\nname = \"dracula\"\n";
        let t = parse_theme_section(toml);
        assert_eq!(t.accent, Color::Rgb(0, 0, 0));
        assert_eq!(t.error, parse_hex("#ff5555"));
    }

    #[test]
    fn unknown_preset_name_falls_back_to_default() {
        let t = parse_theme_section("[theme]\nname = \"solarized-zorp\"\n");
        assert_eq!(t.accent, Theme::default().accent);
        assert_eq!(t.foreground, Color::Reset);
    }

    #[test]
    fn every_preset_resolves_and_round_trips_its_name() {
        for p in Preset::ALL {
            assert_eq!(Preset::from_name(p.name()), Some(p));
            let t = Theme::from_palette(&p.palette());
            assert_ne!(t.accent, Color::Reset);
            // item_note (teal) must stay distinct from success (green).
            assert_ne!(t.item_note, t.success);
        }
    }

    #[test]
    fn preset_next_prev_wrap() {
        assert_eq!(Preset::CatppuccinMocha.prev(), Preset::CatppuccinLatte);
        assert_eq!(Preset::CatppuccinLatte.next(), Preset::CatppuccinMocha);
    }
}
