use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, List, ListItem, ListState, Paragraph, Row},
};

use crate::domain::filter::{ITEM_FILTERS, ItemFilter};
use crate::domain::item::item_type_label;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::screens::Focus;
use crate::tui::view::action::action_line;
use crate::tui::view::widgets::{
    self, cmdlog_height, draw_scrollbar, empty_state_lines, favorite_star, focus_color,
    render_cmd_bar_with_help, titled_block, titled_block_styled,
};

thread_local! {

    static VAULT_LIST_OFFSET: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub fn vault_list_offset() -> usize {
    VAULT_LIST_OFFSET.with(|o| o.get())
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let t = &app.theme;
    let area = frame.area();

    let cmd_h = cmdlog_height(area.height);
    let outer = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(cmd_h),
        Constraint::Length(1),
    ])
    .split(area);
    let body = Layout::horizontal([Constraint::Percentage(26), Constraint::Percentage(74)])
        .split(outer[0]);

    let folder_rows = 3 + app.folders.len() + app.collections.len();
    let folders_h = (folder_rows as u16 + 2).clamp(5, 14);
    let sidebar = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(folders_h),
        Constraint::Min(0),
    ])
    .split(body[0]);
    let main = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(body[1]);

    render_hint_bar(frame, app, outer[2]);
    render_status(frame, app, sidebar[0]);
    render_vaults(frame, app, sidebar[1]);
    render_filters(frame, app, sidebar[2]);
    render_search(frame, app, main[0]);
    render_list(frame, app, main[1]);
    render_cmd_log(frame, app, outer[1], cmd_h);

    app.mouse_areas.status = Some(sidebar[0]);
    app.mouse_areas.folders = Some(sidebar[1]);
    app.mouse_areas.items = Some(sidebar[2]);
    app.mouse_areas.search = Some(main[0]);
    app.mouse_areas.list = Some(main[1]);
    app.mouse_areas.cmdlog = Some(outer[1]);

    use crate::tui::view::widgets::{ScrollTarget, register_scroll};
    register_scroll(main[1], ScrollTarget::Vault);
    register_scroll(sidebar[1], ScrollTarget::Folders);
    register_scroll(sidebar[2], ScrollTarget::Filters);
    register_scroll(outer[1], ScrollTarget::CmdLog);

    let _ = t;
}

fn render_hint_bar(frame: &mut Frame, app: &App, bar: ratatui::layout::Rect) {
    let t = &app.theme;

    let hints_pairs: &[(&str, &str)] = match app.focus {
        Focus::Search => {
            if app.vault.is_trash_view() {
                &[("Esc", "clear"), ("↑↓", "nav"), ("Enter", "open")]
            } else {
                &[
                    ("Esc", "clear"),
                    ("↑↓", "nav"),
                    ("Enter", "open"),
                    ("Alt+N", "new"),
                    ("Alt+C", "pass"),
                ]
            }
        }
        Focus::Items => &[("j/k", "filter"), ("Enter", "apply"), ("Tab", "next")],

        Focus::Folders => &[
            ("j/k", "folder"),
            ("Enter", "apply"),
            ("n", "new"),
            ("r", "rename"),
        ],
        Focus::CmdLog => &[("j/k", "scroll"), ("Tab", "next")],

        Focus::List | Focus::Status => {
            if app.vault.is_trash_view() {
                &[("j/k", "nav"), ("Enter", "open"), ("r", "restore")]
            } else {
                &[
                    ("j/k", "nav"),
                    ("Enter", "open"),
                    ("n", "new"),
                    ("c", "pass"),
                ]
            }
        }
    };
    render_cmd_bar_with_help(frame, bar, hints_pairs, t);
}

fn render_status(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let t = &app.theme;
    let sf = app.focus == Focus::Status;
    let (title_style, status_line) = if app.worker_dead {
        (
            t.danger_title(),
            Line::from(Span::styled(
                format!("{} WORKER DEAD", app.icons.warning()),
                t.danger_title(),
            )),
        )
    } else {
        match &app.action_state {
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
        }
    };

    frame.render_widget(
        Paragraph::new(status_line).block(titled_block_styled(
            "─[0]-Status",
            title_style,
            "",
            sf,
            t,
        )),
        area,
    );
}

fn render_vaults(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    use crate::tui::folders::{FolderFilter, row_count};

    let t = &app.theme;
    let ff = app.focus == Focus::Folders;

    let mut rows: Vec<ListItem> = Vec::with_capacity(row_count(&app.folders, &app.collections) + 1);

    let all_active = matches!(app.vault.active_folder, FolderFilter::All);
    rows.push(folder_row(
        &format!("  {} All folders", app.icons.folder()),
        all_active,
        app.vault.items.len(),
        t,
    ));

    let none_active = matches!(app.vault.active_folder, FolderFilter::NoFolder);
    rows.push(folder_row(
        "    (No folder)",
        none_active,
        app.vault.no_folder_count,
        t,
    ));

    rows.push(separator_row(area.width, t));

    for folder in &app.folders {
        let active =
            matches!(&app.vault.active_folder, FolderFilter::Folder(id) if id == &folder.id);
        let count = app
            .vault
            .folder_counts
            .get(&folder.id)
            .copied()
            .unwrap_or(0);
        rows.push(folder_row(
            &format!("  {} {}", app.icons.folder(), folder.name),
            active,
            count,
            t,
        ));
    }

    for collection in &app.collections {
        let active = matches!(&app.vault.active_folder, FolderFilter::Collection(id) if id == &collection.id);
        let count = app
            .vault
            .collection_counts
            .get(&collection.id)
            .copied()
            .unwrap_or(0);
        let org_name = collection
            .organization_id
            .as_deref()
            .and_then(|id| app.organizations.iter().find(|o| o.id == id))
            .map(|o| o.name.as_str());
        let mark = app.icons.collection();
        let label = match org_name {
            Some(org) => format!("  {mark} {org} / {}", collection.name),
            None => format!("  {mark} {}", collection.name),
        };
        rows.push(folder_row(&label, active, count, t));
    }

    let display_sel = if app.vault.folder_selected >= 2 {
        app.vault.folder_selected + 1
    } else {
        app.vault.folder_selected
    };
    let mut state = ListState::default();
    state.select(Some(display_sel));

    let total = row_count(&app.folders, &app.collections);
    let indicator = format!("{} of {}", app.vault.folder_selected + 1, total);

    let display_rows = rows.len();
    frame.render_stateful_widget(
        List::new(rows)
            .block(titled_block("─[1]-Folders", &indicator, ff, t))
            .highlight_style(
                Style::default()
                    .bg(t.selected_bg)
                    .fg(t.foreground)
                    .add_modifier(t.select_mark()),
            )
            .highlight_symbol("▶ "),
        area,
        &mut state,
    );

    draw_scrollbar(frame, area, display_rows, display_sel, t);
}

fn separator_row<'a>(width: u16, t: &crate::tui::theme::Theme) -> ListItem<'a> {
    let w = (width as usize).saturating_sub(4);
    ListItem::new(Line::from(Span::styled(
        "┈".repeat(w),
        Style::default().fg(t.muted),
    )))
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
            let count = app.vault.count_for(f);
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
                ItemFilter::All => " ",
                ItemFilter::Favorites => "★",
                ItemFilter::Login => app.icons.login(),
                ItemFilter::Card => app.icons.card(),
                ItemFilter::Identity => app.icons.identity(),
                ItemFilter::SecureNote => app.icons.note(),
                ItemFilter::SshKey => app.icons.ssh(),
                ItemFilter::Trash => app.icons.trash(),
            };
            let active = *f == app.vault.active_filter;
            let style = if active {
                Style::default().fg(col).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(col)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {icon} {}", f.label()), style),
                Span::styled(format!("  {count}"), Style::default().fg(t.dim)),
            ]))
        })
        .collect();

    let mut filter_items_with_sep: Vec<ListItem> = Vec::with_capacity(filter_items.len() + 1);
    for (i, item) in filter_items.into_iter().enumerate() {
        if i == ITEM_FILTERS.len() - 1 {
            filter_items_with_sep.push(separator_row(area.width, t));
        }
        filter_items_with_sep.push(item);
    }

    let mut state = ListState::default();
    let display_sel = if app.vault.filter_selected == ITEM_FILTERS.len() - 1 {
        app.vault.filter_selected + 1
    } else {
        app.vault.filter_selected
    };
    state.select(Some(display_sel));
    let indicator = format!(
        "{} of {}",
        app.vault.filter_selected + 1,
        ITEM_FILTERS.len()
    );

    let display_rows = filter_items_with_sep.len();
    frame.render_stateful_widget(
        List::new(filter_items_with_sep)
            .block(titled_block("─[2]-Items", &indicator, itf, t))
            .highlight_style(
                Style::default()
                    .bg(t.selected_bg)
                    .fg(t.foreground)
                    .add_modifier(t.select_mark()),
            )
            .highlight_symbol("▶ "),
        area,
        &mut state,
    );

    draw_scrollbar(frame, area, display_rows, display_sel, t);
}

fn render_search(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let t = &app.theme;
    let sf = app.focus == Focus::Search;

    let line = if sf {
        let mut spans = vec![Span::styled(
            format!(" {} ", app.icons.search()),
            Style::default().fg(t.foreground),
        )];
        spans.extend(widgets::editor_spans(&app.vault.search_query, true, t));
        Line::from(spans)
    } else if !app.vault.search_query.is_empty() {
        Line::from(vec![
            Span::styled(
                format!(" {} ", app.icons.search()),
                Style::default().fg(t.dim),
            ),
            Span::styled(app.vault.search_query.text(), Style::default().fg(t.dim)),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                format!(" {} ", app.icons.search()),
                Style::default().fg(t.placeholder),
            ),
            Span::styled("type to filter…", Style::default().fg(t.placeholder)),
        ])
    };

    frame.render_widget(
        Paragraph::new(line).block(titled_block("─[/]-Search", "", sf, t)),
        area,
    );
}

fn list_type_label(item_type: u8) -> &'static str {
    match item_type {
        2 => "Note",
        4 => "Ident",
        5 => "SSH",
        other => item_type_label(other),
    }
}

fn render_list(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let t = &app.theme;
    let lf = app.focus == Focus::List;
    let filtered = app.vault.filtered_items();

    let (mut any_fav, mut any_reprompt, mut any_org) = (false, false, false);

    let mut type_w = 0usize;
    for it in filtered.iter() {
        any_fav |= it.favorite;
        any_reprompt |= it.needs_reprompt();
        any_org |= it.organization_id.is_some();
        type_w = type_w.max(list_type_label(it.item_type).len() + 2);
    }
    let ind_w = (any_fav as u16) * 2 + (any_reprompt as u16) * 2 + (any_org as u16) * 2;
    let type_w = type_w.clamp(6, 14) as u16;

    let rows: Vec<Row> = filtered
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

            let mut spans: Vec<Span> = Vec::with_capacity(4);
            if any_fav {
                spans.push(if item.favorite {
                    favorite_star(t)
                } else {
                    Span::raw("  ")
                });
            }
            if any_reprompt {
                spans.push(if item.needs_reprompt() {
                    Span::styled(
                        format!("{} ", app.icons.locked()),
                        Style::default().fg(t.error),
                    )
                } else {
                    Span::raw("  ")
                });
            }
            if any_org {
                spans.push(if item.organization_id.is_some() {
                    Span::styled(
                        format!("{} ", app.icons.collection()),
                        Style::default().fg(t.accent),
                    )
                } else {
                    Span::raw("  ")
                });
            }
            spans.push(Span::styled(
                format!("[{}]", list_type_label(item.item_type)),
                Style::default().fg(col),
            ));
            Row::new(vec![
                Cell::from(Line::from(spans)),
                Cell::from(Span::raw(item.name.as_str())),
            ])
        })
        .collect();

    let flen = filtered.len();
    let sel = (flen > 0).then_some(app.vault.selected_index.min(flen.saturating_sub(1)));
    let indicator = if flen > 0 {
        format!("{} of {}", app.vault.selected_index + 1, flen)
    } else {
        "0 of 0".into()
    };

    let empty = if !app.vault.search_query.is_empty() {
        empty_state_lines("No items match", &["Esc clears the search"], t)
    } else if app.vault.is_trash_view() {
        empty_state_lines(
            "Trash is empty",
            &["deleted items land here (x on an item)"],
            t,
        )
    } else {
        empty_state_lines(
            "No items in this view",
            &["Alt+N creates an item", "Alt+S syncs the vault"],
            t,
        )
    };
    let offset = widgets::list_table(
        frame,
        t,
        area,
        widgets::ListTable {
            title: "─[3]-Vault",
            counter: indicator,
            focused: lf,
            headers: None,
            widths: vec![Constraint::Length(ind_w + type_w), Constraint::Min(0)],
            rows,
            selected: sel,
            empty,
        },
    );

    VAULT_LIST_OFFSET.with(|o| o.set(offset));
}

fn render_cmd_log(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, cmd_h: u16) {
    let t = &app.theme;
    let clf = app.focus == Focus::CmdLog;
    let visible = (cmd_h as usize).saturating_sub(2);
    let total = app.cmd_log.entries.len();

    let scroll = app.cmd_log.scroll.min(total.saturating_sub(visible));

    let counter = if total == 0 {
        String::new()
    } else {
        format!("{} of {}", total - scroll, total)
    };
    let block = titled_block("─[4]-Command Log", &counter, clf, t);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if total == 0 {
        frame.render_widget(
            Paragraph::new(empty_state_lines(
                "No commands yet",
                &[
                    "every bw call lands here — session keys redacted",
                    "Alt+S runs one · j/k scrolls this panel",
                ],
                t,
            )),
            inner,
        );
        return;
    }
    let end = total - scroll;
    let start = end.saturating_sub(visible);
    let lines: Vec<Line> = app.cmd_log.entries[start..end]
        .iter()
        .map(|e| {
            let mark = if e.ok { "✓" } else { "✗" };
            let mark_style = Style::default().fg(if e.ok { t.success } else { t.error });
            Line::from(vec![
                Span::styled(format!("  {mark} "), mark_style),
                Span::styled(e.cmd.clone(), Style::default().fg(t.foreground)),
                Span::styled(format!("  →  {}", e.detail), Style::default().fg(t.dim)),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);

    draw_scrollbar(frame, area, total, end.saturating_sub(1), t);
}
