//! Renderer for the command palette (`Ctrl+P`) — the standard picker
//! modal: a fuzzy query row over a windowed command list, each row with
//! its keybinding right-aligned (the executable cheat-sheet).

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

/// The index into `state.filtered` under `(column, row)`, if a command
/// row is there — consumed by the mouse layer to run that command.
pub fn palette_row_at(column: u16, row: u16) -> Option<usize> {
    picker_row_at(column, row)
}

/// Draws the command palette over its origin screen.
pub fn draw(frame: &mut Frame, app: &App) {
    let Some(state) = app.palette.as_ref() else {
        return;
    };
    let t = &app.theme;
    let width = modal_inner_width(frame);

    // Command rows — label left, keybinding right-aligned in `key_style`.
    let rows: Vec<PickerRow> = state
        .filtered
        .iter()
        .filter_map(|&i| state.all.get(i))
        .map(|c| {
            let label_w = c.label.chars().count();
            let keys_w = c.keys.chars().count();
            let pad = width.saturating_sub(label_w + keys_w + 1).max(1);
            PickerRow::Item(vec![Line::from(vec![
                Span::styled(c.label, Style::default().fg(t.foreground)),
                Span::raw(" ".repeat(pad)),
                Span::styled(c.keys, key_style(t)),
            ])])
        })
        .collect();

    draw_picker_modal(
        frame,
        t,
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
        },
    );
}
