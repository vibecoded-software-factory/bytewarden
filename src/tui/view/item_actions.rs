use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
};

use crate::tui::app::App;
use crate::tui::view::widgets::{
    PickerModal, PickerRow, draw_picker_modal, empty_state_lines, picker_row_at,
};

const MENU_WIDTH_PCT: u16 = 28;

const MENU_CHROME_ROWS: u16 = 3;

pub fn item_action_at(column: u16, row: u16) -> Option<usize> {
    picker_row_at(column, row)
}

pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.item_actions else {
        return;
    };
    let t = &app.theme;

    let rows: Vec<PickerRow> = state
        .actions
        .iter()
        .map(|a| {
            PickerRow::Item(vec![Line::from(Span::styled(
                a.label(),
                Style::default().fg(t.foreground),
            ))])
        })
        .collect();
    let height = (state.actions.len() as u16 + MENU_CHROME_ROWS)
        .min(area.height.saturating_sub(2))
        .max(5);

    draw_picker_modal(
        frame,
        t,
        app.icons,
        PickerModal {
            title: " Actions ".to_string(),

            query: None,
            rows,
            selected: state.cursor,

            empty: empty_state_lines("No actions for this item", &["Esc closes the menu"], t),
            legend: &[("↑↓", "pick"), ("Enter", "do"), ("Esc", "close")],

            scroll_target: None,
            size: Some((MENU_WIDTH_PCT, height)),
        },
    );
}
