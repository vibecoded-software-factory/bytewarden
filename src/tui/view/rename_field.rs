//! Rename-custom-field popup renderer.
//!
//! Drawn over the detail edit-mode screen. Shows the row being renamed
//! and a single text input pre-filled with its current label.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::view::widgets::{center_rect, editor_spans, rounded_block};

/// Renders the rename popup. No-op when no rename is in flight.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.rename_field else {
        return;
    };
    let t = &app.theme;
    let popup = center_rect(60, 9, area);
    crate::tui::view::widgets::register_modal(popup);
    frame.render_widget(Clear, popup);

    // Outer block.
    let outer = Block::default()
        .title(" Rename field ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    // Inner vertical split: padding / label / input / hint.
    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Length(1),
    ])
    .split(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" New label", Style::default().fg(t.dim)),
            Span::styled(
                "  (Enter to apply, Esc to cancel)",
                Style::default().fg(t.dim),
            ),
        ])),
        chunks[1],
    );

    let line = if state.input.is_empty() {
        Line::from(vec![
            Span::styled("█", Style::default().fg(t.accent)),
            Span::styled("  type a new name", Style::default().fg(t.placeholder)),
        ])
    } else {
        Line::from(editor_spans(&state.input, true, t))
    };
    frame.render_widget(
        Paragraph::new(line).block(rounded_block(Style::default().fg(t.accent))),
        chunks[2],
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " The current value of the field is preserved.",
            Style::default().fg(t.dim).add_modifier(Modifier::ITALIC),
        ))),
        chunks[3],
    );
}
