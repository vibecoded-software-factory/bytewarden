use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};

use crate::tui::theme::Theme;
use crate::tui::view::icons::IconSet;

thread_local! {

    static SCROLL_REGIONS: std::cell::RefCell<Vec<(Rect, ScrollTarget)>> =
        const { std::cell::RefCell::new(Vec::new()) };

    static MODAL_RECT: std::cell::RefCell<Option<Rect>> = const { std::cell::RefCell::new(None) };

    static BUTTONS: std::cell::RefCell<Vec<(Rect, ClickAction)>> =
        const { std::cell::RefCell::new(Vec::new()) };

    static FIELD_HITS: std::cell::RefCell<Vec<(Rect, usize)>> =
        const { std::cell::RefCell::new(Vec::new()) };

    static PICKER_HITS: std::cell::RefCell<(Rect, Vec<Option<usize>>)> =
        const { std::cell::RefCell::new((Rect::ZERO, Vec::new())) };
}

pub fn register_action_row(popup: Rect, line_idx: u16, code: crossterm::event::KeyCode) {
    let rect = Rect {
        x: popup.x + 1,
        y: popup.y + 1 + line_idx,
        width: popup.width.saturating_sub(2),
        height: 1,
    };
    register_button(
        rect,
        ClickAction::Key(crossterm::event::KeyEvent::new(
            code,
            crossterm::event::KeyModifiers::NONE,
        )),
    );
}

pub fn register_field_hit(rect: Rect, idx: usize) {
    if rect.width > 0 && rect.height > 0 {
        FIELD_HITS.with(|h| h.borrow_mut().push((rect, idx)));
    }
}

pub fn field_hit_at(column: u16, row: u16) -> Option<usize> {
    FIELD_HITS.with(|h| {
        h.borrow()
            .iter()
            .rev()
            .find(|(r, _)| {
                column >= r.x && column < r.x + r.width && row >= r.y && row < r.y + r.height
            })
            .map(|(_, i)| *i)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickAction {
    OpenHelp,

    OpenSettings,

    Key(crossterm::event::KeyEvent),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollTarget {
    Vault,

    Folders,

    Filters,

    CmdLog,

    Detail,

    Help,

    Palette,

    SettingsTheme,

    Memberships,
}

pub fn reset_scroll_regions() {
    SCROLL_REGIONS.with(|s| s.borrow_mut().clear());
    MODAL_RECT.with(|m| *m.borrow_mut() = None);
    BUTTONS.with(|b| b.borrow_mut().clear());
    FIELD_HITS.with(|h| h.borrow_mut().clear());
    PICKER_HITS.with(|h| *h.borrow_mut() = (Rect::ZERO, Vec::new()));
}

pub fn register_button(rect: Rect, action: ClickAction) {
    if rect.width > 0 && rect.height > 0 {
        BUTTONS.with(|b| b.borrow_mut().push((rect, action)));
    }
}

pub fn button_at(column: u16, row: u16) -> Option<ClickAction> {
    BUTTONS.with(|b| {
        b.borrow()
            .iter()
            .rev()
            .find(|(r, _)| {
                column >= r.x && column < r.x + r.width && row >= r.y && row < r.y + r.height
            })
            .map(|(_, a)| *a)
    })
}

pub fn register_modal(rect: Rect) {
    MODAL_RECT.with(|m| *m.borrow_mut() = Some(rect));
}

pub fn active_modal_rect() -> Option<Rect> {
    MODAL_RECT.with(|m| *m.borrow())
}

pub fn register_scroll(rect: Rect, target: ScrollTarget) {
    if rect.width > 0 && rect.height > 0 {
        SCROLL_REGIONS.with(|s| s.borrow_mut().push((rect, target)));
    }
}

pub fn scroll_target_at(column: u16, row: u16) -> Option<ScrollTarget> {
    SCROLL_REGIONS.with(|s| {
        s.borrow()
            .iter()
            .rev()
            .find(|(r, _)| {
                column >= r.x && column < r.x + r.width && row >= r.y && row < r.y + r.height
            })
            .map(|(_, t)| *t)
    })
}

pub struct ListTable<'a> {
    pub title: &'a str,

    pub counter: String,
    pub focused: bool,

    pub headers: Option<&'a [&'a str]>,

    pub widths: Vec<Constraint>,
    pub rows: Vec<ratatui::widgets::Row<'a>>,

    pub selected: Option<usize>,

    pub empty: Vec<Line<'static>>,
}

pub fn list_table(frame: &mut Frame, t: &Theme, area: Rect, lt: ListTable) -> usize {
    let len = lt.rows.len();
    let block = titled_block(lt.title, &lt.counter, lt.focused, t);
    if len == 0 {
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(lt.empty),
            area.inner(Margin {
                horizontal: 2,
                vertical: 1,
            }),
        );
        return 0;
    }
    let mut table = ratatui::widgets::Table::new(lt.rows, lt.widths)
        .column_spacing(1)
        .block(block)
        .row_highlight_style(
            Style::default()
                .bg(t.selected_bg)
                .fg(t.foreground)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    if let Some(headers) = lt.headers {
        table = table.header(
            ratatui::widgets::Row::new(headers.iter().map(|h| h.to_string()))
                .style(Style::default().fg(t.dim).add_modifier(Modifier::BOLD)),
        );
    }
    let sel = lt.selected.map(|s| s.min(len - 1));
    let mut state = ratatui::widgets::TableState::default().with_selected(sel);
    frame.render_stateful_widget(table, area, &mut state);

    draw_scrollbar(frame, area, len, sel.unwrap_or(0), t);
    state.offset()
}

pub fn cmdlog_height(total: u16) -> u16 {
    match total {
        0..=19 => 3,
        20..=23 => 4,
        24..=29 => 5,
        _ => 6,
    }
}

pub fn draw_scrollbar(
    frame: &mut Frame,
    area: Rect,
    content_len: usize,
    selected: usize,
    t: &Theme,
) {
    let viewport = area.height.saturating_sub(2) as usize;
    if viewport == 0 || content_len <= viewport {
        return;
    }
    let mut state = ScrollbarState::new(content_len)
        .viewport_content_length(viewport)
        .position(selected);
    let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .thumb_style(Style::default().fg(t.dim))
        .track_style(Style::default().fg(t.muted))
        .begin_symbol(None)
        .end_symbol(None);

    let track = area.inner(Margin {
        horizontal: 0,
        vertical: 1,
    });
    frame.render_stateful_widget(sb, track, &mut state);
}

pub fn focus_color(focused: bool, accent: Color, inactive: Color) -> Color {
    if focused { accent } else { inactive }
}

pub fn focus_border(focused: bool, accent: Color) -> Style {
    if focused {
        Style::default().fg(accent)
    } else {
        Style::default()
    }
}

pub fn focus_style(t: &Theme, focused: bool) -> Style {
    if focused {
        t.emphasis()
    } else {
        Style::default().fg(t.inactive)
    }
}

pub fn key_style(t: &Theme) -> Style {
    t.emphasis()
}

pub fn legend_line(items: &[(&str, &str)], width: u16, t: &Theme) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    let width = width as usize;
    for (i, (key, label)) in items.iter().enumerate() {
        let sep = if i == 0 { 0 } else { 3 };
        let seg = key.chars().count() + 1 + label.chars().count();

        let reserve = if i + 1 < items.len() { 2 } else { 0 };
        if i > 0 && used + sep + seg + reserve > width {
            spans.push(Span::styled(" …", Style::default().fg(t.muted)));
            break;
        }
        if i > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(t.muted)));
        }
        spans.push(Span::styled(
            crate::tui::keyboard::label(key).into_owned(),
            key_style(t),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            (*label).to_string(),
            Style::default().fg(t.dim),
        ));
        used += sep + seg;
    }
    Line::from(spans)
}

pub fn empty_state_lines(head: &str, hints: &[&str], t: &Theme) -> Vec<Line<'static>> {
    let key = crate::tui::keyboard::label;
    let mut out = vec![Line::from(Span::styled(
        format!("  {}", key(head)),
        Style::default()
            .fg(t.foreground)
            .add_modifier(Modifier::BOLD),
    ))];
    for h in hints {
        out.push(Line::from(Span::styled(
            format!("  {}", key(h)),
            Style::default().fg(t.dim),
        )));
    }
    out
}

pub fn favorite_star(t: &Theme) -> Span<'static> {
    Span::styled("★ ", Style::default().fg(t.item_favorite))
}

pub enum ConfirmTone {
    Primary,

    Danger,

    Cancel,
}

pub struct ConfirmAction {
    pub key: &'static str,
    pub code: crossterm::event::KeyCode,
    pub label: &'static str,
    pub tone: ConfirmTone,
}

pub struct ConfirmPopup<'a> {
    pub title: &'a str,
    pub width_pct: u16,
    pub body: Vec<Line<'static>>,
    pub actions: Vec<ConfirmAction>,
}

pub fn draw_confirm_popup(frame: &mut Frame, area: Rect, t: &Theme, p: ConfirmPopup) {
    let height = (p.body.len() + p.actions.len() + 5) as u16;
    let popup = center_rect(p.width_pct, height, area);
    register_modal(popup);
    frame.render_widget(Clear, popup);

    let mut lines = vec![Line::from("")];
    lines.extend(p.body);
    lines.push(Line::from(""));
    let base_idx = lines.len() as u16;
    for (i, a) in p.actions.iter().enumerate() {
        let (kstyle, lstyle) = match a.tone {
            ConfirmTone::Primary => (
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                Style::default().fg(t.foreground),
            ),
            ConfirmTone::Danger => (
                Style::default().fg(t.error).add_modifier(Modifier::BOLD),
                Style::default().fg(t.error),
            ),
            ConfirmTone::Cancel => (Style::default().fg(t.dim), Style::default().fg(t.dim)),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<5}", a.key), kstyle),
            Span::styled(format!("  {}", a.label), lstyle),
        ]));
        register_action_row(popup, base_idx + i as u16, a.code);
    }
    lines.push(Line::from(""));

    frame.render_widget(
        Paragraph::new(lines)
            .block(rounded_block(Style::default().fg(t.error)).title(p.title.to_string())),
        popup,
    );
}

pub enum InputFooter<'a> {
    Legend(&'a [(&'a str, &'a str)]),

    Note(&'a str),
}

pub struct InputPopup<'a> {
    pub title: &'a str,
    pub width_pct: u16,

    pub context: Vec<Line<'static>>,
    pub label: &'a str,
    pub label_hint: &'a str,
    pub editor: &'a crate::domain::LineEditor,
    pub placeholder: &'a str,
    pub footer: InputFooter<'a>,
}

pub fn draw_input_popup(frame: &mut Frame, area: Rect, t: &Theme, p: InputPopup) {
    let height = (p.context.len() + 9) as u16;
    let popup = center_rect(p.width_pct, height, area);
    register_modal(popup);
    frame.render_widget(Clear, popup);

    let outer = rounded_block(Style::default().fg(t.accent)).title(p.title.to_string());
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let mut constraints = vec![Constraint::Length(1)];
    constraints.extend(std::iter::repeat_n(Constraint::Length(1), p.context.len()));
    constraints.extend([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
    ]);
    let chunks = Layout::vertical(constraints).split(inner);

    for (i, line) in p.context.into_iter().enumerate() {
        frame.render_widget(Paragraph::new(line), chunks[1 + i]);
    }
    let label_row = chunks[chunks.len() - 3];
    let input_row = chunks[chunks.len() - 2];
    let footer_row = chunks[chunks.len() - 1];

    let mut label_spans = vec![Span::styled(
        format!(" {}", p.label),
        Style::default().fg(t.dim),
    )];
    if !p.label_hint.is_empty() {
        label_spans.push(Span::styled(
            format!("  ({})", p.label_hint),
            Style::default().fg(t.dim),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(label_spans)), label_row);

    frame.render_widget(
        Paragraph::new(editor_line_hinted(p.editor, p.placeholder, t))
            .block(rounded_block(Style::default().fg(t.accent))),
        input_row,
    );

    match p.footer {
        InputFooter::Legend(items) => {
            let mut line = legend_line(items, footer_row.width.saturating_sub(1), t);
            line.spans.insert(0, Span::raw(" "));
            frame.render_widget(Paragraph::new(line), footer_row);
        }
        InputFooter::Note(note) => {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" {note}"),
                    Style::default().fg(t.dim).add_modifier(Modifier::ITALIC),
                ))),
                footer_row,
            );
        }
    }
}

pub const MODAL_WIDTH_PCT: u16 = 60;

pub const MODAL_HEIGHT: u16 = 20;

pub enum PickerRow {
    Item(Vec<Line<'static>>),
    Header(Line<'static>),
}

pub struct PickerModal<'a> {
    pub title: String,

    pub query: Option<(&'a crate::domain::LineEditor, &'a str)>,
    pub rows: Vec<PickerRow>,

    pub selected: usize,

    pub empty: Vec<Line<'static>>,

    pub legend: &'a [(&'a str, &'a str)],

    pub scroll_target: Option<ScrollTarget>,

    pub size: Option<(u16, u16)>,
}

pub fn modal_inner_width(frame: &Frame) -> usize {
    center_rect(MODAL_WIDTH_PCT, MODAL_HEIGHT, frame.area())
        .width
        .saturating_sub(4) as usize
}

pub fn picker_row_at(column: u16, row: u16) -> Option<usize> {
    PICKER_HITS.with(|h| {
        let (rect, ref map) = *h.borrow();
        if rect.width == 0
            || column < rect.x
            || column >= rect.x + rect.width
            || row < rect.y
            || row >= rect.y + rect.height
        {
            return None;
        }
        map.get((row - rect.y) as usize).copied().flatten()
    })
}

pub fn draw_picker_modal(frame: &mut Frame, t: &Theme, icons: IconSet, m: PickerModal<'_>) {
    let (width_pct, height) = m.size.unwrap_or((MODAL_WIDTH_PCT, MODAL_HEIGHT));
    let area = center_rect(width_pct, height, frame.area());
    register_modal(area);
    frame.render_widget(Clear, area);

    let block =
        rounded_block(Style::default().fg(t.accent)).title(Span::styled(m.title, t.emphasis()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(target) = m.scroll_target {
        register_scroll(area, target);
    }
    let has_query = m.query.is_some();
    let chunks = Layout::vertical([
        Constraint::Length(if has_query { 1 } else { 0 }),
        Constraint::Length(if has_query { 1 } else { 0 }),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(inner);

    if let Some((editor, placeholder)) = m.query {
        let mut spans = vec![Span::styled(
            format!("{} ", icons.search()),
            Style::default().fg(t.dim),
        )];
        if editor.is_empty() {
            spans.push(Span::styled(
                placeholder.to_string(),
                Style::default().fg(t.placeholder),
            ));
        } else {
            spans.extend(editor_spans(editor, true, t));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), chunks[0]);
    }

    let vh = chunks[2].height.max(1) as usize;
    if m.rows.is_empty() {
        frame.render_widget(Paragraph::new(m.empty), chunks[2]);
        PICKER_HITS.with(|h| *h.borrow_mut() = (chunks[2], Vec::new()));
    } else {
        let mut display: Vec<Line<'static>> = Vec::new();

        let mut line_items: Vec<Option<usize>> = Vec::new();
        let (mut sel_start, mut sel_len) = (0usize, 1usize);
        let mut item_i = 0usize;
        let mut item_count = 0usize;
        for row in m.rows {
            match row {
                PickerRow::Header(l) => {
                    display.push(l);
                    line_items.push(None);
                }
                PickerRow::Item(ls) => {
                    let selected = item_i == m.selected;
                    if selected {
                        sel_start = display.len();
                        sel_len = ls.len().max(1);
                    }
                    for (li, l) in ls.into_iter().enumerate() {
                        let prefix = if li == 0 && selected { "▶ " } else { "  " };
                        let mut spans = vec![Span::styled(prefix.to_string(), t.emphasis())];
                        spans.extend(l.spans);
                        let mut l = Line::from(spans);
                        if selected {
                            for s in l.spans.iter_mut() {
                                s.style = s.style.bg(t.selected_bg).add_modifier(Modifier::BOLD);
                            }
                        }
                        display.push(l);
                        line_items.push(Some(item_i));
                    }
                    item_i += 1;
                    item_count += 1;
                }
            }
        }

        let sel_end = sel_start + sel_len;
        let scroll = sel_end.saturating_sub(vh);
        let total_lines = display.len();
        let visible: Vec<Line<'static>> = display.into_iter().skip(scroll).take(vh).collect();
        let visible_items: Vec<Option<usize>> =
            line_items.into_iter().skip(scroll).take(vh).collect();
        frame.render_widget(Paragraph::new(visible), chunks[2]);
        PICKER_HITS.with(|h| *h.borrow_mut() = (chunks[2], visible_items));
        if total_lines > vh {
            draw_scrollbar(frame, chunks[2], item_count, m.selected, t);
        }
    }

    frame.render_widget(
        Paragraph::new(legend_line(m.legend, chunks[3].width, t)),
        chunks[3],
    );
}

pub fn rounded_block(border_style: Style) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
}

pub fn titled_block(title: &str, bottom: &str, focused: bool, t: &Theme) -> Block<'static> {
    let mut title_style = Style::default().fg(if focused { t.accent } else { t.inactive });
    if focused {
        title_style = title_style.add_modifier(Modifier::BOLD);
    }
    titled_block_styled(title, title_style, bottom, focused, t)
}

pub fn titled_block_styled(
    title: &str,
    title_style: Style,
    bottom: &str,
    focused: bool,
    t: &Theme,
) -> Block<'static> {
    let col = if focused { t.accent } else { t.inactive };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(title.to_string(), title_style))
        .title_bottom(
            Line::from(Span::styled(bottom.to_string(), Style::default().fg(t.dim)))
                .right_aligned(),
        )
        .border_style(Style::default().fg(col))
}

pub fn render_cmd_bar(frame: &mut Frame, bar: Rect, hints: &[(&str, &str)], t: &Theme) {
    render_cmd_bar_inner(frame, bar, hints, t, None);
}

pub fn render_cmd_bar_with_help(frame: &mut Frame, bar: Rect, hints: &[(&str, &str)], t: &Theme) {
    render_cmd_bar_inner(frame, bar, hints, t, Some("F1 help · F10 settings"));
}

fn render_cmd_bar_inner(
    frame: &mut Frame,
    bar: Rect,
    hints: &[(&str, &str)],
    t: &Theme,
    anchor: Option<&str>,
) {
    let inner = bar;
    let suffix = anchor.unwrap_or("");
    let total = inner.width as usize;

    let suffix_block = if suffix.is_empty() {
        0
    } else {
        suffix.chars().count() + 2
    };
    let hints_avail = total.saturating_sub(suffix_block + 1);

    if hints_avail > 0 && !hints.is_empty() {
        let mut line = legend_line(hints, hints_avail as u16, t);
        line.spans.insert(0, Span::raw(" "));
        frame.render_widget(Paragraph::new(line), inner);
    }
    if !suffix.is_empty() {
        frame.render_widget(
            Paragraph::new(
                Line::from(Span::styled(
                    suffix,
                    Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                ))
                .right_aligned(),
            ),
            inner,
        );

        let anchor_len = suffix.chars().count() as u16;
        if inner.width >= anchor_len {
            let ax = inner.x + inner.width - anchor_len;
            let help_w = "F1 help".chars().count() as u16;
            let sep_w = " · ".chars().count() as u16;
            let set_w = "F10 settings".chars().count() as u16;
            register_button(
                Rect::new(ax, inner.y, help_w, 1),
                crate::tui::view::widgets::ClickAction::OpenHelp,
            );
            register_button(
                Rect::new(ax + help_w + sep_w, inner.y, set_w, 1),
                crate::tui::view::widgets::ClickAction::OpenSettings,
            );
        }
    }
}

fn cursor_spans(text: &str, cursor: usize, focused: bool, t: &Theme) -> Vec<Span<'static>> {
    let base = Style::default().fg(t.foreground);
    if !focused {
        return vec![Span::styled(text.to_string(), base)];
    }
    let cursor_style = Style::default().add_modifier(Modifier::REVERSED);
    let chars: Vec<char> = text.chars().collect();
    let pos = cursor.min(chars.len());
    let before: String = chars[..pos].iter().collect();
    if pos >= chars.len() {
        return vec![Span::styled(before, base), Span::styled(" ", cursor_style)];
    }
    let under: String = chars[pos].to_string();
    let after: String = chars[pos + 1..].iter().collect();
    vec![
        Span::styled(before, base),
        Span::styled(under, cursor_style),
        Span::styled(after, base),
    ]
}

pub fn editor_spans(
    editor: &crate::domain::LineEditor,
    focused: bool,
    t: &Theme,
) -> Vec<Span<'static>> {
    cursor_spans(editor.text(), editor.cursor(), focused, t)
}

pub fn editor_spans_masked(
    editor: &crate::domain::LineEditor,
    focused: bool,
    t: &Theme,
) -> Vec<Span<'static>> {
    let total = editor.len_chars();
    let base = Style::default().fg(t.foreground);
    if !focused {
        return vec![Span::styled("●".repeat(total), base)];
    }
    let cursor_style = Style::default().add_modifier(Modifier::REVERSED);
    let cur = editor.cursor().min(total);
    let before = "●".repeat(cur);
    if cur >= total {
        return vec![Span::styled(before, base), Span::styled(" ", cursor_style)];
    }
    vec![
        Span::styled(before, base),
        Span::styled("●".to_string(), cursor_style),
        Span::styled("●".repeat(total - cur - 1), base),
    ]
}

pub fn editor_line_hinted(
    editor: &crate::domain::LineEditor,
    placeholder: &str,
    t: &Theme,
) -> Line<'static> {
    let mut spans = editor_spans(editor, true, t);
    if editor.is_empty() {
        spans.push(Span::styled(
            format!(" {placeholder}"),
            Style::default().fg(t.placeholder),
        ));
    }
    Line::from(spans)
}

pub fn render_checkbox(
    frame: &mut Frame,
    label: &str,
    checked: bool,
    focused: bool,
    accent: Color,
    inactive: Color,
    area: Rect,
) {
    let icon = if checked { "☑" } else { "☐" };
    let icol = if checked { accent } else { inactive };
    let lcol = if focused { accent } else { inactive };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(icon, Style::default().fg(icol)),
            Span::styled(format!(" {label}"), Style::default().fg(lcol)),
        ])),
        area,
    );
}

pub fn field_areas(count: usize, area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::vertical(
        (0..count)
            .map(|_| Constraint::Length(4))
            .collect::<Vec<_>>(),
    )
    .split(area)
}

pub fn field_card_capacity(area: Rect) -> usize {
    (area.height / 4).max(1) as usize
}

pub fn field_areas_windowed(
    count: usize,
    selected: usize,
    area: Rect,
) -> (std::rc::Rc<[Rect]>, usize) {
    let cap = field_card_capacity(area);
    if count <= cap {
        return (field_areas(count, area), 0);
    }

    let start = selected
        .min(count - 1)
        .saturating_sub(cap - 1)
        .min(count - cap);
    (field_areas(cap, area), start)
}

pub fn render_field_card(
    frame: &mut Frame,
    label: &str,
    hint: &str,
    vline: Line,
    bcol: Color,
    area: Rect,
    t: &Theme,
) {
    let fc = Layout::vertical([Constraint::Length(1), Constraint::Length(3)]).split(area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {label}"), Style::default().fg(bcol)),
            Span::styled(
                crate::tui::keyboard::label(hint).into_owned(),
                Style::default().fg(t.dim),
            ),
        ])),
        fc[0],
    );
    frame.render_widget(
        Paragraph::new(vline).block(rounded_block(Style::default().fg(bcol))),
        fc[1],
    );
}

pub fn center_rect(width_pct: u16, height: u16, area: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - width_pct) / 2),
        Constraint::Percentage(width_pct),
        Constraint::Percentage((100 - width_pct) / 2),
    ])
    .split(v[1])[1]
}

pub fn center_rect_abs(width: u16, height: u16, area: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height.min(area.height)),
        Constraint::Fill(1),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width.min(area.width)),
        Constraint::Fill(1),
    ])
    .split(v[1])[1]
}

pub fn center_rect_pct(width_pct: u16, height_pct: u16, area: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Percentage((100 - height_pct) / 2),
        Constraint::Percentage(height_pct),
        Constraint::Percentage((100 - height_pct) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - width_pct) / 2),
        Constraint::Percentage(width_pct),
        Constraint::Percentage((100 - width_pct) / 2),
    ])
    .split(v[1])[1]
}

pub fn help_line<'a>(key: &'a str, desc: &'a str, t: &Theme) -> Line<'a> {
    let key = crate::tui::keyboard::label(key);
    Line::from(vec![
        Span::raw("  "),
        Span::styled(format!("{key:<14}"), Style::default().fg(t.accent)),
        Span::styled(desc, Style::default().fg(t.foreground)),
    ])
}

#[cfg(test)]
mod tests {
    use super::{
        ScrollTarget, cmdlog_height, register_scroll, reset_scroll_regions, scroll_target_at,
    };
    use ratatui::layout::Rect;

    #[test]
    fn scroll_registry_dispatches_by_position_top_most_wins() {
        reset_scroll_regions();

        register_scroll(Rect::new(0, 0, 10, 5), ScrollTarget::Vault);
        register_scroll(Rect::new(2, 1, 4, 2), ScrollTarget::Help);

        assert_eq!(scroll_target_at(0, 0), Some(ScrollTarget::Vault));

        assert_eq!(scroll_target_at(3, 1), Some(ScrollTarget::Help));

        assert_eq!(scroll_target_at(50, 50), None);

        register_scroll(Rect::new(0, 0, 0, 5), ScrollTarget::CmdLog);
        assert_eq!(scroll_target_at(0, 0), Some(ScrollTarget::Vault));

        reset_scroll_regions();
        assert_eq!(scroll_target_at(0, 0), None);
    }

    #[test]
    fn cmdlog_height_is_bounded_and_monotonic() {
        for h in 0u16..80 {
            let r = cmdlog_height(h);
            assert!((3..=6).contains(&r), "out of range at {h}: {r}");
        }

        for h in 1u16..80 {
            assert!(cmdlog_height(h) >= cmdlog_height(h - 1));
        }

        assert_eq!(cmdlog_height(18), 3);
        assert_eq!(cmdlog_height(40), 6);
    }
}
