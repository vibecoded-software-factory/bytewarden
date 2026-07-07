//! Confirm-delete-attachment popup renderer.

use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::tui::app::App;
use crate::tui::view::widgets::{ConfirmAction, ConfirmPopup, ConfirmTone, draw_confirm_popup};

/// Renders the confirm-delete-attachment popup.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let (file_name, item_name) = match &app.attachment_delete {
        Some(s) => (s.file_name.as_str(), s.item_name.as_str()),
        None => ("this attachment", ""),
    };
    draw_confirm_popup(
        frame,
        area,
        t,
        ConfirmPopup {
            title: " Confirm Delete Attachment ",
            width_pct: 58,
            body: vec![
                Line::from(vec![
                    Span::styled("  Delete attachment: ", Style::default().fg(t.inactive)),
                    Span::styled(
                        file_name.to_string(),
                        Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  From item: ", Style::default().fg(t.inactive)),
                    Span::styled(item_name.to_string(), Style::default().fg(t.foreground)),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  This cannot be undone. The file is removed from the server.",
                    Style::default().fg(t.dim),
                )),
            ],
            actions: vec![
                ConfirmAction {
                    key: "Enter",
                    code: KeyCode::Enter,
                    label: "Delete attachment",
                    tone: ConfirmTone::Danger,
                },
                ConfirmAction {
                    key: "Esc",
                    code: KeyCode::Esc,
                    label: "Cancel",
                    tone: ConfirmTone::Cancel,
                },
            ],
        },
    );
}
