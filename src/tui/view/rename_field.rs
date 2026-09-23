use ratatui::{Frame, layout::Rect};

use crate::tui::app::App;
use crate::tui::view::widgets::{InputFooter, InputPopup, draw_input_popup};

pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.rename_field else {
        return;
    };
    draw_input_popup(
        frame,
        area,
        &app.theme,
        InputPopup {
            title: " Rename field ",
            width_pct: 60,
            context: vec![],
            label: "New label",
            label_hint: "Enter to apply, Esc to cancel",
            editor: &state.input,
            placeholder: "type a new name",
            footer: InputFooter::Note("The current value of the field is preserved."),
        },
    );
}
