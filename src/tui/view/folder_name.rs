//! Folder-name input popup (used for both Create and Rename).

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::flows::folders::FolderNamePurpose;
use crate::tui::view::widgets::{center_rect, editor_line, rounded_block};

/// Renders the folder-name popup. No-op when no popup is in flight.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.folder_name else {
        return;
    };
    let t = &app.theme;
    let popup = center_rect(60, 9, area);
    crate::tui::view::widgets::register_modal(popup);
    frame.render_widget(Clear, popup);

    let title = match state.purpose {
        FolderNamePurpose::Create => " New folder ",
        FolderNamePurpose::Rename { .. } => " Rename folder ",
    };
    let outer = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Length(1),
    ])
    .split(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Folder name", Style::default().fg(t.dim)),
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
            Span::styled("  type a folder name", Style::default().fg(t.placeholder)),
        ])
    } else {
        editor_line(&state.input, t)
    };
    frame.render_widget(
        Paragraph::new(line).block(rounded_block(Style::default().fg(t.accent))),
        chunks[2],
    );

    let footer = match &state.purpose {
        FolderNamePurpose::Create => " A new folder will be created at the top level.".to_string(),
        FolderNamePurpose::Rename { .. } => " Items inside the folder are unaffected.".to_string(),
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            footer,
            Style::default().fg(t.dim).add_modifier(Modifier::ITALIC),
        ))),
        chunks[3],
    );
}
