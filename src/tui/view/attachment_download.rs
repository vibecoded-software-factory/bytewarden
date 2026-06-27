//! Attachment-download popup renderer.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::view::widgets::{center_rect, cursor_line, rounded_block};

/// Renders the attachment-download popup over the detail screen.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.attachment_download else {
        return;
    };
    let t = &app.theme;
    let popup = center_rect(70, 11, area);
    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .title(" Download attachment ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(t.accent));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Length(1),
    ])
    .split(inner);

    // Item / file header lines.
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
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" File: ", Style::default().fg(t.dim)),
            Span::styled(state.file_name.as_str(), Style::default().fg(t.foreground)),
        ])),
        chunks[2],
    );

    // Path label + input.
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Save to", Style::default().fg(t.dim)),
            Span::styled(
                "  (full destination path; existing files are overwritten by bw)",
                Style::default().fg(t.dim),
            ),
        ])),
        chunks[3],
    );
    let line = if state.path.is_empty() {
        Line::from(vec![
            Span::styled("█", Style::default().fg(t.accent)),
            Span::styled("  /path/to/save", Style::default().fg(t.placeholder)),
        ])
    } else {
        cursor_line(&state.path, state.path_cursor, t)
    };
    frame.render_widget(
        Paragraph::new(line).block(rounded_block(Style::default().fg(t.accent))),
        chunks[4],
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Enter download · Esc cancel",
            Style::default().fg(t.dim),
        ))),
        chunks[5],
    );
}
