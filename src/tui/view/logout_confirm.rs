//! Confirm-logout popup renderer.
//!
//! Drawn on top of the vault screen — same overlay pattern used by
//! [`crate::tui::view::confirm`].

use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::tui::app::App;
use crate::tui::view::widgets::{ConfirmAction, ConfirmPopup, ConfirmTone, draw_confirm_popup};

/// Renders the confirm-logout popup.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    draw_confirm_popup(
        frame,
        area,
        t,
        ConfirmPopup {
            title: " Confirm Logout ",
            width_pct: 50,
            body: vec![
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
            ],
            actions: vec![
                ConfirmAction {
                    key: "Enter",
                    code: KeyCode::Enter,
                    label: "Log out",
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
