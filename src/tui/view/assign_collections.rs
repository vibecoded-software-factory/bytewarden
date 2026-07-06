//! Multi-select popup view: pick the collections the item belongs to.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::tui::app::App;
use crate::tui::view::widgets::{center_rect, draw_scrollbar};

thread_local! {
    /// Frame-local hit map for the collections list — one `(rect, index)`
    /// per visible row, recorded as the list draws (accounting for its
    /// scroll offset) so a click toggles the row the pointer is over.
    static COLLECTION_HITS: std::cell::RefCell<Vec<(Rect, usize)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn register_hit(rect: Rect, idx: usize) {
    if rect.width > 0 && rect.height > 0 {
        COLLECTION_HITS.with(|h| h.borrow_mut().push((rect, idx)));
    }
}

/// The collection-row index under `(column, row)`, if any — consumed by the
/// mouse layer to toggle that collection on a click.
pub fn collection_row_at(column: u16, row: u16) -> Option<usize> {
    COLLECTION_HITS.with(|h| {
        h.borrow()
            .iter()
            .rev()
            .find(|(r, _)| {
                column >= r.x && column < r.x + r.width && row >= r.y && row < r.y + r.height
            })
            .map(|(_, i)| *i)
    })
}

/// Renders the popup. No-op when no popup is in flight.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    COLLECTION_HITS.with(|h| h.borrow_mut().clear());
    let Some(state) = &app.assign_collections else {
        return;
    };
    let t = &app.theme;
    let popup = center_rect(60, 16, area);
    crate::tui::view::widgets::register_modal(popup);
    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .title(" Assign collections ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1), // padding
        ratatui::layout::Constraint::Length(1), // header
        ratatui::layout::Constraint::Min(0),    // list
        ratatui::layout::Constraint::Length(1), // feedback
        ratatui::layout::Constraint::Length(1), // hint
    ])
    .split(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            format!(
                " {} of {} selected",
                state.selected.len(),
                state.available.len()
            ),
            Style::default().fg(t.dim),
        )])),
        chunks[1],
    );

    if state.available.is_empty() {
        // Personal item or org with no visible collections — surface
        // the empty state instead of an empty list.
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                " No collections available for this organisation.",
                Style::default().fg(t.dim).add_modifier(Modifier::ITALIC),
            )])),
            chunks[2],
        );
    } else {
        let items: Vec<ListItem> = state
            .available
            .iter()
            .map(|c| {
                let mark = if state.selected.contains(&c.id) {
                    "[x]"
                } else {
                    "[ ]"
                };
                ListItem::new(Line::from(vec![Span::styled(
                    format!("  {mark}  {}", c.name),
                    Style::default().fg(t.foreground),
                )]))
            })
            .collect();
        // Reserve a one-column gutter for the scrollbar when the list
        // overflows, so the track never overwrites a collection name.
        let overflow = state.available.len() > chunks[2].height as usize;
        let list_area = if overflow {
            Rect {
                width: chunks[2].width.saturating_sub(1),
                ..chunks[2]
            }
        } else {
            chunks[2]
        };
        let mut ls = ListState::default();
        ls.select(Some(state.cursor));
        let list = List::new(items)
            .highlight_style(Style::default().bg(t.selected_bg))
            .highlight_symbol("▸ ");
        frame.render_stateful_widget(list, list_area, &mut ls);
        // Register each visible row from the list's realised scroll offset,
        // so a click maps to the right `available` index even when scrolled.
        let offset = ls.offset();
        for vis in 0..list_area.height {
            let idx = offset + vis as usize;
            if idx >= state.available.len() {
                break;
            }
            register_hit(
                Rect::new(list_area.x, list_area.y + vis, list_area.width, 1),
                idx,
            );
        }
        if overflow {
            draw_scrollbar(frame, chunks[2], state.available.len(), state.cursor, t);
        }
    }

    if state.error {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    " ✕ ",
                    Style::default().fg(t.error).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Pick at least one collection — org items can't be uncollected.",
                    Style::default().fg(t.error),
                ),
            ])),
            chunks[3],
        );
    }

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " j/k or ↑↓ to navigate · Space to toggle · Enter to apply · Esc to cancel",
            Style::default().fg(t.dim).add_modifier(Modifier::ITALIC),
        ))),
        chunks[4],
    );
}
