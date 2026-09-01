use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::reprompt::ProtectedAction;
use crate::tui::view::widgets::{center_rect, editor_spans_masked, legend_line, rounded_block};

pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some(state) = &app.reprompt else {
        return;
    };
    let t = &app.theme;
    let popup = center_rect(60, 11, area);
    crate::tui::view::widgets::register_modal(popup);
    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .title(" Master password required ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
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

    let line = if state.input.is_empty() {
        let mut spans = editor_spans_masked(&state.input, true, t);
        spans.push(Span::styled(
            " type your master password",
            Style::default().fg(t.placeholder),
        ));
        Line::from(spans)
    } else {
        Line::from(editor_spans_masked(&state.input, true, t))
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

    let mut hints = legend_line(
        &[("Enter", "verify"), ("Esc", "cancel")],
        chunks[5].width.saturating_sub(1),
        t,
    );
    hints.spans.insert(0, Span::raw(" "));
    frame.render_widget(Paragraph::new(hints), chunks[5]);
}
