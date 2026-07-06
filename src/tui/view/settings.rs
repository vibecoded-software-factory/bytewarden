//! Settings overlay renderer — a section sidebar plus the active
//! section's panel, centered over the originating screen. Today the only
//! section is Theme (a live-previewing preset picker); new sections slot
//! into the sidebar without changing the layout.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use crate::tui::app::App;
use crate::tui::settings_overlay::{SettingsFocus, SettingsSection};
use crate::tui::theme;
use crate::tui::view::widgets::draw_scrollbar;

pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let accent = Style::default().fg(t.accent).add_modifier(Modifier::BOLD);

    let w = area.width.saturating_sub(6).clamp(50, 72);
    let h = area.height.saturating_sub(4).clamp(12, 24);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let popup = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    frame.render_widget(Clear, popup);
    let outer = Block::default()
        .title(Span::styled(" Settings ", accent))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(inner);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(16), Constraint::Min(10)])
        .split(rows[0]);

    draw_sidebar(frame, app, cols[0]);
    draw_panel(frame, app, cols[1]);

    let hint = match app.settings_ui.focus {
        SettingsFocus::Sidebar => "↑/↓ section · →/Enter open · Esc close",
        SettingsFocus::Panel => "↑/↓ preview · Enter apply+save · ←/Tab back · Esc cancel",
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(hint, Style::default().fg(t.dim)))),
        rows[1],
    );
}

/// One bordered block whose title + border go accent+bold when focused,
/// else inactive — the same focus affordance as the main screens.
fn focus_block(app: &App, title: &str, focused: bool) -> Block<'static> {
    let t = &app.theme;
    let color = if focused { t.accent } else { t.inactive };
    let mut title_style = Style::default().fg(color);
    if focused {
        title_style = title_style.add_modifier(Modifier::BOLD);
    }
    Block::default()
        .title(Span::styled(format!(" {title} "), title_style))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
}

fn draw_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let focused = app.settings_ui.focus == SettingsFocus::Sidebar;
    let block = focus_block(app, "Sections", focused);
    let body = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = SettingsSection::ALL
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let selected = i == app.settings_ui.section;
            let marker = if selected { "▶ " } else { "  " };
            let style = if selected && focused {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else if selected {
                Style::default().fg(t.foreground)
            } else {
                Style::default().fg(t.dim)
            };
            Line::from(Span::styled(format!("{marker}{}", s.label()), style))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), body);
}

fn draw_panel(frame: &mut Frame, app: &App, area: Rect) {
    match SettingsSection::ALL[app.settings_ui.section] {
        SettingsSection::Theme => draw_theme_panel(frame, app, area),
    }
}

fn draw_theme_panel(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let focused = app.settings_ui.focus == SettingsFocus::Panel;
    let block = focus_block(app, "Theme", focused);
    let body = block.inner(area);
    frame.render_widget(block, area);

    let presets = theme::Preset::ALL;
    let total = presets.len();
    let idx = app.settings_ui.theme_idx.min(total.saturating_sub(1));
    let vh = body.height as usize; // preset rows that fit in the panel

    // Window the list so the highlighted preset is always on screen: it
    // sits at (or above) the last visible row once we've scrolled past
    // the first page, and the window never overshoots the end.
    let start = idx
        .saturating_sub(vh.saturating_sub(1))
        .min(total.saturating_sub(vh));
    let end = (start + vh).min(total);

    let lines: Vec<Line> = presets[start..end]
        .iter()
        .enumerate()
        .map(|(vis, p)| {
            let i = start + vis;
            let selected = i == idx;
            let marker = if selected { "▶ " } else { "  " };
            let style = if selected {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.foreground)
            };
            Line::from(Span::styled(format!("{marker}{}", p.label()), style))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), body);

    // Scroll cue on the panel's right border when the presets overflow.
    draw_scrollbar(frame, area, total, idx, t);
}
