//! Attachment-download popup renderer.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::tui::app::App;
use crate::tui::view::widgets::{InputFooter, InputPopup, draw_input_popup};

/// Renders the attachment-download popup over the detail screen.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.attachment_download else {
        return;
    };
    let t = &app.theme;
    draw_input_popup(
        frame,
        area,
        t,
        InputPopup {
            title: " Download attachment ",
            width_pct: 70,
            context: vec![
                Line::from(vec![
                    Span::styled(" Item: ", Style::default().fg(t.dim)),
                    Span::styled(
                        state.item_name.to_string(),
                        Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(" File: ", Style::default().fg(t.dim)),
                    Span::styled(
                        state.file_name.to_string(),
                        Style::default().fg(t.foreground),
                    ),
                ]),
            ],
            label: "Save to",
            label_hint: "full destination path; existing files are overwritten by bw",
            editor: &state.path,
            placeholder: "/path/to/save",
            footer: InputFooter::Legend(&[("Enter", "download"), ("Esc", "cancel")]),
        },
    );
}
