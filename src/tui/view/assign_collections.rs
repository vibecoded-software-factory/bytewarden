//! Multi-select popup view: pick the collections the item belongs to.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::tui::app::App;
use crate::tui::view::widgets::center_rect;

/// Renders the popup. No-op when no popup is in flight.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.assign_collections else {
        return;
    };
    let t = &app.theme;
    let popup = center_rect(60, 16, area);
    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .title(" Assign collections ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
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
        let mut ls = ListState::default();
        ls.select(Some(state.cursor));
        let list = List::new(items)
            .highlight_style(Style::default().bg(t.selected_bg))
            .highlight_symbol("▸ ");
        frame.render_stateful_widget(list, chunks[2], &mut ls);
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
