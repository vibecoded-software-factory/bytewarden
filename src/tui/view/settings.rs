use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use crate::tui::app::App;
use crate::tui::settings_overlay::{SettingsFocus, SettingsSection};
use crate::tui::theme;
use crate::tui::view::widgets::{center_rect_abs, draw_scrollbar, legend_line};

const MIN_W: u16 = 50;
const MAX_W: u16 = 72;
const MIN_H: u16 = 12;
const MAX_H: u16 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsHit {
    Section(usize),
    Row(usize),
    Theme(usize),
}

thread_local! {

    static SETTINGS_HITS: std::cell::RefCell<Vec<(Rect, SettingsHit)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn register_hit(rect: Rect, hit: SettingsHit) {
    if rect.width > 0 && rect.height > 0 {
        SETTINGS_HITS.with(|h| h.borrow_mut().push((rect, hit)));
    }
}

pub fn settings_hit_at(column: u16, row: u16) -> Option<SettingsHit> {
    SETTINGS_HITS.with(|h| {
        h.borrow()
            .iter()
            .rev()
            .find(|(r, _)| {
                column >= r.x && column < r.x + r.width && row >= r.y && row < r.y + r.height
            })
            .map(|(_, t)| *t)
    })
}

fn row_rect(area: Rect, line: u16) -> Rect {
    Rect::new(area.x, area.y + line, area.width, 1)
}

pub fn draw_popup(frame: &mut Frame, area: Rect, app: &App) {
    SETTINGS_HITS.with(|h| h.borrow_mut().clear());
    let t = &app.theme;
    let accent = Style::default().fg(t.accent).add_modifier(Modifier::BOLD);

    let popup = center_rect_abs(
        area.width.saturating_sub(6).clamp(MIN_W, MAX_W),
        area.height.saturating_sub(4).clamp(MIN_H, MAX_H),
        area,
    );

    frame.render_widget(Clear, popup);
    crate::tui::view::widgets::register_modal(popup);
    let outer = Block::default()
        .title(Span::styled(" Settings ", accent))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let inner = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(2),
        height: inner.height,
    };

    let described = focused_row_hint(app);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(u16::from(described.is_some())),
            Constraint::Length(1),
        ])
        .split(inner);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(16), Constraint::Min(10)])
        .split(rows[0]);

    draw_sidebar(frame, app, cols[0]);
    draw_panel(frame, app, cols[1]);

    let hint: &[(&str, &str)] = match app.settings_ui.focus {
        SettingsFocus::Sidebar => &[("↑/↓", "section"), ("→/Enter", "open"), ("Esc", "close")],
        SettingsFocus::Panel => match SettingsSection::ALL[app.settings_ui.section] {
            SettingsSection::Theme => &[
                ("↑/↓", "preview"),
                ("Enter", "apply+save"),
                ("←/Tab", "back"),
                ("Esc", "cancel"),
            ],
            _ => &[
                ("↑/↓", "row"),
                ("←/→", "change"),
                ("Tab", "back"),
                ("Esc", "close"),
            ],
        },
    };
    if let Some(description) = described {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {description}"),
                Style::default().fg(t.dim).add_modifier(Modifier::ITALIC),
            ))),
            rows[1],
        );
    }
    let mut hints = legend_line(hint, rows[2].width, t);
    hints.spans.insert(0, Span::raw(" "));
    frame.render_widget(Paragraph::new(hints), rows[2]);
}

fn focused_row_hint(app: &App) -> Option<&'static str> {
    if app.settings_ui.focus != SettingsFocus::Panel {
        return None;
    }
    let rows = SettingsSection::ALL[app.settings_ui.section].rows();
    rows.get(app.settings_ui.row.min(rows.len().saturating_sub(1)))
        .map(|r| r.hint())
}

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
        .border_style(
            Style::default()
                .fg(color)
                .add_modifier(t.focus_mark(focused)),
        )
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

    for i in 0..SettingsSection::ALL.len() {
        register_hit(row_rect(body, i as u16), SettingsHit::Section(i));
    }
}

fn draw_panel(frame: &mut Frame, app: &App, area: Rect) {
    match SettingsSection::ALL[app.settings_ui.section] {
        SettingsSection::Theme => draw_theme_panel(frame, app, area),
        section => draw_rows_panel(frame, app, area, section),
    }
}

fn draw_rows_panel(frame: &mut Frame, app: &App, area: Rect, section: SettingsSection) {
    let t = &app.theme;
    let focused = app.settings_ui.focus == SettingsFocus::Panel;
    let block = focus_block(app, section.label(), focused);
    let body = block.inner(area);
    frame.render_widget(block, area);

    let rows = section.rows();
    let sel = app.settings_ui.row.min(rows.len().saturating_sub(1));
    let label_w = rows
        .iter()
        .map(|r| Span::raw(r.label()).width())
        .max()
        .unwrap_or(0);

    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(i, &r)| {
            let selected = i == sel;
            let marker = if selected { "▶ " } else { "  " };
            let name_style = if selected && focused {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else if selected {
                Style::default()
                    .fg(t.foreground)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.foreground)
            };
            let value_style = if selected {
                Style::default().fg(t.accent)
            } else {
                Style::default().fg(t.dim)
            };
            Line::from(vec![
                Span::styled(
                    format!(
                        "{marker}{}{}  ",
                        r.label(),
                        " ".repeat(label_w.saturating_sub(Span::raw(r.label()).width()))
                    ),
                    name_style,
                ),
                Span::styled(app.settings_row_value(r), value_style),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), body);

    for i in 0..rows.len() {
        register_hit(row_rect(body, i as u16), SettingsHit::Row(i));
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
    let vh = body.height as usize;

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

    for i in start..end {
        register_hit(row_rect(body, (i - start) as u16), SettingsHit::Theme(i));
    }

    crate::tui::view::widgets::register_scroll(
        body,
        crate::tui::view::widgets::ScrollTarget::SettingsTheme,
    );

    draw_scrollbar(frame, area, total, idx, t);
}
