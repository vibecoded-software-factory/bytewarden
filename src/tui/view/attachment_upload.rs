//! Attachment-upload popup renderer.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::tui::app::App;
use crate::tui::view::widgets::{InputFooter, InputPopup, draw_input_popup};

/// Renders the attachment-upload popup over the detail screen.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.attachment_upload else {
        return;
    };
    let t = &app.theme;
    draw_input_popup(
        frame,
        area,
        t,
        InputPopup {
            title: " Upload attachment ",
            width_pct: 70,
            context: vec![Line::from(vec![
                Span::styled(" Item: ", Style::default().fg(t.dim)),
                Span::styled(
                    state.item_name.to_string(),
                    Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                ),
            ])],
            label: "File path",
            label_hint: "full path to the file to upload",
            editor: &state.path,
            placeholder: "/path/to/file.pdf",
            footer: InputFooter::Legend(&[("Enter", "upload"), ("Esc", "cancel")]),
        },
    );
}
