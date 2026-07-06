//! Confirm-delete popup renderer.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::view::widgets::center_rect;

/// Renders the confirm-delete popup over the vault screen.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let name = app
        .vault
        .selected_item()
        .map(|i| i.name.as_str())
        .unwrap_or("this item");
    let popup = center_rect(50, 10, area);
    frame.render_widget(Clear, popup);

    // In trash view, Enter = permanent delete (the item is already
    // trashed). In the regular vault, Enter = trash and D = permanent.
    let lines = if app.vault.is_trash_view() {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Delete: ", Style::default().fg(t.inactive)),
                Span::styled(
                    name,
                    Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Already in trash — this will delete permanently.",
                Style::default().fg(t.dim),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  Enter",
                    Style::default().fg(t.error).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  Delete permanently", Style::default().fg(t.error)),
            ]),
            Line::from(vec![
                Span::styled("  Esc  ", Style::default().fg(t.dim)),
                Span::styled("  Cancel", Style::default().fg(t.dim)),
            ]),
            Line::from(""),
        ]
    } else {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Delete: ", Style::default().fg(t.inactive)),
                Span::styled(
                    name,
                    Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  This action cannot be easily undone.",
                Style::default().fg(t.dim),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  Enter",
                    Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  Move to trash", Style::default().fg(t.foreground)),
            ]),
            Line::from(vec![
                Span::styled(
                    "  D    ",
                    Style::default().fg(t.error).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  Delete permanently", Style::default().fg(t.error)),
            ]),
            Line::from(vec![
                Span::styled("  Esc  ", Style::default().fg(t.dim)),
                Span::styled("  Cancel", Style::default().fg(t.dim)),
            ]),
            Line::from(""),
        ]
    };

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" Confirm Delete ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(t.error)),
        ),
        popup,
    );
}
