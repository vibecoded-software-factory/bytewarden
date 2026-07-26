//! Terminal glyph capability, and a fallback that keeps the interface
//! legible on a bare console font.
//!
//! The UI leans on a few decorative symbols a rich terminal font draws
//! cleanly — Nerd-font item icons, the search glass, the favourite star. A
//! bare framebuffer console (`TERM=linux`, `dumb`, or unset) renders through
//! a tiny fixed font that lacks most of them, so those cells draw as tofu.
//!
//! This is the glyph twin of [`ColorCaps`](super::theme::ColorCaps): detect
//! the terminal's capability once, then adapt. Two mechanisms cooperate:
//!
//! - **Selection.** On the [`Console`](GlyphCaps::Console) tier the icon set
//!   is forced to Unicode regardless of the `icon_style` setting (see
//!   [`super::view::icons::resolve_icons`]), so a bare console never emits a
//!   Nerd private-use glyph in the first place.
//! - **Sanitising.** [`sanitize_buffer`] is a belt over the finished frame:
//!   any leftover private-use-area glyph (where Nerd fonts live) degrades to
//!   a one-column `?`. Its scope is deliberately PUA-only — the other
//!   decorative symbols the app already ships (box drawing, bullets, the
//!   star) are left to the console font rather than mapped from an
//!   unverified table.

use ratatui::buffer::Buffer;

/// Whether the terminal can draw the UI's decorative glyphs, or needs the
/// console-safe fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphCaps {
    /// Rich font: draw every symbol as authored.
    Full,
    /// Bare console font: force the Unicode icon set and sanitise stray
    /// private-use glyphs.
    Console,
}

impl GlyphCaps {
    /// Detects the capability from the environment.
    ///
    /// `BYTEWARDEN_GLYPHS` forces the tier explicitly (`console` — or its
    /// alias `ascii` — versus `full`) for the cases detection cannot see: a
    /// rich font loaded into the console, or a multiplexer that hides the
    /// real terminal. Otherwise `TERM` decides: the framebuffer console
    /// (`linux`), a `dumb` terminal, or an unset `TERM` take the fallback;
    /// everything else keeps the full set.
    pub fn detect() -> GlyphCaps {
        let over = std::env::var("BYTEWARDEN_GLYPHS").ok();
        let term = std::env::var("TERM").unwrap_or_default();
        classify(&term, over.as_deref())
    }
}

/// The pure decision behind [`GlyphCaps::detect`], split out so the
/// precedence (explicit override, then terminal type) is unit-testable
/// without touching the process environment.
fn classify(term: &str, override_var: Option<&str>) -> GlyphCaps {
    match override_var.map(str::trim) {
        Some("console") | Some("ascii") => return GlyphCaps::Console,
        Some("full") => return GlyphCaps::Full,
        _ => {}
    }
    if term.is_empty() || term == "linux" || term == "dumb" {
        GlyphCaps::Console
    } else {
        GlyphCaps::Full
    }
}

/// Rewrites every private-use-area glyph to a one-column `?`, leaving
/// everything else untouched. Applied to the finished frame on the
/// [`Console`](GlyphCaps::Console) tier so a stray Nerd icon can never tofu.
///
/// The replacement is one column wide, matching every PUA icon the app
/// draws, so it never shifts a cell or disturbs the layout.
pub fn sanitize_buffer(buf: &mut Buffer) {
    for cell in &mut buf.content {
        let symbol = cell.symbol();
        let mut chars = symbol.chars();
        if let (Some(c), None) = (chars.next(), chars.next())
            && is_private_use(c)
        {
            cell.set_symbol("?");
        }
    }
}

/// Whether `c` sits in one of Unicode's private-use blocks — where Nerd
/// fonts place their glyphs and where no bare-console font has coverage.
fn is_private_use(c: char) -> bool {
    matches!(c as u32, 0xE000..=0xF8FF | 0xF_0000..=0xF_FFFD | 0x10_0000..=0x10_FFFD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn override_wins_over_terminal_type() {
        assert_eq!(
            classify("xterm-256color", Some("console")),
            GlyphCaps::Console
        );
        assert_eq!(
            classify("xterm-256color", Some("ascii")),
            GlyphCaps::Console
        );
        assert_eq!(classify("linux", Some("full")), GlyphCaps::Full);
        assert_eq!(classify("linux", Some(" console ")), GlyphCaps::Console);
    }

    #[test]
    fn bare_consoles_get_the_fallback_rich_terminals_keep_full() {
        assert_eq!(classify("linux", None), GlyphCaps::Console);
        assert_eq!(classify("dumb", None), GlyphCaps::Console);
        assert_eq!(classify("", None), GlyphCaps::Console);
        assert_eq!(classify("xterm-256color", None), GlyphCaps::Full);
    }

    #[test]
    fn unknown_override_falls_through_to_terminal_type() {
        assert_eq!(classify("linux", Some("garbage")), GlyphCaps::Console);
        assert_eq!(classify("xterm", Some("garbage")), GlyphCaps::Full);
    }

    #[test]
    fn sanitize_rewrites_only_private_use_glyphs() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
        buf[(0, 0)].set_symbol("\u{f0349}"); // Nerd search -> degraded
        buf[(1, 0)].set_symbol("a"); // ascii   -> untouched
        buf[(2, 0)].set_symbol("★"); // BMP star -> untouched
        buf[(3, 0)].set_symbol("│"); // box draw -> untouched
        buf[(4, 0)].set_symbol("◆"); // Unicode icon -> untouched
        sanitize_buffer(&mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "?");
        assert_eq!(buf[(1, 0)].symbol(), "a");
        assert_eq!(buf[(2, 0)].symbol(), "★");
        assert_eq!(buf[(3, 0)].symbol(), "│");
        assert_eq!(buf[(4, 0)].symbol(), "◆");
    }
}
