//! Renderer for the command palette (`Ctrl+P`) — a centered picker
//! modal: a fuzzy query row over a windowed command list, each row with
//! its keybinding right-aligned (the executable cheat-sheet).

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState, Paragraph},
};

use crate::tui::app::App;
use crate::tui::view::widgets::{center_rect, cursor_line, key_style, rounded_block};

/// Draws the command palette over its origin screen.
pub fn draw(frame: &mut Frame, app: &App) {
    let Some(state) = app.palette.as_ref() else {
        return;
    };
    let t = &app.theme;
    let area = center_rect(60, 20, frame.area());
    frame.render_widget(Clear, area);

    let title = format!(" Command palette · {} ", state.filtered.len());
    let block =
        rounded_block(Style::default().fg(t.accent)).title(Span::styled(title, t.emphasis()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::vertical([
        Constraint::Length(1), // query
        Constraint::Length(1), // spacer
        Constraint::Min(1),    // list
        Constraint::Length(1), // hint
    ])
    .split(inner);

    // Query row: `⌕ ` prefix + the editor with its block cursor.
    let mut ql = vec![Span::styled("⌕ ", Style::default().fg(t.dim))];
    ql.extend(cursor_line(state.query.text(), state.query.cursor(), t).spans);
    frame.render_widget(Paragraph::new(Line::from(ql)), rows[0]);

    // Command list — label left, keybinding right-aligned in `key_style`.
    let width = rows[2].width as usize;
    let items: Vec<ListItem> = state
        .filtered
        .iter()
        .filter_map(|&i| state.all.get(i))
        .map(|c| {
            let label_w = c.label.chars().count();
            let keys_w = c.keys.chars().count();
            // 2 for the `▶ ` highlight symbol column + a 1-col gap.
            let pad = width.saturating_sub(2 + label_w + keys_w + 1).max(1);
            ListItem::new(Line::from(vec![
                Span::styled(c.label, Style::default().fg(t.foreground)),
                Span::raw(" ".repeat(pad)),
                Span::styled(c.keys, key_style(t)),
            ]))
        })
        .collect();

    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  no matching command",
                Style::default().fg(t.dim),
            ))),
            rows[2],
        );
    } else {
        let mut ls = ListState::default().with_selected(Some(state.selected));
        frame.render_stateful_widget(
            List::new(items)
                .highlight_style(
                    Style::default()
                        .bg(t.selected_bg)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ "),
            rows[2],
            &mut ls,
        );
    }

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "↑↓ select · Enter run · Esc cancel",
            Style::default().fg(t.dim),
        ))),
        rows[3],
    );
}
