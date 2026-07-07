//! Attachment-upload popup renderer.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::view::widgets::{center_rect, editor_spans, rounded_block};

/// Renders the attachment-upload popup over the detail screen.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.attachment_upload else {
        return;
    };
    let t = &app.theme;
    let popup = center_rect(70, 10, area);
    crate::tui::view::widgets::register_modal(popup);
    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .title(" Upload attachment ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Length(1),
    ])
    .split(inner);

    // Item header
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Item: ", Style::default().fg(t.dim)),
            Span::styled(
                state.item_name.as_str(),
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
            ),
        ])),
        chunks[1],
    );

    // Path label + input
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" File path", Style::default().fg(t.dim)),
            Span::styled(
                "  (full path to the file to upload)",
                Style::default().fg(t.dim),
            ),
        ])),
        chunks[2],
    );
    let line = if state.path.is_empty() {
        Line::from(vec![
            Span::styled("█", Style::default().fg(t.accent)),
            Span::styled("  /path/to/file.pdf", Style::default().fg(t.placeholder)),
        ])
    } else {
        Line::from(editor_spans(&state.path, true, t))
    };
    frame.render_widget(
        Paragraph::new(line).block(rounded_block(Style::default().fg(t.accent))),
        chunks[3],
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Enter upload · Esc cancel",
            Style::default().fg(t.dim),
        ))),
        chunks[4],
    );
}
