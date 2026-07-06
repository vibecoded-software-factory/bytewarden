//! Send-create popup renderer.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::send::SendFocus;
use crate::tui::view::widgets::{center_rect, editor_line, register_field_hit, rounded_block};

/// Renders the send-create popup.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.send_create else {
        return;
    };
    let t = &app.theme;
    let popup = center_rect(72, 17, area);
    crate::tui::view::widgets::register_modal(popup);
    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .title(" Create Send ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1), // padding
        ratatui::layout::Constraint::Length(1), // name label
        ratatui::layout::Constraint::Length(3), // name input
        ratatui::layout::Constraint::Length(1), // days label
        ratatui::layout::Constraint::Length(1), // days value
        ratatui::layout::Constraint::Length(1), // content label
        ratatui::layout::Constraint::Length(3), // content input
        ratatui::layout::Constraint::Length(1), // hints
        ratatui::layout::Constraint::Length(1), // warning
    ])
    .split(inner);

    // Clickable field regions (label + input each): 0 → Name, 1 → Days,
    // 2 → Content.
    register_field_hit(chunks[1], 0);
    register_field_hit(chunks[2], 0);
    register_field_hit(chunks[3], 1);
    register_field_hit(chunks[4], 1);
    register_field_hit(chunks[5], 2);
    register_field_hit(chunks[6], 2);

    // Name
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Name",
            Style::default().fg(t.dim),
        ))),
        chunks[1],
    );
    let name_focus = state.focus == SendFocus::Name;
    let name_line = if name_focus {
        if state.name.is_empty() {
            Line::from(vec![
                Span::styled("█", Style::default().fg(t.accent)),
                Span::styled(
                    "  e.g. \"meeting notes\"",
                    Style::default().fg(t.placeholder),
                ),
            ])
        } else {
            editor_line(&state.name, t)
        }
    } else {
        Line::from(Span::styled(
            state.name.as_str(),
            Style::default().fg(t.inactive),
        ))
    };
    frame.render_widget(
        Paragraph::new(name_line).block(rounded_block(if name_focus {
            Style::default().fg(t.accent)
        } else {
            Style::default().fg(t.inactive)
        })),
        chunks[2],
    );

    // Days
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Expires in", Style::default().fg(t.dim)),
            Span::styled("  (←→ adjust, 1-31 days)", Style::default().fg(t.dim)),
        ])),
        chunks[3],
    );
    let days_focus = state.focus == SendFocus::Days;
    let arrow = if days_focus { "▶ " } else { "  " };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                arrow,
                Style::default().fg(if days_focus { t.accent } else { t.inactive }),
            ),
            Span::styled(
                format!(
                    "{} day{}",
                    state.days,
                    if state.days == 1 { "" } else { "s" }
                ),
                Style::default()
                    .fg(if days_focus { t.accent } else { t.foreground })
                    .add_modifier(if days_focus {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        ])),
        chunks[4],
    );

    // Content
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Content", Style::default().fg(t.dim)),
            Span::styled(
                "  (single line; for multi-line use bw send directly)",
                Style::default().fg(t.dim),
            ),
        ])),
        chunks[5],
    );
    let content_focus = state.focus == SendFocus::Content;
    let content_line = if content_focus {
        if state.content.is_empty() {
            Line::from(vec![
                Span::styled("█", Style::default().fg(t.accent)),
                Span::styled("  the text to share", Style::default().fg(t.placeholder)),
            ])
        } else {
            editor_line(&state.content, t)
        }
    } else {
        Line::from(Span::styled(
            state.content.as_str(),
            Style::default().fg(t.inactive),
        ))
    };
    frame.render_widget(
        Paragraph::new(content_line).block(rounded_block(if content_focus {
            Style::default().fg(t.accent)
        } else {
            Style::default().fg(t.inactive)
        })),
        chunks[6],
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Tab switch field · Enter create · Esc cancel",
            Style::default().fg(t.dim),
        ))),
        chunks[7],
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " On success the URL is copied to your clipboard.",
            Style::default().fg(t.dim).add_modifier(Modifier::ITALIC),
        ))),
        chunks[8],
    );
}
