use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::tui::app::App;
use crate::tui::view::widgets::{
    PickerModal, PickerRow, ScrollTarget, draw_picker_modal, empty_state_lines,
};

pub fn draw_popup(frame: &mut Frame, _area: Rect, app: &App) {
    let Some(state) = &app.memberships else {
        return;
    };
    let t = &app.theme;

    let header = |text: String| {
        PickerRow::Header(Line::from(Span::styled(
            text,
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
        )))
    };
    let note = |text: &str| {
        PickerRow::Header(Line::from(Span::styled(
            format!("    {text}"),
            Style::default().fg(t.dim).add_modifier(Modifier::ITALIC),
        )))
    };
    let collection = |name: &str| {
        PickerRow::Item(vec![Line::from(Span::styled(
            name.to_string(),
            Style::default().fg(t.foreground),
        ))])
    };

    let mut rows: Vec<PickerRow> = Vec::new();
    for org in &state.organizations {
        rows.push(header(format!("{} {}", app.icons.org(), org.name)));
        let mut any = false;
        for c in state
            .collections
            .iter()
            .filter(|c| c.organization_id.as_deref() == Some(org.id.as_str()))
        {
            rows.push(collection(&c.name));
            any = true;
        }
        if !any {
            rows.push(note("(no collections visible to you)"));
        }
    }

    let orphans: Vec<&crate::domain::Collection> = state
        .collections
        .iter()
        .filter(|c| c.organization_id.is_none())
        .collect();
    if !orphans.is_empty() {
        rows.push(header("(orphan collections, no parent org)".to_string()));
        for c in orphans {
            rows.push(collection(&c.name));
        }
    }

    let orgs = state.organizations.len();
    let cols = state.collections.len();
    draw_picker_modal(
        frame,
        t,
        app.icons,
        PickerModal {
            title: format!(" Memberships · {orgs} org / {cols} collections "),

            query: None,
            rows,
            selected: state.cursor,
            empty: empty_state_lines(
                "No organisations",
                &[
                    "personal items live in folders — see the [1] sidebar",
                    "organisations are joined from the Bitwarden web vault",
                    "Esc closes",
                ],
                t,
            ),
            legend: &[("j/k ↑↓", "scroll"), ("Esc", "close")],
            scroll_target: Some(ScrollTarget::Memberships),

            size: None,
        },
    );
}
