//! Master-password reverify popup.
//!
//! Drawn over whichever screen the user came from when they triggered
//! a reprompt-protected action. Always renders the password as
//! `●` characters — there is no F2-style reveal here, since the
//! point of the popup is the user re-typing a value they already
//! know.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::reprompt::ProtectedAction;
use crate::tui::view::widgets::{center_rect, rounded_block};

/// Renders the popup. No-op when no reprompt is in flight.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.reprompt else {
        return;
    };
    let t = &app.theme;
    let popup = center_rect(60, 11, area);
    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .title(" Master password required ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1), // padding
        ratatui::layout::Constraint::Length(1), // explanation label
        ratatui::layout::Constraint::Length(1), // input label
        ratatui::layout::Constraint::Length(3), // input
        ratatui::layout::Constraint::Length(1), // feedback strip
        ratatui::layout::Constraint::Length(1), // hint
    ])
    .split(inner);

    let action_label = match state.after {
        ProtectedAction::CopyPassword => "copying the password",
        ProtectedAction::CopyTotp(_) => "copying the TOTP code",
        ProtectedAction::CopySelectedDetailField => "copying the focused field",
        ProtectedAction::RevealDetail => "revealing hidden fields",
        ProtectedAction::RevealEditField => "revealing the focused field",
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            format!(" This item asks to re-verify before {action_label}."),
            Style::default().fg(t.dim),
        )])),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            " Master password",
            Style::default().fg(t.dim),
        )])),
        chunks[2],
    );

    // Always rendered masked — the popup never reveals the typed
    // password, even with F2.
    let dots = "●".repeat(state.input.len_chars());
    let line = if state.input.is_empty() {
        Line::from(vec![
            Span::styled("█", Style::default().fg(t.accent)),
            Span::styled(
                "  type your master password",
                Style::default().fg(t.placeholder),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(dots, Style::default().fg(t.foreground)),
            Span::styled("█", Style::default().fg(t.accent)),
        ])
    };
    frame.render_widget(
        Paragraph::new(line).block(rounded_block(Style::default().fg(t.accent))),
        chunks[3],
    );

    if state.error {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    " ✕ ",
                    Style::default().fg(t.error).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Invalid master password. Please try again.",
                    Style::default().fg(t.error),
                ),
            ])),
            chunks[4],
        );
    }

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Enter to verify · Esc to cancel",
            Style::default().fg(t.dim).add_modifier(Modifier::ITALIC),
        ))),
        chunks[5],
    );
}
