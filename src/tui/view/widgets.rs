//! Shared widget builders used across screens.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

use crate::tui::theme::Theme;

thread_local! {
    /// Frame-local **scroll registry**: every scrollable surface records its
    /// viewport rect + logical [`ScrollTarget`] here as it draws, so the mouse
    /// wheel dispatches by pointer position — one generic path, no per-screen
    /// `match` in the input layer. Cleared each frame by [`reset_scroll_regions`].
    static SCROLL_REGIONS: std::cell::RefCell<Vec<(Rect, ScrollTarget)>> =
        const { std::cell::RefCell::new(Vec::new()) };

    /// Frame-local rect of the **active centered overlay** (a popup / confirm /
    /// settings / help), if one is drawn. The mouse layer uses it for
    /// click-outside-to-dismiss — one generic close path for every overlay.
    static MODAL_RECT: std::cell::RefCell<Option<Rect>> = const { std::cell::RefCell::new(None) };

    /// Frame-local **clickable-button registry**: chrome that looks like a
    /// button (the `F1`/`F10` command-bar anchor, …) records its rect + a
    /// [`ClickAction`] here as it draws, so the mouse layer dispatches a click
    /// on it generically — the same pattern as the scroll registry.
    static BUTTONS: std::cell::RefCell<Vec<(Rect, ClickAction)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// A semantic action a rendered "button" triggers on click — the mouse twin of
/// its key. Registered by the widget that draws the button ([`register_button`])
/// and dispatched by the input layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickAction {
    /// Open the help overlay (the `F1 help` anchor).
    OpenHelp,
    /// Open the settings overlay (the `F10 settings` anchor).
    OpenSettings,
}

/// What the mouse wheel moves when it's over a registered region. The widget /
/// view that draws a scrollable surface tags it with one of these; the input
/// layer owns the single table that maps a tag to the state it scrolls. Adding
/// a new scrollable list is one `register_scroll` call — the wheel handler
/// never changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollTarget {
    /// The vault item list.
    Vault,
    /// The item-filter sidebar.
    Filters,
    /// The command-log panel.
    CmdLog,
    /// The item detail's field list.
    Detail,
    /// The help overlay (2-D scroll; `Shift`+wheel pans horizontally).
    Help,
}

/// Clears the frame-local registries (scroll regions, the active modal rect,
/// the button rects). Called once per frame before drawing, alongside the
/// `MouseAreas` reset.
pub fn reset_scroll_regions() {
    SCROLL_REGIONS.with(|s| s.borrow_mut().clear());
    MODAL_RECT.with(|m| *m.borrow_mut() = None);
    BUTTONS.with(|b| b.borrow_mut().clear());
}

/// Records a clickable button for this frame (rect + the action it triggers).
pub fn register_button(rect: Rect, action: ClickAction) {
    if rect.width > 0 && rect.height > 0 {
        BUTTONS.with(|b| b.borrow_mut().push((rect, action)));
    }
}

/// The action of the clickable button under `(column, row)`, if any — the
/// last-registered (top-most) match.
pub fn button_at(column: u16, row: u16) -> Option<ClickAction> {
    BUTTONS.with(|b| {
        b.borrow()
            .iter()
            .rev()
            .find(|(r, _)| {
                column >= r.x && column < r.x + r.width && row >= r.y && row < r.y + r.height
            })
            .map(|(_, a)| *a)
    })
}

/// Records the active centered-overlay rect for this frame. Every overlay drawer
/// calls this (right where it computes its `center_rect` popup) so the mouse
/// layer can dismiss the overlay on a click outside it.
pub fn register_modal(rect: Rect) {
    MODAL_RECT.with(|m| *m.borrow_mut() = Some(rect));
}

/// The active centered-overlay rect, if one is drawn this frame.
pub fn active_modal_rect() -> Option<Rect> {
    MODAL_RECT.with(|m| *m.borrow())
}

/// Records a scrollable region for this frame. Overlays draw after the base
/// screen, so later registrations win on overlap (a modal captures the wheel).
pub fn register_scroll(rect: Rect, target: ScrollTarget) {
    if rect.width > 0 && rect.height > 0 {
        SCROLL_REGIONS.with(|s| s.borrow_mut().push((rect, target)));
    }
}

/// The scroll target under `(column, row)`, if any — the last-registered
/// (top-most) region that contains the point.
pub fn scroll_target_at(column: u16, row: u16) -> Option<ScrollTarget> {
    SCROLL_REGIONS.with(|s| {
        s.borrow()
            .iter()
            .rev()
            .find(|(r, _)| {
                column >= r.x && column < r.x + r.width && row >= r.y && row < r.y + r.height
            })
            .map(|(_, t)| *t)
    })
}

/// Responsive command-log height (rows, borders included): 6 when the
/// terminal is roomy, shrinking to 3 at the floor so the body keeps its
/// rows. **Monotonic** — a taller terminal never shrinks the log (and so
/// never shrinks the body it sits above).
pub fn cmdlog_height(total: u16) -> u16 {
    match total {
        0..=19 => 3,
        20..=23 => 4,
        24..=29 => 5,
        _ => 6,
    }
}

/// Draws a dim vertical scrollbar on the right border of a bordered
/// `area` — **only when the content overflows** the visible rows, so
/// short lists stay clean. `content_len` is the total row count and
/// `selected` is the index of the **current (selected) row**.
///
/// Two subtleties Ratatui's `Scrollbar` forces:
/// - It only puts the thumb at the very bottom when `position ==
///   content_length - 1`, so we feed it the **selection index** (which
///   spans `0..=len-1`), not the top-of-viewport offset (which tops out
///   at `len - viewport` and would leave the thumb short of the end).
/// - Rendered over the full block rect it paints the rounded corners; we
///   inset the track by one row top and bottom so it sits *between* the
///   borders instead of overrunning them.
pub fn draw_scrollbar(
    frame: &mut Frame,
    area: Rect,
    content_len: usize,
    selected: usize,
    t: &Theme,
) {
    let viewport = area.height.saturating_sub(2) as usize; // inside the borders
    if viewport == 0 || content_len <= viewport {
        return;
    }
    let mut state = ScrollbarState::new(content_len)
        .viewport_content_length(viewport)
        .position(selected);
    let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .thumb_style(Style::default().fg(t.dim))
        .track_style(Style::default().fg(t.muted))
        .begin_symbol(None)
        .end_symbol(None);
    // Inset one row top/bottom so the track lands between the horizontal
    // borders and never overwrites the rounded corners.
    let track = area.inner(Margin {
        horizontal: 0,
        vertical: 1,
    });
    frame.render_stateful_widget(sb, track, &mut state);
}

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

/// Rounded-bordered section [`Block`] with a top-left title and a dim
/// bottom-right counter. Focused → accent border + **bold** title;
/// otherwise the `inactive` tint. `focused` is passed explicitly so the
/// widget never has to reverse-engineer it. Section panels share the
/// rounded chrome with popups / field cards ([`rounded_block`]).
pub fn titled_block(title: &str, bottom: &str, focused: bool, t: &Theme) -> Block<'static> {
    let col = if focused { t.accent } else { t.inactive };
    let mut title_style = Style::default().fg(col);
    if focused {
        title_style = title_style.add_modifier(Modifier::BOLD);
    }
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
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
        Some("F1 help · F10 settings"),
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
        // The anchor is clickable — the mouse twin of the function keys. It's
        // right-aligned in `inner`; split it into its two labels by char width
        // (the `·` separator is multi-byte, so count chars, not bytes).
        let anchor_len = suffix.chars().count() as u16;
        if inner.width >= anchor_len {
            let ax = inner.x + inner.width - anchor_len;
            let help_w = "F1 help".chars().count() as u16;
            let sep_w = " · ".chars().count() as u16;
            let set_w = "F10 settings".chars().count() as u16;
            register_button(
                Rect::new(ax, inner.y, help_w, 1),
                crate::tui::view::widgets::ClickAction::OpenHelp,
            );
            register_button(
                Rect::new(ax + help_w + sep_w, inner.y, set_w, 1),
                crate::tui::view::widgets::ClickAction::OpenSettings,
            );
        }
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

/// Number of 4-row field cards that fit in `area`.
pub fn field_card_capacity(area: Rect) -> usize {
    (area.height / 4).max(1) as usize
}

/// Lays out the field cards that fit in `area`, windowed so the `selected`
/// card is always visible. Returns the visible card rects paired with the
/// index of the first visible field, so the caller renders `fields[start..]`
/// and can map a clicked rect back to its real field index. When every card
/// fits, this is [`field_areas`] with `start == 0`.
pub fn field_areas_windowed(
    count: usize,
    selected: usize,
    area: Rect,
) -> (std::rc::Rc<[Rect]>, usize) {
    let cap = field_card_capacity(area);
    if count <= cap {
        return (field_areas(count, area), 0);
    }
    // Keep the selected card at or above the bottom edge until it would fall
    // off the top — the same windowing the preset picker uses.
    let start = selected
        .min(count - 1)
        .saturating_sub(cap - 1)
        .min(count - cap);
    (field_areas(cap, area), start)
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

#[cfg(test)]
mod tests {
    use super::{
        ScrollTarget, cmdlog_height, register_scroll, reset_scroll_regions, scroll_target_at,
    };
    use ratatui::layout::Rect;

    #[test]
    fn scroll_registry_dispatches_by_position_top_most_wins() {
        reset_scroll_regions();
        // A base region, then a smaller overlapping one registered later (as an
        // overlay would draw over the base).
        register_scroll(Rect::new(0, 0, 10, 5), ScrollTarget::Vault);
        register_scroll(Rect::new(2, 1, 4, 2), ScrollTarget::Help);
        // Only the base covers this point.
        assert_eq!(scroll_target_at(0, 0), Some(ScrollTarget::Vault));
        // Overlap → the later (top-most) registration wins.
        assert_eq!(scroll_target_at(3, 1), Some(ScrollTarget::Help));
        // Outside every region.
        assert_eq!(scroll_target_at(50, 50), None);
        // Empty rects are never registered.
        register_scroll(Rect::new(0, 0, 0, 5), ScrollTarget::CmdLog);
        assert_eq!(scroll_target_at(0, 0), Some(ScrollTarget::Vault));
        // Reset clears the registry.
        reset_scroll_regions();
        assert_eq!(scroll_target_at(0, 0), None);
    }

    #[test]
    fn cmdlog_height_is_bounded_and_monotonic() {
        // Bounded to [3, 6].
        for h in 0u16..80 {
            let r = cmdlog_height(h);
            assert!((3..=6).contains(&r), "out of range at {h}: {r}");
        }
        // Monotonic non-decreasing in terminal height (a taller terminal
        // never shrinks the log — and so never shrinks the body).
        for h in 1u16..80 {
            assert!(cmdlog_height(h) >= cmdlog_height(h - 1));
        }
        // The floor is 3 (short terminal), the ceiling 6 (roomy).
        assert_eq!(cmdlog_height(18), 3);
        assert_eq!(cmdlog_height(40), 6);
    }
}
