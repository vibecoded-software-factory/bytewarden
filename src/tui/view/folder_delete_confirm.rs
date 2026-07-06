//! Confirm-delete-folder popup renderer.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::flows::folders::focused_folder;
use crate::tui::view::widgets::center_rect;

/// Renders the confirm-delete-folder popup.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let folder_name = focused_folder(app)
        .map(|f| f.name.as_str())
        .unwrap_or("this folder");
    let popup = center_rect(50, 11, area);
    frame.render_widget(Clear, popup);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Delete folder: ", Style::default().fg(t.inactive)),
            Span::styled(
                folder_name,
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Items inside the folder are NOT deleted —",
            Style::default().fg(t.dim),
        )),
        Line::from(Span::styled(
            "  they move to the \"(No folder)\" bucket.",
            Style::default().fg(t.dim),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Enter",
                Style::default().fg(t.error).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  Delete folder", Style::default().fg(t.foreground)),
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
                .title(" Confirm Delete Folder ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(t.error)),
        ),
        popup,
    );
}
