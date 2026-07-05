//! Shared widget builders used across screens.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::tui::theme::Theme;

/// Returns the accent color when focused, the inactive color otherwise.
pub fn focus_color(focused: bool, accent: Color, inactive: Color) -> Color {
    if focused { accent } else { inactive }
}

/// Returns a border [`Style`] using the accent color when focused.
pub fn focus_border(focused: bool, accent: Color) -> Style {
    if focused {
        Style::default().fg(accent)
    } else {
        Style::default()
    }
}

/// The single focused-vs-unfocused chrome [`Style`]: accent + bold when
/// focused, the `inactive` tint otherwise. Panels resolve "what focus
/// looks like" through this rather than assembling it inline.
pub fn focus_style(t: &Theme, focused: bool) -> Style {
    if focused {
        t.emphasis()
    } else {
        Style::default().fg(t.inactive)
    }
}

/// The keybind-letter [`Style`] (accent + bold) — every shortcut glyph
/// in the help popup, footer hints and legends reads the same through
/// this.
pub fn key_style(t: &Theme) -> Style {
    t.emphasis()
}

/// Rounded-border [`Block`] with the supplied border style.
pub fn rounded_block(border_style: Style) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
}

/// Square-bordered section [`Block`] with a top-left title and a dim
/// bottom-right counter. Focused → accent border + **bold** title;
/// otherwise the `inactive` tint. `focused` is passed explicitly so the
/// widget never has to reverse-engineer it. (Section panels are square;
/// popups / field cards use the rounded [`rounded_block`].)
pub fn titled_block(title: &str, bottom: &str, focused: bool, t: &Theme) -> Block<'static> {
    let col = if focused { t.accent } else { t.inactive };
    let mut title_style = Style::default().fg(col);
    if focused {
        title_style = title_style.add_modifier(Modifier::BOLD);
    }
    Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title.to_string(), title_style))
        .title_bottom(
            Line::from(Span::styled(bottom.to_string(), Style::default().fg(t.dim)))
                .right_aligned(),
        )
        .border_style(Style::default().fg(col))
}

/// Renders the bottom command-bar with graceful truncation.
///
/// `full` is shown when there is enough horizontal space, otherwise the
/// `short` form is used; if `short` itself is too long it is truncated.
///
/// This variant is used by popups (which have their own self-contained
/// instructions and no F1-help affordance).
pub fn render_cmd_bar(
    frame: &mut Frame,
    area: Rect,
    bar: Rect,
    full: &str,
    short: &str,
    col: Color,
    t: &Theme,
) {
    render_cmd_bar_inner(frame, area, bar, full, short, col, t, None);
}

/// Like [`render_cmd_bar`] but anchors `F1: help` at the right edge of
/// the bar. The anchor survives any truncation: if neither the long
/// nor the short hint string fits alongside the anchor, the hints are
/// dropped entirely so the user always sees they can press F1.
///
/// Use this for the main screens (Login, Vault, Detail, Create) where
/// F1 is a meaningful global shortcut.
pub fn render_cmd_bar_with_help(
    frame: &mut Frame,
    area: Rect,
    bar: Rect,
    full: &str,
    short: &str,
    col: Color,
    t: &Theme,
) {
    render_cmd_bar_inner(
        frame,
        area,
        bar,
        full,
        short,
        col,
        t,
        Some("F1 help · F9 settings"),
    );
}

/// Internal — picks the longest hint string that fits next to the
/// optional always-visible `anchor`, then renders the bar.
#[allow(clippy::too_many_arguments)]
fn render_cmd_bar_inner(
    frame: &mut Frame,
    area: Rect,
    bar: Rect,
    full: &str,
    short: &str,
    col: Color,
    t: &Theme,
    anchor: Option<&str>,
) {
    let _ = area; // budget is computed from the footer rect itself
    // A borderless bottom strip: a dim hint on the left and an
    // accent-bold affordance anchored to the right edge — no top rule
    // above it. The anchor always wins the space contest so the user can
    // always discover the help / settings shortcuts; the hint degrades
    // full → short → truncated to fit whatever is left.
    let inner = bar;
    let suffix = anchor.unwrap_or("");
    let total = inner.width as usize;
    // +2 gap before the anchor, +1 for the leading space on the hint.
    let suffix_block = if suffix.is_empty() {
        0
    } else {
        suffix.chars().count() + 2
    };
    let hints_avail = total.saturating_sub(suffix_block + 1);

    let hints: &str = if full.chars().count() <= hints_avail {
        full
    } else if short.chars().count() <= hints_avail {
        short
    } else if hints_avail == 0 {
        ""
    } else {
        // Truncate `short` on a char boundary to avoid breaking UTF-8.
        let mut idx = hints_avail.min(short.len());
        while !short.is_char_boundary(idx) && idx > 0 {
            idx -= 1;
        }
        &short[..idx]
    };

    if !hints.is_empty() {
        frame.render_widget(
            Paragraph::new(format!(" {hints}")).style(Style::default().fg(col)),
            inner,
        );
    }
    if !suffix.is_empty() {
        frame.render_widget(
            Paragraph::new(
                Line::from(Span::styled(
                    suffix,
                    Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                ))
                .right_aligned(),
            ),
            inner,
        );
    }
}

/// Builds a `Line` showing a text input with a block cursor at
/// `cursor_pos` (a character index, not a byte offset). When `focused`
/// is `false` the cursor is omitted.
pub fn input_with_cursor<'a>(
    text: &'a str,
    cursor_pos: usize,
    focused: bool,
    t: &Theme,
) -> Line<'a> {
    if !focused {
        return Line::from(Span::raw(text));
    }
    let chars: Vec<char> = text.chars().collect();
    let before: String = chars[..cursor_pos.min(chars.len())].iter().collect();
    let after: String = chars[cursor_pos.min(chars.len())..].iter().collect();
    Line::from(vec![
        Span::raw(before),
        Span::styled("█", Style::default().fg(t.accent)),
        Span::styled(after, Style::default().fg(t.foreground)),
    ])
}

/// Like [`input_with_cursor`] but always renders the cursor.
pub fn cursor_line(display: &str, cursor: usize, t: &Theme) -> Line<'static> {
    let chars: Vec<char> = display.chars().collect();
    let pos = cursor.min(chars.len());
    let before: String = chars[..pos].iter().collect();
    let after: String = chars[pos..].iter().collect();
    Line::from(vec![
        Span::raw(before),
        Span::styled("█", Style::default().fg(t.accent)),
        Span::styled(after, Style::default().fg(t.foreground)),
    ])
}

/// Renders a [`LineEditor`] as a one-line span with the block cursor —
/// the single popup text-input renderer. Thin wrapper over
/// [`cursor_line`] so callers pass the editor, not a `(text, cursor)`
/// pair.
pub fn editor_line(editor: &crate::domain::LineEditor, t: &Theme) -> Line<'static> {
    cursor_line(editor.text(), editor.cursor(), t)
}

/// Renders a labelled checkbox (☐ / ☑).
pub fn render_checkbox(
    frame: &mut Frame,
    label: &str,
    checked: bool,
    focused: bool,
    accent: Color,
    inactive: Color,
    area: Rect,
) {
    let icon = if checked { "☑" } else { "☐" };
    let icol = if checked { accent } else { inactive };
    let lcol = if focused { accent } else { inactive };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(icon, Style::default().fg(icol)),
            Span::styled(format!(" {label}"), Style::default().fg(lcol)),
        ])),
        area,
    );
}

/// Builds a vertical layout of `count` 4-row slots (1 label + 3 box)
/// for the detail/edit/create field cards.
pub fn field_areas(count: usize, area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::vertical(
        (0..count)
            .map(|_| Constraint::Length(4))
            .collect::<Vec<_>>(),
    )
    .split(area)
}

/// Renders a single labelled field card (1-row label + 3-row bordered
/// value box).
pub fn render_field_card(
    frame: &mut Frame,
    label: &str,
    hint: &str,
    vline: Line,
    bcol: Color,
    area: Rect,
    t: &Theme,
) {
    let fc = Layout::vertical([Constraint::Length(1), Constraint::Length(3)]).split(area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {label}"), Style::default().fg(bcol)),
            Span::styled(hint, Style::default().fg(t.dim)),
        ])),
        fc[0],
    );
    frame.render_widget(
        Paragraph::new(vline).block(rounded_block(Style::default().fg(bcol))),
        fc[1],
    );
}

/// Returns a sub-rectangle centered horizontally and vertically inside
/// `area`. `width_pct` is a percentage (0–100), `height` is in rows.
pub fn center_rect(width_pct: u16, height: u16, area: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - width_pct) / 2),
        Constraint::Percentage(width_pct),
        Constraint::Percentage((100 - width_pct) / 2),
    ])
    .split(v[1])[1]
}

/// Like [`center_rect`] but with the **height as a percentage** of
/// `area` too (both axes proportional) — the help popup's geometry.
pub fn center_rect_pct(width_pct: u16, height_pct: u16, area: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Percentage((100 - height_pct) / 2),
        Constraint::Percentage(height_pct),
        Constraint::Percentage((100 - height_pct) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - width_pct) / 2),
        Constraint::Percentage(width_pct),
        Constraint::Percentage((100 - width_pct) / 2),
    ])
    .split(v[1])[1]
}

/// One row of the help popup (key + description).
pub fn help_line<'a>(key: &'a str, desc: &'a str, t: &Theme) -> Line<'a> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(format!("{key:<14}"), Style::default().fg(t.accent)),
        Span::styled(desc, Style::default().fg(t.foreground)),
    ])
}

/// Modifier shorthand used in title strings.
pub fn bold() -> Modifier {
    Modifier::BOLD
}
