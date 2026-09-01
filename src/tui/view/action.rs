use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::tui::action::ActionState;
use crate::tui::app::App;

const SPINNER: [&str; 4] = ["⠋", "⠙", "⠸", "⠴"];

pub fn spinner_frame(tick: u8) -> &'static str {
    SPINNER[tick as usize % SPINNER.len()]
}

pub fn action_line(app: &App) -> Option<Line<'static>> {
    let sp = spinner_frame(app.action_tick);
    let t = &app.theme;
    match &app.action_state {
        ActionState::Idle => None,
        ActionState::Running(msg) => Some(Line::from(vec![
            Span::styled(
                format!(" {sp} "),
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(msg.clone(), Style::default().fg(t.accent)),
        ])),
        ActionState::Done(msg) => Some(Line::from(vec![
            Span::styled(
                " ✓ ",
                Style::default().fg(t.success).add_modifier(Modifier::BOLD),
            ),
            Span::styled(msg.clone(), Style::default().fg(t.success)),
        ])),
        ActionState::Error(msg) => Some(Line::from(vec![
            Span::styled(
                " ✕ ",
                Style::default().fg(t.error).add_modifier(Modifier::BOLD),
            ),
            Span::styled(msg.clone(), Style::default().fg(t.error)),
        ])),
    }
}

pub fn action_text_style(app: &App) -> (String, Style) {
    let sp = spinner_frame(app.action_tick);
    let t = &app.theme;
    match &app.action_state {
        ActionState::Idle => (String::new(), Style::default()),
        ActionState::Running(msg) => (
            format!("{sp} {msg}"),
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
        ),
        ActionState::Done(msg) => (
            format!("✓ {msg}"),
            Style::default().fg(t.success).add_modifier(Modifier::BOLD),
        ),
        ActionState::Error(msg) => (
            format!("✕ {msg}"),
            Style::default().fg(t.error).add_modifier(Modifier::BOLD),
        ),
    }
}
