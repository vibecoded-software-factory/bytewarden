//! Spinner + action-state line builders shared between several screens.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::tui::action::ActionState;
use crate::tui::app::App;

/// Spinner glyph rotation.
const SPINNER: [&str; 4] = ["-", "\\", "|", "/"];

/// Returns the spinner glyph for the given tick. Rotates every 3 ticks
/// for a calmer animation than 1-tick rotation.
pub fn spinner_frame(tick: u8) -> &'static str {
    SPINNER[(tick / 3) as usize % SPINNER.len()]
}

/// Builds a `Line` showing the current action state (spinner / ✓ / ✕).
/// Returns `None` when the state is `Idle`.
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

/// Returns a `(text, style)` tuple describing the current action state —
/// used in the detail header.
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
