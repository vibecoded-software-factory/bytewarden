//! Renderer for [`crate::tui::screens::Screen::Generator`].

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::ports::GeneratorMode;
use crate::tui::app::App;
use crate::tui::generator::{GeneratorFocus, GeneratorState, ReturnTarget, focusable_for};
use crate::tui::view::action::action_line;
use crate::tui::view::widgets::{focus_color, render_cmd_bar, rounded_block};

/// Renders the generator screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let t = &app.theme;
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    // ── Header ────────────────────────────────────────────────────────────
    let title = match app.generator.options.mode {
        GeneratorMode::Password => " Generate password",
        GeneratorMode::Passphrase => " Generate passphrase",
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            title,
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
        )]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(t.inactive)),
        ),
        chunks[0],
    );

    // ── Body ──────────────────────────────────────────────────────────────
    render_body(frame, app, chunks[1]);

    // ── Footer hints ──────────────────────────────────────────────────────
    let has_target = app.generator.return_target.is_some();
    let (full, short) = footer_hints(has_target);
    render_cmd_bar(frame, area, chunks[2], full, short, t.dim, t);
}

fn footer_hints(has_target: bool) -> (&'static str, &'static str) {
    if has_target {
        (
            "Tab/↑↓ field · ←→/Space change · Enter regenerate · Alt+U use · Alt+C copy · Esc cancel",
            "Tab:field  Enter:regen  Alt+U:use  Alt+C:copy  Esc:back",
        )
    } else {
        (
            "Tab/↑↓ field · ←→/Space change · Enter regenerate · Alt+C copy · Esc back",
            "Tab:field  Enter:regen  Alt+C:copy  Esc:back",
        )
    }
}

fn render_body(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    // Center a fixed-width column so the form looks tidy on wide
    // terminals.
    let body_w = area.width.saturating_sub(8).clamp(48, 78);
    let body = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(body_w),
        Constraint::Fill(1),
    ])
    .split(area)[1];

    let g = &app.generator;
    let focusables = focusable_for(g.options.mode);

    // Per-row layout: 2 padding + N rows of 1 + 1 spacer + 3 result + 2 spinner.
    let mut constraints: Vec<Constraint> = Vec::new();
    constraints.push(Constraint::Length(1)); // top padding
    for _ in 0..focusables.len() - 1 {
        // all controls except Result are 1 row tall
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1)); // spacer above result
    constraints.push(Constraint::Length(3)); // result box
    constraints.push(Constraint::Length(1)); // status line
    constraints.push(Constraint::Min(0));

    let rows = Layout::vertical(constraints).split(body);
    let mut row_idx = 1usize; // skip the top padding row

    for f in focusables.iter() {
        if matches!(f, GeneratorFocus::Result) {
            continue; // result is rendered separately below
        }
        if row_idx >= rows.len() {
            break;
        }
        let focused = *f == g.focus;
        let area = rows[row_idx];
        row_idx += 1;
        render_control(frame, area, g, *f, focused, t);
    }
    // The "spacer above result" sits at row_idx; advance past it.
    row_idx += 1;

    // Result box
    if row_idx < rows.len() {
        let focused = g.focus == GeneratorFocus::Result;
        let bcol = if focused { t.accent } else { t.inactive };
        let value = if g.result.is_empty() {
            "(press Enter to generate)"
        } else {
            g.result.as_str()
        };
        let style = if g.result.is_empty() {
            Style::default().fg(t.dim)
        } else {
            Style::default()
                .fg(t.foreground)
                .add_modifier(Modifier::BOLD)
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(value, style))).block(
                rounded_block(Style::default().fg(bcol)).title(Span::styled(
                    " Result ",
                    Style::default().fg(focus_color(focused, t.accent, t.inactive)),
                )),
            ),
            rows[row_idx],
        );
        row_idx += 1;
    }

    // Status line — surface action_line so success / error flashes are
    // visible without leaving the screen.
    if row_idx < rows.len()
        && let Some(line) = action_line(app)
    {
        frame.render_widget(Paragraph::new(line), rows[row_idx]);
    }
}

fn render_control(
    frame: &mut Frame,
    area: Rect,
    g: &GeneratorState,
    f: GeneratorFocus,
    focused: bool,
    t: &crate::tui::theme::Theme,
) {
    let arrow = if focused { "▶ " } else { "  " };
    let label_style = if focused {
        Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(t.inactive)
    };

    let (label, value): (&str, String) = match f {
        GeneratorFocus::Mode => (
            "Mode",
            match g.options.mode {
                GeneratorMode::Password => "Password   (←→ to switch)".into(),
                GeneratorMode::Passphrase => "Passphrase (←→ to switch)".into(),
            },
        ),
        GeneratorFocus::Length => ("Length", format!("{}  (←→ adjust)", g.options.length)),
        GeneratorFocus::Uppercase => ("Uppercase", checkbox(g.options.uppercase)),
        GeneratorFocus::Lowercase => ("Lowercase", checkbox(g.options.lowercase)),
        GeneratorFocus::Numbers => ("Numbers", checkbox(g.options.numbers)),
        GeneratorFocus::Special => ("Special", checkbox(g.options.special)),
        GeneratorFocus::Ambiguous => ("Avoid ambiguous", checkbox(g.options.avoid_ambiguous)),
        GeneratorFocus::Words => ("Words", format!("{}  (←→ adjust)", g.options.words)),
        GeneratorFocus::Separator => (
            "Separator",
            format!("{:?}  (type to change)", g.options.separator),
        ),
        GeneratorFocus::Capitalize => ("Capitalize", checkbox(g.options.capitalize)),
        GeneratorFocus::IncludeNumber => ("Include number", checkbox(g.options.include_number)),
        GeneratorFocus::Result => return,
    };

    let line = Line::from(vec![
        Span::raw(arrow),
        Span::styled(format!("{label:<18}"), label_style),
        Span::styled(value, Style::default().fg(t.foreground)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn checkbox(on: bool) -> String {
    if on {
        "[✓] on".into()
    } else {
        "[ ] off".into()
    }
}

/// Surfaces the return-target type so the view can show distinct
/// hints. (Currently the view derives this from `app.generator.return_target`
/// directly, but the helper is kept for future use by the input layer.)
#[allow(dead_code)]
pub fn target_kind(target: Option<ReturnTarget>) -> &'static str {
    match target {
        Some(ReturnTarget::EditField(_)) => "edit",
        Some(ReturnTarget::CreateField(_)) => "create",
        None => "standalone",
    }
}
