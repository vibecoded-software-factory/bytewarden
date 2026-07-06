//! Item-detail screen renderer (read-only and edit modes).
//!
//! The actual detail field list is built by
//! [`crate::tui::detail_fields::build_detail_fields`] so other parts of
//! the TUI (e.g. the edit-mode entry flow) walk the same field order.

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::domain::item::{Item, item_type_label};
use crate::tui::app::App;
use crate::tui::detail_fields::build_detail_fields;
use crate::tui::view::action::action_text_style;
use crate::tui::view::widgets::{
    cursor_line, field_areas, render_cmd_bar_with_help, render_field_card,
};

/// Renders the detail screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
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

    app.mouse_areas.detail = Some(chunks[1]);

    if app.edit.active {
        render_edit_form(frame, app, chunks[1]);
        // Edit mode hints stay short and load-bearing; the per-row
        // structural shortcuts (Alt+N add field, Alt+R rename, Alt+T
        // type cycle, Alt+U add URL, Alt+Del remove, F2 reveal) are
        // documented in F1.
        let full = "Tab/↑↓ field · ←→ cursor · Enter save · Esc cancel";
        let short = "Tab  Enter  Esc";
        render_cmd_bar_with_help(frame, area, chunks[2], full, short, t.dim, t);
    } else {
        render_read_only(frame, app, &item, chunks[1]);
        let (full, short) = if app.vault.is_trash_view() {
            ("j/k field · Esc back · Alt+R restore", "j/k  Esc  Alt+R")
        } else {
            (
                "j/k field · Esc back · Alt+C copy · Alt+E edit",
                "j/k  Esc  Alt+C  Alt+E",
            )
        };
        render_cmd_bar_with_help(frame, area, chunks[2], full, short, t.dim, t);
    }
}

fn render_read_only(frame: &mut Frame, app: &App, item: &Item, area: ratatui::layout::Rect) {
    let t = &app.theme;
    let fields = build_detail_fields(item, app.show_password, app.detail_field);
    let sel = app.detail_field.min(fields.len().saturating_sub(1));
    let fas = field_areas(fields.len(), area);
    for (i, field) in fields.iter().enumerate() {
        if i >= fas.len() {
            break;
        }
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
        render_field_card(frame, &field.label, hint, vline, bcol, fas[i], t);
    }
}

fn render_edit_form(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let t = &app.theme;
    let ef = &app.edit.fields;
    let fas = field_areas(ef.len(), area);
    for (i, field) in ef.iter().enumerate() {
        if i >= fas.len() {
            break;
        }
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
        render_field_card(frame, &field.label, &combined_hint, vline, bcol, fas[i], t);
    }
}
