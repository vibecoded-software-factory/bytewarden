//! "Create new item" screen renderer (type-picker + form).

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::domain::filter::CREATE_ITEM_TYPES;
use crate::tui::app::App;
use crate::tui::view::action::action_line;
use crate::tui::view::widgets::{
    cursor_line, field_areas, render_cmd_bar_with_help, render_field_card, rounded_block,
};

/// Renders the create screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let t = &app.theme;
    let area = frame.area();
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .split(area);

    let title = if app.create_choosing_type {
        "New Item — choose type"
    } else {
        app.create_type.label()
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " + ",
                Style::default().fg(t.success).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                title,
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(t.inactive)),
        ),
        chunks[0],
    );

    if app.create_choosing_type {
        let areas = Layout::vertical(
            (0..CREATE_ITEM_TYPES.len())
                .map(|_| Constraint::Length(3))
                .collect::<Vec<_>>(),
        )
        .split(chunks[1]);
        for (i, ct) in CREATE_ITEM_TYPES.iter().enumerate() {
            if i >= areas.len() {
                break;
            }
            let sel = i == app.create_type_idx;
            let col = if sel { t.accent } else { t.inactive };
            let prefix = if sel { "▶ " } else { "  " };
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!("{}{}", prefix, ct.label()),
                    Style::default().fg(col).add_modifier(if sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
                )))
                .block(rounded_block(Style::default().fg(col))),
                areas[i],
            );
        }
    } else {
        let feedback = action_line(app);
        let fb_h: u16 = if feedback.is_some() { 1 } else { 0 };
        let form_parts =
            Layout::vertical([Constraint::Length(fb_h), Constraint::Min(0)]).split(chunks[1]);
        if let Some(line) = feedback {
            frame.render_widget(Paragraph::new(line), form_parts[0]);
        }

        let fas = field_areas(app.create_fields.len(), form_parts[1]);
        for (i, field) in app.create_fields.iter().enumerate() {
            if i >= fas.len() {
                break;
            }
            let sel = i == app.create_field_idx;
            let bcol = if sel { t.accent } else { t.inactive };
            let hint = if field.is_organization() && sel {
                "  (← → to cycle)"
            } else if field.is_collections() && sel {
                "  (Alt+L to assign)"
            } else if field.hidden && sel && !field.revealed {
                "  (F2: reveal)"
            } else if field.hidden && sel && field.revealed {
                "  (F2: hide)"
            } else {
                ""
            };
            let display = if field.hidden && !field.revealed {
                "●".repeat(field.value.chars().count())
            } else if field.is_collections() && field.value.is_empty() {
                "(none — Alt+L to pick)".to_string()
            } else {
                field.value.to_string()
            };
            // Read-only rows don't accept text input — render them
            // without a cursor so the user isn't tempted to type.
            let vline = if sel && !field.read_only {
                cursor_line(&display, field.cursor, t)
            } else {
                Line::from(Span::styled(display, Style::default().fg(t.inactive)))
            };
            render_field_card(frame, &field.label, hint, vline, bcol, fas[i], t);
        }
    }

    let (hf, hs) = if app.create_choosing_type {
        ("j/k type · Enter pick · Esc cancel", "j/k  Enter  Esc")
    } else {
        (
            "Tab/↑↓ field · Enter create · Esc cancel",
            "Tab  Enter  Esc",
        )
    };
    render_cmd_bar_with_help(frame, area, chunks[2], hf, hs, t.dim, t);
}
