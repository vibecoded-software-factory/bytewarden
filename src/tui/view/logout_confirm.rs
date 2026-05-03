//! Confirm-logout popup renderer.
//!
//! Drawn on top of the vault screen — same overlay pattern used by
//! [`crate::tui::view::confirm`].

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::view::widgets::center_rect;

/// Renders the confirm-logout popup.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let popup = center_rect(50, 11, area);
    frame.render_widget(Clear, popup);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Log out of this account?",
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  The account will be removed from the local Bitwarden",
            Style::default().fg(t.dim),
        )),
        Line::from(Span::styled(
            "  CLI. Use Lock instead if you only want to clear the",
            Style::default().fg(t.dim),
        )),
        Line::from(Span::styled(
            "  session key for now.",
            Style::default().fg(t.dim),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Enter",
                Style::default().fg(t.error).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  Log out", Style::default().fg(t.foreground)),
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
                .title(" Confirm Logout ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(t.error)),
        ),
        popup,
    );
}
