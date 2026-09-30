use ratatui::{
    Frame,
    style::Style,
    text::{Line, Span},
};

use crate::tui::app::App;
use crate::tui::view::widgets::{
    PickerModal, PickerRow, ScrollTarget, draw_picker_modal, empty_state_lines, key_style,
    modal_inner_width, picker_row_at,
};

pub fn palette_row_at(column: u16, row: u16) -> Option<usize> {
    picker_row_at(column, row)
}

pub fn draw(frame: &mut Frame, app: &App) {
    let Some(state) = app.palette.as_ref() else {
        return;
    };
    let t = &app.theme;
    let width = modal_inner_width(frame);

    let rows: Vec<PickerRow> = state
        .filtered
        .iter()
        .filter_map(|&i| state.all.get(i))
        .map(|c| {
            let keys = crate::tui::keyboard::label(c.keys).into_owned();
            let label_w = Span::raw(c.label).width();
            let keys_w = Span::raw(keys.as_str()).width();
            let pad = width.saturating_sub(label_w + keys_w + 1).max(1);
            PickerRow::Item(vec![Line::from(vec![
                Span::styled(c.label, Style::default().fg(t.foreground)),
                Span::raw(" ".repeat(pad)),
                Span::styled(keys, key_style(t)),
            ])])
        })
        .collect();

    draw_picker_modal(
        frame,
        t,
        app.icons,
        PickerModal {
            title: format!(" Command palette · {} ", state.filtered.len()),
            query: Some((&state.query, "type to search commands…")),
            rows,
            selected: state.selected,
            empty: empty_state_lines(
                "No matching command",
                &["fewer letters fuzzy-match more", "Esc closes"],
                t,
            ),
            legend: &[("↑↓", "select"), ("Enter", "run"), ("Esc", "cancel")],
            scroll_target: Some(ScrollTarget::Palette),

            size: None,
        },
    );
}
