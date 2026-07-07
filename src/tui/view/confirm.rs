//! Confirm-delete popup renderer.

use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::tui::app::App;
use crate::tui::view::widgets::{ConfirmAction, ConfirmPopup, ConfirmTone, draw_confirm_popup};

/// Renders the confirm-delete popup over the vault screen.
///
/// In trash view, Enter = permanent delete (the item is already
/// trashed). In the regular vault, Enter = trash and D = permanent.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let name = app
        .vault
        .selected_item()
        .map(|i| i.name.as_str())
        .unwrap_or("this item");
    let name_line = Line::from(vec![
        Span::styled("  Delete: ", Style::default().fg(t.inactive)),
        Span::styled(
            name.to_string(),
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
        ),
    ]);

    let popup = if app.vault.is_trash_view() {
        ConfirmPopup {
            title: " Confirm Delete ",
            width_pct: 50,
            body: vec![
                name_line,
                Line::from(""),
                Line::from(Span::styled(
                    "  Already in trash — this will delete permanently.",
                    Style::default().fg(t.dim),
                )),
            ],
            actions: vec![
                ConfirmAction {
                    key: "Enter",
                    code: KeyCode::Enter,
                    label: "Delete permanently",
                    tone: ConfirmTone::Danger,
                },
                ConfirmAction {
                    key: "Esc",
                    code: KeyCode::Esc,
                    label: "Cancel",
                    tone: ConfirmTone::Cancel,
                },
            ],
        }
    } else {
        ConfirmPopup {
            title: " Confirm Delete ",
            width_pct: 50,
            body: vec![
                name_line,
                Line::from(""),
                Line::from(Span::styled(
                    "  This action cannot be easily undone.",
                    Style::default().fg(t.dim),
                )),
            ],
            actions: vec![
                ConfirmAction {
                    key: "Enter",
                    code: KeyCode::Enter,
                    label: "Move to trash",
                    tone: ConfirmTone::Primary,
                },
                ConfirmAction {
                    key: "D",
                    code: KeyCode::Char('D'),
                    label: "Delete permanently",
                    tone: ConfirmTone::Danger,
                },
                ConfirmAction {
                    key: "Esc",
                    code: KeyCode::Esc,
                    label: "Cancel",
                    tone: ConfirmTone::Cancel,
                },
            ],
        }
    };
    draw_confirm_popup(frame, area, t, popup);
}
