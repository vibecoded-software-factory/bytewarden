//! Vault list screen renderer (sidebar + search + list + cmd-log).

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};

use crate::domain::filter::{ITEM_FILTERS, ItemFilter};
use crate::domain::item::item_type_label;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::screens::Focus;
use crate::tui::view::action::action_line;
use crate::tui::view::widgets::{
    focus_border, focus_color, render_cmd_bar_with_help, titled_block,
};

/// Renders the vault screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let t = &app.theme;
    let area = frame.area();

    let outer = Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).split(area);
    let body = Layout::horizontal([Constraint::Percentage(26), Constraint::Percentage(74)])
        .split(outer[0]);
    let sidebar = Layout::vertical([
        Constraint::Length(3),
        Constraint::Percentage(30),
        Constraint::Min(0),
    ])
    .split(body[0]);
    let cmd_h = if app.cmd_log.is_empty() { 4u16 } else { 9 };
    let main = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(cmd_h),
    ])
    .split(body[1]);

    render_hint_bar(frame, app, area, outer[1]);
    render_status(frame, app, sidebar[0]);
    render_vaults(frame, app, sidebar[1]);
    render_filters(frame, app, sidebar[2]);
    render_search(frame, app, main[0]);
    render_list(frame, app, main[1]);
    render_cmd_log(frame, app, main[2], cmd_h);

    app.mouse_areas.status = Some(sidebar[0]);
    app.mouse_areas.folders = Some(sidebar[1]);
    app.mouse_areas.items = Some(sidebar[2]);
    app.mouse_areas.search = Some(main[0]);
    app.mouse_areas.list = Some(main[1]);
    app.mouse_areas.cmdlog = Some(main[2]);

    let _ = t; // some helpers re-borrow theme; suppress unused-warning in slim builds
}

fn render_hint_bar(
    frame: &mut Frame,
    app: &App,
    area: ratatui::layout::Rect,
    bar: ratatui::layout::Rect,
) {
    let t = &app.theme;
    // Per-focus hints — kept intentionally short. Anything not here
    // (Alt+S sync, Alt+G gen, Alt+E export, Alt+M import, Alt+W send,
    //  Alt+B memberships, Alt+I fingerprint, Alt+L lock, Alt+O logout,
    //  Alt+F favorite, Alt+U username, Alt+R restore in trash, …) is
    // discoverable via F1 — which is anchored at the right of the bar
    // and never truncated.
    let hints_pairs: &[(&str, &str)] = match app.focus {
        Focus::Search => {
            if app.is_trash_view() {
                &[("Esc", "clear"), ("j/k", "nav"), ("Enter", "detail")]
            } else {
                &[
                    ("Esc", "clear"),
                    ("j/k", "nav"),
                    ("Enter", "detail"),
                    ("Alt+N", "new"),
                    ("Alt+C", "pass"),
                ]
            }
        }
        Focus::Items => &[("j/k", "filter"), ("Enter", "apply"), ("Tab", "next")],
        Focus::Folders => &[
            ("j/k", "folder"),
            ("Enter", "apply"),
            ("Alt+N", "new"),
            ("Tab", "next"),
        ],
        Focus::CmdLog => &[("j/k", "scroll"), ("Tab", "next")],
        Focus::List | Focus::Status => {
            if app.is_trash_view() {
                &[("j/k", "nav"), ("Enter", "detail"), ("Alt+R", "restore")]
            } else {
                &[
                    ("j/k", "nav"),
                    ("Enter", "detail"),
                    ("Alt+N", "new"),
                    ("Alt+C", "pass"),
                ]
            }
        }
    };
    let full = hints_pairs
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join("  |  ");
    let short = hints_pairs
        .iter()
        .map(|(k, v)| format!("{k}:{v}"))
        .collect::<Vec<_>>()
        .join("  ");
    render_cmd_bar_with_help(frame, area, bar, &full, &short, t.dim, t);
}

fn render_status(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let t = &app.theme;
    let sf = app.focus == Focus::Status;
    let (title_style, status_line) = match &app.action_state {
        ActionState::Idle => (
            Style::default().fg(focus_color(sf, t.accent, t.inactive)),
            Line::from(""),
        ),
        _ => (
            Style::default().fg(match &app.action_state {
                ActionState::Running(_) => t.accent,
                ActionState::Done(_) => t.success,
                _ => t.error,
            }),
            action_line(app).unwrap_or_else(|| Line::from("")),
        ),
    };
    frame.render_widget(
        Paragraph::new(status_line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(Span::styled("─[0]-Status", title_style))
                .border_style(Style::default().fg(focus_color(sf, t.accent, t.inactive))),
        ),
        area,
    );
}

fn render_vaults(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    use crate::tui::folders::{FolderFilter, row_count};

    let t = &app.theme;
    let ff = app.focus == Focus::Folders;

    // Build the rows: "All folders", "(No folder)", separator, each folder.
    let mut rows: Vec<ListItem> = Vec::with_capacity(row_count(&app.folders) + 1);

    // Row 0 — All folders.
    let all_active = matches!(app.active_folder, FolderFilter::All);
    rows.push(folder_row(
        "  📁 All folders",
        all_active,
        app.items.len(),
        t,
    ));

    // Row 1 — (No folder) — count items where folder_id is None.
    let none_active = matches!(app.active_folder, FolderFilter::NoFolder);
    let no_folder_count = app.items.iter().filter(|i| i.folder_id.is_none()).count();
    rows.push(folder_row(
        "    (No folder)",
        none_active,
        no_folder_count,
        t,
    ));

    // Separator before the named folders.
    rows.push(ListItem::new(Line::from(Span::styled(
        "  ─────────────────",
        Style::default().fg(t.muted),
    ))));

    // One row per folder (alphabetised at load time).
    for folder in &app.folders {
        let active = matches!(&app.active_folder, FolderFilter::Folder(id) if id == &folder.id);
        let count = app
            .items
            .iter()
            .filter(|i| i.folder_id.as_deref() == Some(folder.id.as_str()))
            .count();
        rows.push(folder_row(
            &format!("    {}", folder.name),
            active,
            count,
            t,
        ));
    }

    // The visual selection index has to skip the separator row at
    // position 2 so it lines up with the underlying logical index.
    let display_sel = if app.folder_selected >= 2 {
        app.folder_selected + 1
    } else {
        app.folder_selected
    };
    let mut state = ListState::default();
    state.select(Some(display_sel));

    let total = 2 + app.folders.len();
    let indicator = format!("─{} of {}─", app.folder_selected + 1, total);

    frame.render_stateful_widget(
        List::new(rows)
            .block(titled_block(
                "─[1]-Folders",
                &indicator,
                focus_color(ff, t.accent, t.inactive),
                t,
            ))
            .highlight_style(Style::default().bg(t.selected_bg).fg(t.foreground))
            .highlight_symbol("▶ "),
        area,
        &mut state,
    );
}

fn folder_row<'a>(
    label: &str,
    active: bool,
    count: usize,
    t: &crate::tui::theme::Theme,
) -> ListItem<'a> {
    let style = if active {
        Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(t.foreground)
    };
    ListItem::new(Line::from(vec![
        Span::styled(label.to_string(), style),
        Span::styled(format!("  {count}"), Style::default().fg(t.dim)),
    ]))
}

fn render_filters(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let t = &app.theme;
    let itf = app.focus == Focus::Items;

    let filter_items: Vec<ListItem> = ITEM_FILTERS
        .iter()
        .map(|f| {
            let count = app.count_for(f);
            let col = match f {
                ItemFilter::Login => t.item_login,
                ItemFilter::Card => t.item_card,
                ItemFilter::Identity => t.item_identity,
                ItemFilter::SecureNote => t.item_note,
                ItemFilter::SshKey => t.item_ssh,
                ItemFilter::Favorites => t.item_favorite,
                ItemFilter::Trash => t.error,
                ItemFilter::All => t.foreground,
            };
            let icon = match f {
                ItemFilter::All => "  ",
                ItemFilter::Favorites => "★ ",
                ItemFilter::Login => "󰌋 ",
                ItemFilter::Card => "󰻷 ",
                ItemFilter::Identity => "󰀉 ",
                ItemFilter::SecureNote => "󰎞 ",
                ItemFilter::SshKey => "󰣀 ",
                ItemFilter::Trash => "󰩺 ",
            };
            let active = *f == app.active_filter;
            let style = if active {
                Style::default().fg(col).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(col)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {icon}{}", f.label()), style),
                Span::styled(format!("  {count}"), Style::default().fg(t.dim)),
            ]))
        })
        .collect();

    // Inject a visual separator immediately before the Trash entry.
    let mut filter_items_with_sep: Vec<ListItem> = Vec::with_capacity(filter_items.len() + 1);
    for (i, item) in filter_items.into_iter().enumerate() {
        if i == ITEM_FILTERS.len() - 1 {
            filter_items_with_sep.push(ListItem::new(Line::from(Span::styled(
                "  ─────────────────",
                Style::default().fg(t.muted),
            ))));
        }
        filter_items_with_sep.push(item);
    }

    let mut state = ListState::default();
    let display_sel = if app.filter_selected == ITEM_FILTERS.len() - 1 {
        app.filter_selected + 1 // skip the separator row
    } else {
        app.filter_selected
    };
    state.select(Some(display_sel));
    let indicator = format!("{} of {}", app.filter_selected + 1, ITEM_FILTERS.len());

    frame.render_stateful_widget(
        List::new(filter_items_with_sep)
            .block(titled_block(
                "─[2]-Items",
                &format!("─{indicator}─"),
                focus_color(itf, t.accent, t.inactive),
                t,
            ))
            .highlight_style(Style::default().bg(t.selected_bg).fg(t.foreground))
            .highlight_symbol("▶ "),
        area,
        &mut state,
    );
}

fn render_search(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let t = &app.theme;
    let sf = app.focus == Focus::Search;
    let line = if sf {
        Line::from(vec![
            Span::styled(app.search_query.as_str(), Style::default().fg(t.foreground)),
            Span::styled("█", Style::default().fg(t.accent)),
        ])
    } else if !app.search_query.is_empty() {
        Line::from(Span::styled(
            app.search_query.as_str(),
            Style::default().fg(t.dim),
        ))
    } else {
        Line::from(Span::styled(
            "type to filter…",
            Style::default().fg(t.placeholder),
        ))
    };
    frame.render_widget(
        Paragraph::new(line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(Span::styled(
                    "─[/]-Search",
                    Style::default().fg(focus_color(sf, t.accent, t.inactive)),
                ))
                .border_style(focus_border(sf, t.accent)),
        ),
        area,
    );
}

fn render_list(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let t = &app.theme;
    let lf = app.focus == Focus::List;
    let filtered = app.filtered_items();
    let list_items: Vec<ListItem> = filtered
        .iter()
        .map(|item| {
            let col = match item.item_type {
                1 => t.item_login,
                2 => t.item_note,
                3 => t.item_card,
                4 => t.item_identity,
                5 => t.item_ssh,
                _ => t.dim,
            };
            ListItem::new(Line::from(vec![
                if item.favorite {
                    Span::styled("★ ", Style::default().fg(t.item_favorite))
                } else {
                    Span::raw("  ")
                },
                Span::styled(
                    format!("{:<14}", format!("[{}]", item_type_label(item.item_type))),
                    Style::default().fg(col),
                ),
                Span::raw(item.name.as_str()),
            ]))
        })
        .collect();
    let flen = filtered.len();
    let mut state = ListState::default();
    state.select(if flen == 0 {
        None
    } else {
        Some(app.selected_index)
    });
    let indicator = if flen > 0 {
        format!("{} of {}", app.selected_index + 1, flen)
    } else {
        "0 of 0".into()
    };
    frame.render_stateful_widget(
        List::new(list_items)
            .block(titled_block(
                "─[3]-Vault",
                &format!("─{indicator}─"),
                focus_color(lf, t.accent, t.inactive),
                t,
            ))
            .highlight_style(
                Style::default()
                    .bg(t.selected_bg)
                    .fg(t.foreground)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ "),
        area,
        &mut state,
    );
}

fn render_cmd_log(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, cmd_h: u16) {
    let t = &app.theme;
    let clf = app.focus == Focus::CmdLog;
    let all_log: Vec<Line> = if app.cmd_log.is_empty() {
        vec![Line::from(Span::styled(
            "  no commands yet",
            Style::default().fg(t.dim),
        ))]
    } else {
        app.cmd_log
            .iter()
            .flat_map(|e| {
                let col = if e.ok { t.success } else { t.error };
                let icon = if e.ok { "✓" } else { "✕" };
                vec![
                    Line::from(Span::styled(
                        format!("  $ {}", e.cmd),
                        Style::default().fg(t.dim),
                    )),
                    Line::from(Span::styled(
                        format!("  {icon} {}", e.detail),
                        Style::default().fg(col),
                    )),
                ]
            })
            .collect()
    };
    let visible = cmd_h.saturating_sub(2) as usize;
    let end = all_log.len().saturating_sub(app.cmd_log_scroll);
    let start = end.saturating_sub(visible);
    let title = if app.cmd_log_scroll > 0 {
        "─[4]-Command Log  ↑"
    } else {
        "─[4]-Command Log"
    };
    frame.render_widget(
        Paragraph::new(all_log[start..end].to_vec()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(Span::styled(
                    title,
                    Style::default().fg(focus_color(clf, t.accent, t.inactive)),
                ))
                .border_style(Style::default().fg(focus_color(clf, t.accent, t.inactive))),
        ),
        area,
    );
}
