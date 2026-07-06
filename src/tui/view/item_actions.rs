//! Per-item action menu renderer (`Screen::ItemActions`).
//!
//! A compact centered menu of secondary actions, drawn over the vault by
//! right-clicking a row. Uses the app's own overlay chrome (a rounded
//! accent block + a `▶`-marked selection list + a dim hint line) and
//! records a per-row hit map so the rows are clickable — the same
//! frame-local pattern as the settings / detail hit maps.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::view::widgets::center_rect;

thread_local! {
    /// Frame-local hit map for the action menu — one `(rect, row)` per
    /// action, recorded as the menu draws.
    static ITEM_ACTION_HITS: std::cell::RefCell<Vec<(Rect, usize)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn register_hit(rect: Rect, idx: usize) {
    if rect.width > 0 && rect.height > 0 {
        ITEM_ACTION_HITS.with(|h| h.borrow_mut().push((rect, idx)));
    }
}

/// The action row under `(column, row)`, if any — consumed by the mouse
/// layer to run that action on a click.
pub fn item_action_at(column: u16, row: u16) -> Option<usize> {
    ITEM_ACTION_HITS.with(|h| {
        h.borrow()
            .iter()
            .rev()
            .find(|(r, _)| {
                column >= r.x && column < r.x + r.width && row >= r.y && row < r.y + r.height
            })
            .map(|(_, i)| *i)
    })
}

/// Renders the action menu. No-op when no menu is in flight.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    ITEM_ACTION_HITS.with(|h| h.borrow_mut().clear());
    let Some(state) = &app.item_actions else {
        return;
    };
    let t = &app.theme;

    // Size to the content: one row per action plus the border and the
    // hint line, capped so a long list never runs off a short terminal.
    let rows = state.actions.len() as u16;
    let height = (rows + 4).min(area.height.saturating_sub(2)).max(5);
    let popup = center_rect(28, height, area);
    crate::tui::view::widgets::register_modal(popup); // click outside closes it
    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .title(Span::styled(
            " Actions ",
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);

    let lines: Vec<Line> = state
        .actions
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let selected = i == state.cursor;
            let marker = if selected { "▶ " } else { "  " };
            let style = if selected {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.foreground)
            };
            Line::from(Span::styled(format!(" {marker}{}", a.label()), style))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), chunks[0]);
    // Each action row is clickable — row `i` renders at line `i` of the list.
    for i in 0..state.actions.len() {
        register_hit(
            Rect::new(chunks[0].x, chunks[0].y + i as u16, chunks[0].width, 1),
            i,
        );
    }

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " ↑/↓ pick · Enter do · Esc close",
            Style::default().fg(t.dim).add_modifier(Modifier::ITALIC),
        ))),
        chunks[1],
    );
}
