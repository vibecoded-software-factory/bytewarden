//! Vault-export popup renderer.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::export::{ExportFocus, ExportFormat};
use crate::tui::view::widgets::{center_rect, editor_line, rounded_block};

/// Renders the export popup.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.export else {
        return;
    };
    let t = &app.theme;
    let popup = center_rect(70, 13, area);
    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .title(" Export vault ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1), // padding
        ratatui::layout::Constraint::Length(1), // format label
        ratatui::layout::Constraint::Length(1), // format value
        ratatui::layout::Constraint::Length(1), // path label
        ratatui::layout::Constraint::Length(3), // path input
        ratatui::layout::Constraint::Length(1), // hints
        ratatui::layout::Constraint::Length(1), // security note
    ])
    .split(inner);

    // ── Format ────────────────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Format", Style::default().fg(t.dim)),
            Span::styled("  (Space / ←→ to cycle)", Style::default().fg(t.dim)),
        ])),
        chunks[1],
    );
    let fmt_focus = state.focus == ExportFocus::Format;
    let fmt_label = format!(" {}", state.format.label());
    let fmt_style = if fmt_focus {
        Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(t.foreground)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(if fmt_focus { "▶" } else { " " }, fmt_style),
            Span::styled(fmt_label, fmt_style),
            Span::styled(
                format!("  ({})", state.format.cli_arg()),
                Style::default().fg(t.dim),
            ),
        ])),
        chunks[2],
    );

    // ── Path ──────────────────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Output path", Style::default().fg(t.dim)),
            Span::styled(
                "  (full filename, will overwrite)",
                Style::default().fg(t.dim),
            ),
        ])),
        chunks[3],
    );
    let path_focus = state.focus == ExportFocus::Path;
    let path_line = if path_focus {
        editor_line(&state.path, t)
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

    // ── Footer hints + security note ──────────────────────────────────────
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Tab switch field · Enter export · Esc cancel",
            Style::default().fg(t.dim),
        ))),
        chunks[5],
    );

    let security_note = match state.format {
        ExportFormat::EncryptedJson => {
            " Encrypted with your account key — only re-importable into this same Bitwarden account."
        }
        _ => " The file will contain plaintext credentials. Pick a safe location.",
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            security_note,
            Style::default()
                .fg(if matches!(state.format, ExportFormat::EncryptedJson) {
                    t.dim
                } else {
                    t.error
                })
                .add_modifier(Modifier::ITALIC),
        ))),
        chunks[6],
    );
}
