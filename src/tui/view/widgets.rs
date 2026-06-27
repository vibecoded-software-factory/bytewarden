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

/// Rounded-border [`Block`] with the supplied border style.
pub fn rounded_block(border_style: Style) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
}

/// Rounded block with a top-left title and a dim bottom-right counter.
pub fn titled_block(title: &str, bottom: &str, col: Color, t: &Theme) -> Block<'static> {
    // Match jewel/secretbase: square borders (not rounded) and a
    // bold title when the panel is focused. `col` is the focus color the
    // caller already resolved (accent when focused, else inactive), so a
    // `col == accent` test recovers the focused state for the bold.
    let mut title_style = Style::default().fg(col);
    if col == t.accent {
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
    // Match jewel/secretbase's `draw_status_strip`: a dim hint on the
    // left and an accent-bold affordance anchored to the right edge. The
    // anchor always wins the space contest so the user can always
    // discover the help / settings shortcuts; the hint degrades
    // full → short → truncated to fit whatever is left.
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(t.muted));
    let inner = block.inner(bar);
    frame.render_widget(block, bar);

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
