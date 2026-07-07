//! Item-detail screen renderer (read-only and edit modes).
//!
//! The actual detail field list is built by
//! [`crate::tui::detail_fields::build_detail_fields`] so other parts of
//! the TUI (e.g. the edit-mode entry flow) walk the same field order.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::domain::item::{Item, item_type_label};
use crate::tui::app::App;
use crate::tui::detail_fields::build_detail_fields;
use crate::tui::view::action::action_text_style;
use crate::tui::view::widgets::{
    cursor_line, field_areas_windowed, render_cmd_bar_with_help, render_field_card,
};

thread_local! {
    /// Frame-local hit map for the detail/edit field cards — one `(rect,
    /// field index)` per visible card, recorded from the exact layout rects
    /// so a click focuses the card the user sees (no `/4` row arithmetic).
    static DETAIL_HITS: std::cell::RefCell<Vec<(Rect, usize)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn register_field(rect: Rect, idx: usize) {
    if rect.width > 0 && rect.height > 0 {
        DETAIL_HITS.with(|h| h.borrow_mut().push((rect, idx)));
    }
}

/// The field index under `(column, row)`, if any — consumed by the mouse
/// layer to focus (and, on a repeat click, reveal) the field card.
pub fn detail_field_at(column: u16, row: u16) -> Option<usize> {
    DETAIL_HITS.with(|h| {
        h.borrow()
            .iter()
            .rev()
            .find(|(r, _)| {
                column >= r.x && column < r.x + r.width && row >= r.y && row < r.y + r.height
            })
            .map(|(_, i)| *i)
    })
}

/// Renders the detail screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    DETAIL_HITS.with(|h| h.borrow_mut().clear());
    let area = frame.area();
    let item = match app.vault.selected_item() {
        Some(i) => i.clone(),
        None => return,
    };
    let t = &app.theme;

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    // Header line — name, type, mode tag, action state.
    let (action_text, action_style) = action_text_style(app);
    let mode_tag = if app.edit.active {
        Span::styled(
            "  [EDIT]",
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("")
    };
    let pad = (area.width as usize).saturating_sub(
        4 + item.name.len() + 4 + item_type_label(item.item_type).len() + action_text.len() + 2,
    );
    let padded = format!("{:>width$}", action_text, width = action_text.len() + pad);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ← ", Style::default().fg(t.dim)),
            Span::styled(
                item.name.as_str(),
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  [{}]", item_type_label(item.item_type)),
                Style::default().fg(t.inactive),
            ),
            mode_tag,
            Span::styled(padded, action_style),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(t.inactive)),
        ),
        chunks[0],
    );

    crate::tui::view::widgets::register_scroll(
        chunks[1],
        crate::tui::view::widgets::ScrollTarget::Detail,
    );

    if app.edit.active {
        render_edit_form(frame, app, chunks[1]);
        // Edit mode hints stay short and load-bearing; the per-row
        // structural shortcuts (Alt+N add field, Alt+R rename, Alt+T
        // type cycle, Alt+U add URL, Alt+Del remove, F2 reveal) are
        // documented in F1.
        let hints: &[(&str, &str)] = &[
            ("Tab/↑↓", "field"),
            ("←→", "cursor"),
            ("Enter", "save"),
            ("Esc", "cancel"),
        ];
        render_cmd_bar_with_help(frame, chunks[2], hints, t);
    } else {
        render_read_only(frame, app, &item, chunks[1]);
        let hints: &[(&str, &str)] = if app.vault.is_trash_view() {
            &[("j/k", "field"), ("Esc", "back"), ("Alt+R", "restore")]
        } else {
            &[
                ("j/k", "field"),
                ("Esc", "back"),
                ("Alt+C", "copy"),
                ("Alt+E", "edit"),
            ]
        };
        render_cmd_bar_with_help(frame, chunks[2], hints, t);
    }
}

fn render_read_only(frame: &mut Frame, app: &App, item: &Item, area: Rect) {
    let t = &app.theme;
    let fields = build_detail_fields(item, app.show_password, app.detail_field);
    let sel = app.detail_field.min(fields.len().saturating_sub(1));
    let (fas, start) = field_areas_windowed(fields.len(), sel, area);
    for (vis, area) in fas.iter().enumerate() {
        let i = start + vis;
        let Some(field) = fields.get(i) else {
            break;
        };
        let is_sel = i == sel;
        let bcol = if is_sel { t.accent } else { t.inactive };
        let hint = if field.hidden && is_sel {
            "  (F2: reveal)"
        } else {
            ""
        };
        let vline = if is_sel {
            Line::from(Span::styled(
                field.value.as_str(),
                Style::default().fg(t.foreground),
            ))
        } else {
            Line::from(Span::styled(
                field.value.as_str(),
                Style::default().fg(t.inactive),
            ))
        };
        render_field_card(frame, &field.label, hint, vline, bcol, *area, t);
        register_field(*area, i);
    }
}

fn render_edit_form(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let ef = &app.edit.fields;
    let (fas, start) = field_areas_windowed(ef.len(), app.edit.field_idx, area);
    for (vis, area) in fas.iter().enumerate() {
        let i = start + vis;
        let Some(field) = ef.get(i) else {
            break;
        };
        let sel = i == app.edit.field_idx;
        let bcol = if sel { t.accent } else { t.inactive };
        // Compose the hint: type tag for custom rows + reveal/hide/read-only.
        let custom_tag = match field.custom_type() {
            Some(0) => " [text]",
            Some(1) => " [hidden]",
            Some(2) => " [boolean]",
            Some(3) => " [linked]",
            Some(_) | None => "",
        };
        let action_hint = if field.read_only && sel {
            " (read-only)"
        } else if field.hidden && sel && !field.revealed {
            "  (F2: reveal)"
        } else if field.hidden && sel && field.revealed {
            "  (F2: hide)"
        } else {
            ""
        };
        let combined_hint = format!("{custom_tag}{action_hint}");
        let display = if field.hidden && !field.revealed {
            "●".repeat(field.value.chars().count().max(8))
        } else {
            field.value.to_string()
        };
        let vline = if sel && !field.read_only {
            cursor_line(&display, field.cursor, t)
        } else {
            Line::from(Span::styled(display, Style::default().fg(t.inactive)))
        };
        render_field_card(frame, &field.label, &combined_hint, vline, bcol, *area, t);
        register_field(*area, i);
    }
}
