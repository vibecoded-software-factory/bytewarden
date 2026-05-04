//! Memberships popup renderer.
//!
//! Displays each organisation as a header, followed by a bulleted list
//! of its collections. Personal-only accounts see a friendly empty
//! state.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::view::widgets::center_rect;

/// Renders the memberships popup.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.memberships else {
        return;
    };
    let t = &app.theme;
    let popup = center_rect(70, 22, area);
    frame.render_widget(Clear, popup);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    if state.organizations.is_empty() {
        lines.push(Line::from(Span::styled(
            "  You are not a member of any Bitwarden organization.",
            Style::default().fg(t.dim),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Personal accounts have no collections — items live in",
            Style::default().fg(t.dim),
        )));
        lines.push(Line::from(Span::styled(
            "  folders only (see the Folders sidebar panel).",
            Style::default().fg(t.dim),
        )));
    } else {
        for org in &state.organizations {
            lines.push(Line::from(vec![
                Span::styled("  🏢 ", Style::default().fg(t.accent)),
                Span::styled(
                    org.name.as_str(),
                    Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                ),
            ]));
            let mut org_collections: Vec<&_> = state
                .collections
                .iter()
                .filter(|c| c.organization_id.as_deref() == Some(org.id.as_str()))
                .collect();
            org_collections.sort_by_key(|c| c.name.to_lowercase());
            if org_collections.is_empty() {
                lines.push(Line::from(Span::styled(
                    "      (no collections visible to you)",
                    Style::default().fg(t.dim).add_modifier(Modifier::ITALIC),
                )));
            } else {
                for c in org_collections {
                    lines.push(Line::from(vec![
                        Span::styled("      • ", Style::default().fg(t.dim)),
                        Span::styled(c.name.as_str(), Style::default().fg(t.foreground)),
                    ]));
                }
            }
            lines.push(Line::from(""));
        }
        // Surface any collection that has no parent org (defensive —
        // shouldn't happen with current bw output but cheap to handle).
        let orphans: Vec<&_> = state
            .collections
            .iter()
            .filter(|c| c.organization_id.is_none())
            .collect();
        if !orphans.is_empty() {
            lines.push(Line::from(Span::styled(
                "  (Orphan collections, no parent org)",
                Style::default().fg(t.dim).add_modifier(Modifier::ITALIC),
            )));
            for c in orphans {
                lines.push(Line::from(vec![
                    Span::styled("      • ", Style::default().fg(t.dim)),
                    Span::styled(c.name.as_str(), Style::default().fg(t.foreground)),
                ]));
            }
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Esc / Enter to close.",
        Style::default().fg(t.dim),
    )));

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" Memberships (read-only) ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(t.accent)),
        ),
        popup,
    );
}
