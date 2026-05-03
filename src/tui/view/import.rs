//! Vault-import popup renderer.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::import::ImportFocus;
use crate::tui::view::widgets::{center_rect, cursor_line, rounded_block};

/// Renders the import popup.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.import else {
        return;
    };
    let t = &app.theme;
    let popup = center_rect(70, 14, area);
    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .title(" Import vault ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(t.accent));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
    ])
    .split(inner);

    // Format field
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Format", Style::default().fg(t.dim)),
            Span::styled(
                "  (run `bw import --formats` for the list)",
                Style::default().fg(t.dim),
            ),
        ])),
        chunks[1],
    );
    let fmt_focus = state.focus == ImportFocus::Format;
    let fmt_line = if fmt_focus {
        cursor_line(&state.format, state.format_cursor, t)
    } else {
        Line::from(Span::styled(
            state.format.as_str(),
            Style::default().fg(t.inactive),
        ))
    };
    frame.render_widget(
        Paragraph::new(fmt_line).block(rounded_block(if fmt_focus {
            Style::default().fg(t.accent)
        } else {
            Style::default().fg(t.inactive)
        })),
        chunks[2],
    );

    // Path field
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Input path", Style::default().fg(t.dim)),
            Span::styled(
                "  (full path to the file to import)",
                Style::default().fg(t.dim),
            ),
        ])),
        chunks[3],
    );
    let path_focus = state.focus == ImportFocus::Path;
    let path_line = if path_focus {
        if state.path.is_empty() {
            Line::from(vec![
                Span::styled("█", Style::default().fg(t.accent)),
                Span::styled("  /path/to/export.json", Style::default().fg(t.placeholder)),
            ])
        } else {
            cursor_line(&state.path, state.path_cursor, t)
        }
    } else {
        Line::from(Span::styled(
            state.path.as_str(),
            Style::default().fg(t.inactive),
        ))
    };
    frame.render_widget(
        Paragraph::new(path_line).block(rounded_block(if path_focus {
            Style::default().fg(t.accent)
        } else {
            Style::default().fg(t.inactive)
        })),
        chunks[4],
    );

    // Hints + warning
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Tab: switch field   |   Enter: import   |   Esc: cancel",
            Style::default().fg(t.dim),
        ))),
        chunks[5],
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Imported items are added to your vault — duplicates are not deduped.",
            Style::default().fg(t.error).add_modifier(Modifier::ITALIC),
        ))),
        chunks[6],
    );
}
