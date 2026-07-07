//! Confirm-delete-folder popup renderer.

use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::tui::app::App;
use crate::tui::flows::folders::focused_folder;
use crate::tui::view::widgets::{ConfirmAction, ConfirmPopup, ConfirmTone, draw_confirm_popup};

/// Renders the confirm-delete-folder popup.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let folder_name = focused_folder(app)
        .map(|f| f.name.as_str())
        .unwrap_or("this folder");
    draw_confirm_popup(
        frame,
        area,
        t,
        ConfirmPopup {
            title: " Confirm Delete Folder ",
            width_pct: 50,
            body: vec![
                Line::from(vec![
                    Span::styled("  Delete folder: ", Style::default().fg(t.inactive)),
                    Span::styled(
                        folder_name.to_string(),
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
            ],
            actions: vec![
                ConfirmAction {
                    key: "Enter",
                    code: KeyCode::Enter,
                    label: "Delete folder",
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
