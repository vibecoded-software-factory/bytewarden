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

use ratatui::style::{Color, Modifier, Style};

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
/// The core roles map straight onto the shared `Theme` fields;
/// [`Theme::from_palette`] maps the remaining roles to bytewarden's own
/// domain colors (the splash starfield + per-item-type accents), so every
/// preset gets a coherent set for free.
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

/// A bundled, named theme. The default is [`Preset::DEFAULT`] (Nord).
/// Selected via `name = "<preset>"` in the `[theme]` section of
/// `config.toml`, or live from the in-app theme picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    CatppuccinMocha,
    CatppuccinMacchiato,
    CatppuccinFrappe,
    Dracula,
    Nord,
    TokyoNight,
    TokyoNightStorm,
    GruvboxDark,
    RosePine,
    Everforest,
    Kanagawa,
    OneDark,
    SolarizedDark,
    MonokaiPro,
    CatppuccinLatte,
    RosePineDawn,
    SolarizedLight,
}

impl Preset {
    /// Every bundled preset, in picker order (dark first, light last).
    pub const ALL: [Preset; 17] = [
        Preset::CatppuccinMocha,
        Preset::CatppuccinMacchiato,
        Preset::CatppuccinFrappe,
        Preset::Dracula,
        Preset::Nord,
        Preset::TokyoNight,
        Preset::TokyoNightStorm,
        Preset::GruvboxDark,
        Preset::RosePine,
        Preset::Everforest,
        Preset::Kanagawa,
        Preset::OneDark,
        Preset::SolarizedDark,
        Preset::MonokaiPro,
        Preset::CatppuccinLatte,
        Preset::RosePineDawn,
        Preset::SolarizedLight,
    ];

    /// The default preset — used when the config names no preset.
    pub const DEFAULT: Preset = Preset::Nord;

    /// The stable config key (lower-kebab) written to `config.toml`.
    pub fn name(self) -> &'static str {
        match self {
            Preset::CatppuccinMocha => "catppuccin-mocha",
            Preset::CatppuccinMacchiato => "catppuccin-macchiato",
            Preset::CatppuccinFrappe => "catppuccin-frappe",
            Preset::Dracula => "dracula",
            Preset::Nord => "nord",
            Preset::TokyoNight => "tokyonight",
            Preset::TokyoNightStorm => "tokyonight-storm",
            Preset::GruvboxDark => "gruvbox-dark",
            Preset::RosePine => "rose-pine",
            Preset::Everforest => "everforest",
            Preset::Kanagawa => "kanagawa",
            Preset::OneDark => "one-dark",
            Preset::SolarizedDark => "solarized-dark",
            Preset::MonokaiPro => "monokai-pro",
            Preset::CatppuccinLatte => "catppuccin-latte",
            Preset::RosePineDawn => "rose-pine-dawn",
            Preset::SolarizedLight => "solarized-light",
        }
    }

    /// The human-readable label shown in the picker.
    pub fn label(self) -> &'static str {
        match self {
            Preset::CatppuccinMocha => "Catppuccin Mocha",
            Preset::CatppuccinMacchiato => "Catppuccin Macchiato",
            Preset::CatppuccinFrappe => "Catppuccin Frappé",
            Preset::Dracula => "Dracula",
            Preset::Nord => "Nord",
            Preset::TokyoNight => "Tokyo Night",
            Preset::TokyoNightStorm => "Tokyo Night Storm",
            Preset::GruvboxDark => "Gruvbox Dark",
            Preset::RosePine => "Rosé Pine",
            Preset::Everforest => "Everforest",
            Preset::Kanagawa => "Kanagawa",
            Preset::OneDark => "One Dark",
            Preset::SolarizedDark => "Solarized Dark",
            Preset::MonokaiPro => "Monokai Pro",
            Preset::CatppuccinLatte => "Catppuccin Latte (light)",
            Preset::RosePineDawn => "Rosé Pine Dawn (light)",
            Preset::SolarizedLight => "Solarized Light (light)",
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
            Preset::CatppuccinMacchiato => Palette {
                base: h("#24273a"),
                surface: h("#363a4f"),
                overlay: h("#6e738d"),
                muted: h("#494d64"),
                text: h("#cad3f5"),
                accent: h("#c6a0f6"),
                red: h("#ed8796"),
                green: h("#a6da95"),
                yellow: h("#eed49f"),
                blue: h("#8aadf4"),
                magenta: h("#f5bde6"),
                cyan: h("#8bd5ca"),
                orange: h("#f5a97f"),
            },
            Preset::CatppuccinFrappe => Palette {
                base: h("#303446"),
                surface: h("#414559"),
                overlay: h("#737994"),
                muted: h("#51576d"),
                text: h("#c6d0f5"),
                accent: h("#ca9ee6"),
                red: h("#e78284"),
                green: h("#a6d189"),
                yellow: h("#e5c890"),
                blue: h("#8caaee"),
                magenta: h("#f4b8e4"),
                cyan: h("#81c8be"),
                orange: h("#ef9f76"),
            },
            Preset::TokyoNight => Palette {
                base: h("#1a1b26"),
                surface: h("#24283b"),
                overlay: h("#565f89"),
                muted: h("#414868"),
                text: h("#c0caf5"),
                accent: h("#7aa2f7"),
                red: h("#f7768e"),
                green: h("#9ece6a"),
                yellow: h("#e0af68"),
                blue: h("#7aa2f7"),
                magenta: h("#bb9af7"),
                cyan: h("#7dcfff"),
                orange: h("#ff9e64"),
            },
            Preset::TokyoNightStorm => Palette {
                base: h("#24283b"),
                surface: h("#2f344d"),
                overlay: h("#565f89"),
                muted: h("#3b4261"),
                text: h("#c0caf5"),
                accent: h("#7aa2f7"),
                red: h("#f7768e"),
                green: h("#9ece6a"),
                yellow: h("#e0af68"),
                blue: h("#7aa2f7"),
                magenta: h("#bb9af7"),
                cyan: h("#7dcfff"),
                orange: h("#ff9e64"),
            },
            Preset::GruvboxDark => Palette {
                base: h("#282828"),
                surface: h("#3c3836"),
                overlay: h("#928374"),
                muted: h("#504945"),
                text: h("#ebdbb2"),
                accent: h("#83a598"),
                red: h("#fb4934"),
                green: h("#b8bb26"),
                yellow: h("#fabd2f"),
                blue: h("#83a598"),
                magenta: h("#d3869b"),
                cyan: h("#8ec07c"),
                orange: h("#fe8019"),
            },
            Preset::RosePine => Palette {
                base: h("#191724"),
                surface: h("#1f1d2e"),
                overlay: h("#6e6a86"),
                muted: h("#26233a"),
                text: h("#e0def4"),
                accent: h("#c4a7e7"),
                red: h("#eb6f92"),
                green: h("#31748f"),
                yellow: h("#f6c177"),
                blue: h("#9ccfd8"),
                magenta: h("#c4a7e7"),
                cyan: h("#9ccfd8"),
                orange: h("#ebbcba"),
            },
            Preset::Everforest => Palette {
                base: h("#2d353b"),
                surface: h("#343f44"),
                overlay: h("#859289"),
                muted: h("#3d484d"),
                text: h("#d3c6aa"),
                accent: h("#a7c080"),
                red: h("#e67e80"),
                green: h("#a7c080"),
                yellow: h("#dbbc7f"),
                blue: h("#7fbbb3"),
                magenta: h("#d699b6"),
                cyan: h("#83c092"),
                orange: h("#e69875"),
            },
            Preset::Kanagawa => Palette {
                base: h("#1f1f28"),
                surface: h("#2a2a37"),
                overlay: h("#727169"),
                muted: h("#363646"),
                text: h("#dcd7ba"),
                accent: h("#7e9cd8"),
                red: h("#e46876"),
                green: h("#98bb6c"),
                yellow: h("#e6c384"),
                blue: h("#7e9cd8"),
                magenta: h("#957fb8"),
                cyan: h("#7aa89f"),
                orange: h("#ffa066"),
            },
            Preset::OneDark => Palette {
                base: h("#282c34"),
                surface: h("#2c313a"),
                overlay: h("#5c6370"),
                muted: h("#3e4451"),
                text: h("#abb2bf"),
                accent: h("#61afef"),
                red: h("#e06c75"),
                green: h("#98c379"),
                yellow: h("#e5c07b"),
                blue: h("#61afef"),
                magenta: h("#c678dd"),
                cyan: h("#56b6c2"),
                orange: h("#d19a66"),
            },
            Preset::SolarizedDark => Palette {
                base: h("#002b36"),
                surface: h("#073642"),
                overlay: h("#586e75"),
                muted: h("#094b56"),
                text: h("#839496"),
                accent: h("#268bd2"),
                red: h("#dc322f"),
                green: h("#859900"),
                yellow: h("#b58900"),
                blue: h("#268bd2"),
                magenta: h("#d33682"),
                cyan: h("#2aa198"),
                orange: h("#cb4b16"),
            },
            Preset::MonokaiPro => Palette {
                base: h("#2d2a2e"),
                surface: h("#403e41"),
                overlay: h("#727072"),
                muted: h("#38353a"),
                text: h("#fcfcfa"),
                accent: h("#ff6188"),
                red: h("#ff6188"),
                green: h("#a9dc76"),
                yellow: h("#ffd866"),
                blue: h("#78dce8"),
                magenta: h("#ab9df2"),
                cyan: h("#78dce8"),
                orange: h("#fc9867"),
            },
            Preset::RosePineDawn => Palette {
                base: h("#faf4ed"),
                surface: h("#fffaf3"),
                overlay: h("#9893a5"),
                muted: h("#f2e9e1"),
                text: h("#575279"),
                accent: h("#907aa9"),
                red: h("#b4637a"),
                green: h("#286983"),
                yellow: h("#ea9d34"),
                blue: h("#56949f"),
                magenta: h("#907aa9"),
                cyan: h("#56949f"),
                orange: h("#d7827e"),
            },
            Preset::SolarizedLight => Palette {
                base: h("#fdf6e3"),
                surface: h("#eee8d5"),
                overlay: h("#93a1a1"),
                muted: h("#e6dfc8"),
                text: h("#657b83"),
                accent: h("#268bd2"),
                red: h("#dc322f"),
                green: h("#859900"),
                yellow: h("#b58900"),
                blue: h("#268bd2"),
                magenta: h("#d33682"),
                cyan: h("#2aa198"),
                orange: h("#cb4b16"),
            },
        }
    }
}

impl Theme {
    /// Accent + bold — the emphasis / interaction style (focused panel
    /// titles, the `▶` cursor, keybind letters, the active identity).
    /// The one place that assembly lives, instead of inline everywhere.
    pub fn emphasis(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    /// Error + bold — destructive headlines and danger badges.
    pub fn danger_title(&self) -> Style {
        Style::default().fg(self.error).add_modifier(Modifier::BOLD)
    }

    /// Builds a full theme from a base [`Palette`]. The core roles map
    /// from the palette; the bytewarden-specific fields (the splash
    /// starfield + the per-item-type accent colors) are derived from the
    /// palette roles so every preset gets a coherent set for free.
    pub fn from_palette(p: &Palette) -> Theme {
        Theme {
            accent: p.accent,
            // Legibility hierarchy: `inactive` (unfocused borders) and
            // `dim` (readable secondary text — counters, hints, timestamps)
            // are lifted *out of the dark overlay band toward text* so
            // they stay legible, instead of being painted the near-border
            // gray. `muted` alone stays in the recessive band (chrome).
            inactive: mix(p.overlay, p.text, 0.6),
            selected_bg: p.surface,
            success: p.green,
            error: p.red,
            dim: mix(p.overlay, p.text, 0.5),
            foreground: p.text,
            placeholder: mix(p.overlay, p.text, 0.25),
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
        // Built from the default preset ([`Preset::DEFAULT`] = Nord), but
        // `foreground` stays `Reset` so text inherits the terminal until
        // the user opts into a full preset (via `name = …` or the in-app
        // picker).
        let mut t = Theme::from_palette(&Preset::DEFAULT.palette());
        t.foreground = Color::Reset;
        t
    }
}

/// Loads the theme from the `[theme]` section of `<config_dir>/config.toml`,
/// then **adapts it to the terminal's color capability** (see
/// [`ColorCaps`]) so a headless / low-color terminal gets a deterministic
/// downgrade instead of whatever the emulator would approximate.
///
/// Returns the (adapted) [`Theme::default`] when the file or section is
/// missing.
pub fn load(config_dir: &Path) -> Theme {
    adapt(load_unadapted(config_dir), ColorCaps::detect())
}

/// The theme exactly as configured, before terminal-capability
/// adaptation — kept separate so palette hex values stay exact for the
/// picker preview and the tests.
fn load_unadapted(config_dir: &Path) -> Theme {
    let file = config_dir.join("config.toml");
    match std::fs::read_to_string(&file) {
        Ok(text) => parse_theme_section(&text),
        Err(_) => Theme::default(),
    }
}

/// The terminal's color capability, detected once from the environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorCaps {
    /// `NO_COLOR` set — collapse every hue to a grayscale tier so meaning
    /// comes from brightness + bold/dim, never from a color the terminal
    /// won't show.
    Mono,
    /// No truecolor hint — quantize every RGB to the nearest xterm-256
    /// index deterministically (instead of leaving it to the emulator).
    Indexed256,
    /// `COLORTERM=truecolor|24bit` — pass RGB through untouched.
    True,
}

impl ColorCaps {
    /// Detects the capability from `NO_COLOR` / `COLORTERM`.
    pub fn detect() -> ColorCaps {
        if std::env::var("NO_COLOR").is_ok_and(|v| !v.is_empty()) {
            return ColorCaps::Mono;
        }
        match std::env::var("COLORTERM") {
            Ok(v) if v.contains("truecolor") || v.contains("24bit") => ColorCaps::True,
            _ => ColorCaps::Indexed256,
        }
    }
}

/// Adapts every color of `theme` to `caps`. Applied at *application*
/// time (boot + the live picker), never inside `from_palette`, so the
/// palette values stay exact. `Color::Reset` and named colors pass
/// through every mode (the inherit-terminal contract).
pub fn adapt(theme: Theme, caps: ColorCaps) -> Theme {
    match caps {
        ColorCaps::True => theme,
        ColorCaps::Indexed256 => map_colors(theme, quantize_256),
        ColorCaps::Mono => map_colors(theme, to_gray),
    }
}

/// Applies `f` to **every** color field of the theme. Listed explicitly
/// so a newly-added field can't silently skip adaptation.
fn map_colors(t: Theme, f: fn(Color) -> Color) -> Theme {
    Theme {
        accent: f(t.accent),
        inactive: f(t.inactive),
        selected_bg: f(t.selected_bg),
        success: f(t.success),
        error: f(t.error),
        dim: f(t.dim),
        foreground: f(t.foreground),
        placeholder: f(t.placeholder),
        muted: f(t.muted),
        star_dim: f(t.star_dim),
        star_mid: f(t.star_mid),
        star_bright: f(t.star_bright),
        item_login: f(t.item_login),
        item_card: f(t.item_card),
        item_identity: f(t.item_identity),
        item_note: f(t.item_note),
        item_ssh: f(t.item_ssh),
        item_favorite: f(t.item_favorite),
    }
}

/// NO_COLOR: luma-weighted brightness → one of four named gray tiers.
/// Non-RGB colors (`Reset`, named) pass through unchanged.
fn to_gray(c: Color) -> Color {
    let Color::Rgb(r, g, b) = c else {
        return c;
    };
    let luma = (2 * r as u32 + 3 * g as u32 + b as u32) / 6;
    match luma {
        0..=63 => Color::Black,
        64..=127 => Color::DarkGray,
        128..=191 => Color::Gray,
        _ => Color::White,
    }
}

/// Quantizes an RGB color to the nearest xterm-256 index — whichever of
/// the 6×6×6 color cube (16–231) or the grayscale ramp (232–255) is
/// closer in squared error. Non-RGB colors pass through unchanged.
fn quantize_256(c: Color) -> Color {
    let Color::Rgb(r, g, b) = c else {
        return c;
    };
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let nearest_cube = |v: u8| -> (usize, u8) {
        let mut best = (0usize, LEVELS[0]);
        let mut best_err = i32::MAX;
        for (i, &l) in LEVELS.iter().enumerate() {
            let e = (l as i32 - v as i32).abs();
            if e < best_err {
                best_err = e;
                best = (i, l);
            }
        }
        best
    };
    let (ri, rl) = nearest_cube(r);
    let (gi, gl) = nearest_cube(g);
    let (bi, bl) = nearest_cube(b);
    let cube_idx = 16 + 36 * ri + 6 * gi + bi;
    let sq = |a: u8, x: u8| (a as i32 - x as i32).pow(2);
    let cube_err = sq(rl, r) + sq(gl, g) + sq(bl, b);
    // Grayscale ramp: indices 232..=255 hold values 8, 18, …, 238.
    let avg = (r as i32 + g as i32 + b as i32) / 3;
    let gidx = (((avg - 8) as f32 / 10.0).round() as i32).clamp(0, 23);
    let gv = (8 + gidx * 10) as u8;
    let gray_err = sq(gv, r) + sq(gv, g) + sq(gv, b);
    if gray_err < cube_err {
        Color::Indexed((232 + gidx) as u8)
    } else {
        Color::Indexed(cube_idx as u8)
    }
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
    fn to_gray_maps_by_brightness_and_passes_non_rgb() {
        assert_eq!(to_gray(Color::Rgb(255, 255, 255)), Color::White);
        assert_eq!(to_gray(Color::Rgb(0, 0, 0)), Color::Black);
        assert_eq!(to_gray(Color::Rgb(160, 160, 160)), Color::Gray);
        assert_eq!(to_gray(Color::Rgb(90, 90, 90)), Color::DarkGray);
        // Reset / named colors are never touched (the inherit contract).
        assert_eq!(to_gray(Color::Reset), Color::Reset);
    }

    #[test]
    fn quantize_256_picks_ramp_for_grays_and_cube_for_hues() {
        // A neutral gray should land on the grayscale ramp (232..=255).
        match quantize_256(Color::Rgb(130, 130, 130)) {
            Color::Indexed(i) => assert!((232..=255).contains(&i), "expected ramp, got {i}"),
            other => panic!("expected Indexed, got {other:?}"),
        }
        // A saturated hue should land on the 6×6×6 cube (16..=231).
        match quantize_256(Color::Rgb(255, 0, 0)) {
            Color::Indexed(i) => assert!((16..=231).contains(&i), "expected cube, got {i}"),
            other => panic!("expected Indexed, got {other:?}"),
        }
        assert_eq!(quantize_256(Color::Reset), Color::Reset);
    }

    #[test]
    fn adapt_true_is_a_passthrough_and_reset_survives_every_mode() {
        let t = Theme::from_palette(&Preset::Nord.palette());
        assert_eq!(adapt(t.clone(), ColorCaps::True).accent, t.accent);
        // `foreground: Reset` must survive mono + indexed adaptation.
        let mut r = t.clone();
        r.foreground = Color::Reset;
        assert_eq!(adapt(r.clone(), ColorCaps::Mono).foreground, Color::Reset);
        assert_eq!(adapt(r, ColorCaps::Indexed256).foreground, Color::Reset);
    }

    #[test]
    fn emphasis_is_accent_bold_and_danger_is_error_bold() {
        let t = Theme {
            accent: Color::Rgb(1, 2, 3),
            error: Color::Rgb(9, 8, 7),
            ..Theme::default()
        };
        let e = t.emphasis();
        assert_eq!(e.fg, Some(Color::Rgb(1, 2, 3)));
        assert!(e.add_modifier.contains(Modifier::BOLD));
        let d = t.danger_title();
        assert_eq!(d.fg, Some(Color::Rgb(9, 8, 7)));
        assert!(d.add_modifier.contains(Modifier::BOLD));
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
    fn dim_and_inactive_are_lifted_out_of_the_overlay_band() {
        // The legibility hierarchy: readable secondary text (`dim`) and
        // unfocused borders (`inactive`) must NOT be the raw dark overlay —
        // they're blended toward `text` so they stay legible. Only `muted`
        // stays in the recessive band.
        for p in Preset::ALL {
            let t = Theme::from_palette(&p.palette());
            assert_ne!(t.dim, p.palette().overlay, "dim not lifted: {}", p.name());
            assert_ne!(
                t.inactive,
                p.palette().overlay,
                "inactive not lifted: {}",
                p.name()
            );
            assert_eq!(t.muted, p.palette().muted, "muted moved: {}", p.name());
        }
    }

    #[test]
    fn preset_next_prev_wrap() {
        // Wraps around the ends of `ALL` (first ↔ last).
        let first = Preset::ALL[0];
        let last = Preset::ALL[Preset::ALL.len() - 1];
        assert_eq!(first.prev(), last);
        assert_eq!(last.next(), first);
        // next/prev are inverses everywhere.
        for &p in Preset::ALL.iter() {
            assert_eq!(p.next().prev(), p);
        }
    }

    #[test]
    fn every_preset_round_trips_through_its_name() {
        for &p in Preset::ALL.iter() {
            assert_eq!(Preset::from_name(p.name()), Some(p), "name: {}", p.name());
        }
    }
}
