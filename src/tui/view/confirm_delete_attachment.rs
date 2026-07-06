//! Confirm-delete-attachment popup renderer.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::view::widgets::center_rect;

/// Renders the confirm-delete-attachment popup.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let (file_name, item_name) = match &app.attachment_delete {
        Some(s) => (s.file_name.as_str(), s.item_name.as_str()),
        None => ("this attachment", ""),
    };
    let popup = center_rect(58, 11, area);
    frame.render_widget(Clear, popup);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Delete attachment: ", Style::default().fg(t.inactive)),
            Span::styled(
                file_name,
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  From item: ", Style::default().fg(t.inactive)),
            Span::styled(item_name, Style::default().fg(t.foreground)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  This cannot be undone. The file is removed from the server.",
            Style::default().fg(t.dim),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Enter",
                Style::default().fg(t.error).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  Delete attachment", Style::default().fg(t.foreground)),
        ]),
        Line::from(vec![
            Span::styled("  Esc  ", Style::default().fg(t.dim)),
            Span::styled("  Cancel", Style::default().fg(t.dim)),
        ]),
        Line::from(""),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" Confirm Delete Attachment ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(t.error)),
        ),
        popup,
    );
}
