//! Vault list screen renderer (sidebar + search + list + cmd-log).

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table,
        TableState,
    },
};

use crate::domain::filter::{ITEM_FILTERS, ItemFilter};
use crate::domain::item::item_type_label;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::screens::Focus;
use crate::tui::view::action::action_line;
use crate::tui::view::widgets::{
    cmdlog_height, draw_scrollbar, focus_border, focus_color, render_cmd_bar_with_help,
    titled_block,
};

/// Renders the vault screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let t = &app.theme;
    let area = frame.area();

    // Command-log height: 6 rows (2 border + 4 visible entries). The log
    // is a full-width row at the bottom (spanning sidebar + main), above
    // the hint bar.
    let cmd_h = cmdlog_height(area.height);
    let outer = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(cmd_h),
        Constraint::Length(1),
    ])
    .split(area);
    let body = Layout::horizontal([Constraint::Percentage(26), Constraint::Percentage(74)])
        .split(outer[0]);
    // Folders is sized to its content (small box at the top); the Items
    // filter fills the rest of the column, its border reaching the bottom
    // with the list top-aligned, so there's no dead gutter below the
    // sidebar.
    let folder_rows = 3 + app.folders.len() + app.collections.len();
    let folders_h = (folder_rows as u16 + 2).clamp(5, 14);
    let sidebar = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(folders_h),
        Constraint::Min(0),
    ])
    .split(body[0]);
    let main = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(body[1]);

    render_hint_bar(frame, app, area, outer[2]);
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

    // Wheel targets (position-aware): the item list, the filter sidebar and the
    // command log each scroll their own state.
    use crate::tui::view::widgets::{ScrollTarget, register_scroll};
    register_scroll(main[1], ScrollTarget::Vault);
    register_scroll(sidebar[2], ScrollTarget::Filters);
    register_scroll(outer[1], ScrollTarget::CmdLog);

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
        // On Search the box owns typing, so ↑↓ navigate (not j/k), and
        // row actions ride on `Alt+` (the gradient's text-field rule).
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
        // Bare letters act on the focused Folders panel.
        Focus::Folders => &[
            ("j/k", "folder"),
            ("Enter", "apply"),
            ("n", "new"),
            ("r", "rename"),
        ],
        Focus::CmdLog => &[("j/k", "scroll"), ("Tab", "next")],
        // Bare letters act on the focused row (the gradient).
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
    // `key action` pairs joined with ` · ` — a compact footer that reads
    // tighter than the old `key: action` joined with a spaced pipe.
    let full = hints_pairs
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect::<Vec<_>>()
        .join(" · ");
    render_cmd_bar_with_help(frame, area, bar, &full, &full, t.dim, t);
}

fn render_status(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let t = &app.theme;
    let sf = app.focus == Focus::Status;
    let (title_style, status_line) = if app.worker_dead {
        // Persistent condition badge — unlike the sticky error toast
        // (which the next keypress clears) this stays as long as the
        // condition holds, because the worker is dead until restart.
        (
            t.danger_title(),
            Line::from(Span::styled("⚠ WORKER DEAD", t.danger_title())),
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

    // Build the rows: "All folders", "(No folder)", separator,
    // folders prefixed `📁`, collections prefixed `👥`. Folders and
    // collections share the same scrolling list — the icon prefix
    // tells the user which they're picking. An item can only belong
    // to one folder but several collections, so collection rows
    // commonly overlap with folder rows in terms of which items
    // they surface.
    let mut rows: Vec<ListItem> = Vec::with_capacity(row_count(&app.folders, &app.collections) + 1);

    // Row 0 — All folders.
    let all_active = matches!(app.vault.active_folder, FolderFilter::All);
    rows.push(folder_row(
        "  📁 All folders",
        all_active,
        app.vault.items.len(),
        t,
    ));

    // Row 1 — (No folder). Count is precomputed in
    // `Vault::rebuild_sidebar_counts` to avoid an O(items) scan per frame.
    let none_active = matches!(app.vault.active_folder, FolderFilter::NoFolder);
    rows.push(folder_row(
        "    (No folder)",
        none_active,
        app.vault.no_folder_count,
        t,
    ));

    // Muted rule before the named folder/collection rows — an explicit
    // group divider rather than a blank gap.
    rows.push(separator_row(area.width, t));

    // One row per folder (alphabetised at load time). Per-folder
    // count comes from the precomputed map — see
    // `Vault::rebuild_sidebar_counts`.
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
            &format!("  📁 {}", folder.name),
            active,
            count,
            t,
        ));
    }

    // One row per collection — labelled `Org / Name` so members of
    // multiple organisations can tell sibling collections apart.
    // Personal-only accounts skip this section entirely. Same
    // precomputed-count rationale as the folder rows above.
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
        let label = match org_name {
            Some(org) => format!("  👥 {org} / {}", collection.name),
            None => format!("  👥 {}", collection.name),
        };
        rows.push(folder_row(&label, active, count, t));
    }

    // The visual selection index has to skip the separator row at
    // position 2 so it lines up with the underlying logical index.
    let display_sel = if app.vault.folder_selected >= 2 {
        app.vault.folder_selected + 1
    } else {
        app.vault.folder_selected
    };
    let mut state = ListState::default();
    state.select(Some(display_sel));

    let total = row_count(&app.folders, &app.collections);
    let indicator = format!("{} of {}", app.vault.folder_selected + 1, total);

    frame.render_stateful_widget(
        List::new(rows)
            .block(titled_block("─[1]-Folders", &indicator, ff, t))
            .highlight_style(Style::default().bg(t.selected_bg).fg(t.foreground))
            .highlight_symbol("▶ "),
        area,
        &mut state,
    );
}

/// A muted dotted rule spanning the panel, used as a group divider in
/// the sidebar lists (before the named folders, before Trash) — an
/// explicit separator instead of a blank gap. `width` is the panel's
/// outer width; the rule insets by the 2-cell highlight-symbol gutter
/// every row reserves so it lines up with the row content.
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
                ItemFilter::All => "  ",
                ItemFilter::Favorites => "★ ",
                ItemFilter::Login => "󰌋 ",
                ItemFilter::Card => "󰻷 ",
                ItemFilter::Identity => "󰀉 ",
                ItemFilter::SecureNote => "󰎞 ",
                ItemFilter::SshKey => "󰣀 ",
                ItemFilter::Trash => "󰩺 ",
            };
            let active = *f == app.vault.active_filter;
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
            // Muted rule before the Trash entry — an explicit divider.
            filter_items_with_sep.push(separator_row(area.width, t));
        }
        filter_items_with_sep.push(item);
    }

    let mut state = ListState::default();
    let display_sel = if app.vault.filter_selected == ITEM_FILTERS.len() - 1 {
        app.vault.filter_selected + 1 // skip the separator row
    } else {
        app.vault.filter_selected
    };
    state.select(Some(display_sel));
    let indicator = format!(
        "{} of {}",
        app.vault.filter_selected + 1,
        ITEM_FILTERS.len()
    );

    frame.render_stateful_widget(
        List::new(filter_items_with_sep)
            .block(titled_block("─[2]-Items", &indicator, itf, t))
            .highlight_style(Style::default().bg(t.selected_bg).fg(t.foreground))
            .highlight_symbol("▶ "),
        area,
        &mut state,
    );
}

fn render_search(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let t = &app.theme;
    let sf = app.focus == Focus::Search;
    // Leading magnifying-glass affordance — a one-column left margin off
    // the border, and coloured to match the text of the current state
    // (not accent), so it reads as part of the field rather than a badge.
    let line = if sf {
        Line::from(vec![
            Span::styled(" 󰍉 ", Style::default().fg(t.foreground)),
            Span::styled(
                app.vault.search_query.as_str(),
                Style::default().fg(t.foreground),
            ),
            Span::styled("█", Style::default().fg(t.accent)),
        ])
    } else if !app.vault.search_query.is_empty() {
        Line::from(vec![
            Span::styled(" 󰍉 ", Style::default().fg(t.dim)),
            Span::styled(app.vault.search_query.as_str(), Style::default().fg(t.dim)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" 󰍉 ", Style::default().fg(t.placeholder)),
            Span::styled("type to filter…", Style::default().fg(t.placeholder)),
        ])
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

/// Compact type label for the vault list. Identical to
/// [`item_type_label`] except "Secure Note" is shortened to "Note" so
/// the type column stays narrow; the detail screen still shows the full
/// name.
/// Compact type tags for the list column. The long names ("Secure
/// Note", "Identity", "SSH Key") are abbreviated so the fixed type
/// column stays as narrow as `[Login]` instead of widening every row to
/// fit the longest label. The detail screen shows the full type via
/// [`item_type_label`].
fn list_type_label(item_type: u8) -> &'static str {
    match item_type {
        2 => "Note",  // Secure Note
        4 => "Ident", // Identity
        5 => "SSH",   // SSH Key
        other => item_type_label(other),
    }
}

fn render_list(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let t = &app.theme;
    let lf = app.focus == Focus::List;
    let filtered = app.vault.filtered_items();

    // Only reserve an indicator column when at least one *visible* item
    // actually carries that indicator. A personal-only account (no
    // organisations) never pays for the 👥 column, and a view with no
    // favourites or reprompt-protected items collapses those too — so
    // the [Type]/name columns shift left instead of leaving a dead
    // gutter. The reservation is per-view (not per-row) so alignment
    // stays stable within the list.
    let (mut any_fav, mut any_reprompt, mut any_org) = (false, false, false);
    // Type column sized to the widest *visible* "[label]" instead of a
    // fixed pad — an all-[Login] view gets a 7-wide column, not 11, so
    // names start that much earlier.
    let mut type_w = 0usize;
    for it in filtered.iter() {
        any_fav |= it.favorite;
        any_reprompt |= it.needs_reprompt();
        any_org |= it.organization_id.is_some();
        type_w = type_w.max(list_type_label(it.item_type).len() + 2); // + "[]"
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
            // First cell = indicators + type tag. Indicators (★ / 🔒 /
            // 👥) are each 2 cells wide and only present when the column
            // is reserved; the type tag follows. The table left-aligns
            // the cell inside its fixed `ind_w + type_w` width, so the
            // name column lines up across rows without manual padding.
            // "Secure Note" is shortened to "Note" in `list_type_label`
            // so the common rows stay narrow; the detail view shows the
            // full type.
            let mut spans: Vec<Span> = Vec::with_capacity(4);
            if any_fav {
                spans.push(if item.favorite {
                    Span::styled("★ ", Style::default().fg(t.item_favorite))
                } else {
                    Span::raw("  ")
                });
            }
            if any_reprompt {
                spans.push(if item.needs_reprompt() {
                    Span::styled("🔒", Style::default().fg(t.error))
                } else {
                    Span::raw("  ")
                });
            }
            if any_org {
                spans.push(if item.organization_id.is_some() {
                    Span::styled("👥", Style::default().fg(t.accent))
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
    let mut state = TableState::default().with_selected(sel);
    let indicator = if flen > 0 {
        format!("{} of {}", app.vault.selected_index + 1, flen)
    } else {
        "0 of 0".into()
    };
    frame.render_stateful_widget(
        Table::new(
            rows,
            [Constraint::Length(ind_w + type_w), Constraint::Min(0)],
        )
        .column_spacing(1)
        .block(titled_block("─[3]-Vault", &indicator, lf, t))
        .row_highlight_style(
            Style::default()
                .bg(t.selected_bg)
                .fg(t.foreground)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ "),
        area,
        &mut state,
    );
    // Scroll cue on the right border when the list overflows. Driven by
    // the selection (which reaches both ends) rather than the top offset.
    draw_scrollbar(frame, area, flen, sel.unwrap_or(0), t);
}

fn render_cmd_log(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, cmd_h: u16) {
    let t = &app.theme;
    let clf = app.focus == Focus::CmdLog;
    let color = focus_color(clf, t.accent, t.inactive);
    let visible = (cmd_h as usize).saturating_sub(2);
    let total = app.cmd_log.entries.len();
    // Entry-based scroll-back with a `↑N` tag — the shared command-log
    // convention. One line per entry:
    // `✓ <cmd>  →  <detail>` (was a two-line `$ cmd` / `icon detail`).
    let scroll = app.cmd_log.scroll.min(total.saturating_sub(visible));
    let scroll_tag = if scroll == 0 {
        String::new()
    } else {
        format!("  ↑{scroll}")
    };
    let title = format!("─[4]-Command Log{scroll_tag}");
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(title, Style::default().fg(color)))
        .border_style(Style::default().fg(color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if total == 0 {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  no commands yet",
                Style::default().fg(t.dim),
            ))),
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
}
