use ratatui::buffer::Buffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphCaps {
    Full,

    Console,
}

impl GlyphCaps {
    pub fn detect() -> GlyphCaps {
        let over = std::env::var("BYTEWARDEN_GLYPHS").ok();
        let term = std::env::var("TERM").unwrap_or_default();
        classify(&term, over.as_deref())
    }
}

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
        buf[(0, 0)].set_symbol("\u{f0349}");
        buf[(1, 0)].set_symbol("a");
        buf[(2, 0)].set_symbol("★");
        buf[(3, 0)].set_symbol("│");
        buf[(4, 0)].set_symbol("◆");
        sanitize_buffer(&mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "?");
        assert_eq!(buf[(1, 0)].symbol(), "a");
        assert_eq!(buf[(2, 0)].symbol(), "★");
        assert_eq!(buf[(3, 0)].symbol(), "│");
        assert_eq!(buf[(4, 0)].symbol(), "◆");
    }
}
