//! Help popup renderer — overlaid on top of the screen the user opened
//! it from.
//!
//! The content is *scoped*: it only shows shortcuts that are valid on
//! the originating screen, and inside the vault it further narrows to
//! the focused panel. F1 from any popup is handled by the popup
//! itself (so this renderer never has to think about them).

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::app::App;
use crate::tui::screens::{Focus, Screen};
use crate::tui::theme::Theme;
use crate::tui::view::widgets::{center_rect_pct, help_line};

/// Renders the keyboard-shortcut help popup, scoped to whichever screen
/// the user opened it from.
///
/// Takes `&mut App` because the renderer is the source of truth for
/// the viewport size: it clamps `app.help_scroll` against the actual
/// content / viewport overflow so the input handler can increment the
/// offset freely without doing its own bookkeeping.
pub fn draw_popup(frame: &mut Frame, area: Rect, app: &mut App) {
    let t = &app.theme;
    let from = app.help_from.clone().unwrap_or(Screen::Vault);
    let lines = build_lines(&from, &app.focus, t);

    let popup = center_rect_pct(64, 80, area);
    frame.render_widget(Clear, popup);

    // Inner viewport excludes the double border (1 row/col on each side).
    let inner = Rect {
        x: popup.x + 1,
        y: popup.y + 1,
        width: popup.width.saturating_sub(2),
        height: popup.height.saturating_sub(2),
    };

    // Clamp the scroll offsets against the actual overflow.
    let content_h = lines.len() as u16;
    let content_w = lines.iter().map(line_visual_width).max().unwrap_or(0);
    let max_y = content_h.saturating_sub(inner.height);
    let max_x = content_w.saturating_sub(inner.width);
    app.help_scroll.0 = app.help_scroll.0.min(max_y);
    app.help_scroll.1 = app.help_scroll.1.min(max_x);
    let (scroll_y, scroll_x) = app.help_scroll;

    // Title shows live position when there is overflow on either axis.
    let title = if max_y > 0 || max_x > 0 {
        format!(
            " Help — {}  ({}/{})  F1/Esc close ",
            screen_label(&from, app),
            scroll_y + 1,
            max_y + 1,
        )
    } else {
        format!(" Help — {}  ·  F1/Esc close ", screen_label(&from, app))
    };

    // Accent-bold title.
    let outer = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.accent));
    frame.render_widget(outer, popup);

    frame.render_widget(Paragraph::new(lines).scroll((scroll_y, scroll_x)), inner);

    // Draw scroll indicators on the popup borders when content overflows.
    draw_scroll_indicators(frame, popup, scroll_y, scroll_x, max_y, max_x, t);
}

/// Visible width of a `Line` (sum of span char counts — ignores ANSI
/// styling, which has zero on-screen width).
fn line_visual_width(line: &Line<'_>) -> u16 {
    line.spans
        .iter()
        .map(|s| s.content.chars().count() as u16)
        .sum()
}

/// Renders ↑/↓/←/→ marks on the popup border whenever there is hidden
/// content on that side. Centred along each axis so they read as
/// "more to scroll" rather than as part of the title.
fn draw_scroll_indicators(
    frame: &mut Frame,
    popup: Rect,
    scroll_y: u16,
    scroll_x: u16,
    max_y: u16,
    max_x: u16,
    t: &Theme,
) {
    let style = Style::default().fg(t.accent).add_modifier(Modifier::BOLD);
    if popup.width >= 4 && popup.height >= 4 {
        let mid_x = popup.x + popup.width / 2;
        let mid_y = popup.y + popup.height / 2;
        if scroll_y > 0 {
            frame.render_widget(
                Paragraph::new(Span::styled("▲", style)),
                Rect {
                    x: mid_x,
                    y: popup.y,
                    width: 1,
                    height: 1,
                },
            );
        }
        if scroll_y < max_y {
            frame.render_widget(
                Paragraph::new(Span::styled("▼", style)),
                Rect {
                    x: mid_x,
                    y: popup.y + popup.height - 1,
                    width: 1,
                    height: 1,
                },
            );
        }
        if scroll_x > 0 {
            frame.render_widget(
                Paragraph::new(Span::styled("◀", style)),
                Rect {
                    x: popup.x,
                    y: mid_y,
                    width: 1,
                    height: 1,
                },
            );
        }
        if scroll_x < max_x {
            frame.render_widget(
                Paragraph::new(Span::styled("▶", style)),
                Rect {
                    x: popup.x + popup.width - 1,
                    y: mid_y,
                    width: 1,
                    height: 1,
                },
            );
        }
    }
}

/// Human-readable label for the popup title.
fn screen_label(screen: &Screen, app: &App) -> &'static str {
    match screen {
        Screen::Login => "Login",
        Screen::Vault => match app.focus {
            Focus::Status => "Vault — Status panel",
            Focus::Search => "Vault — Search",
            Focus::Folders => "Vault — Folders panel",
            Focus::Items => "Vault — Items filter",
            Focus::List => "Vault — Item list",
            Focus::CmdLog => "Vault — Command log",
        },
        Screen::Detail => {
            if app.edit.active {
                "Detail (edit)"
            } else {
                "Detail (read)"
            }
        }
        Screen::Create => {
            if app.create.choosing_type {
                "Create item — pick type"
            } else {
                "Create item — fields"
            }
        }
        _ => "Help",
    }
}

/// Top-level dispatcher — picks the right section list per screen.
fn build_lines(screen: &Screen, focus: &Focus, t: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(blank());
    match screen {
        Screen::Login => login_section(&mut lines, t),
        Screen::Vault => vault_sections(&mut lines, focus, t),
        Screen::Detail => detail_sections(&mut lines, t),
        Screen::Create => create_sections(&mut lines, t),
        _ => login_section(&mut lines, t), // Should never trigger.
    }
    lines.push(blank());
    lines.push(global_footer(t));
    lines
}

// ── Per-screen sections ──────────────────────────────────────────────────

fn login_section(out: &mut Vec<Line<'static>>, t: &Theme) {
    out.push(section("Fields", t));
    out.push(help_line(
        "Tab / Shift+Tab",
        "Cycle Server / Email / Password / OTP / checkboxes",
        t,
    ));
    out.push(help_line(
        "← →  Home  End",
        "Move cursor in the focused text field",
        t,
    ));
    out.push(help_line("Backspace / Del", "Delete character", t));
    out.push(help_line(
        "Space",
        "Toggle Save email / Auto-lock / Keep session",
        t,
    ));
    out.push(help_line("F2", "Reveal / hide master password", t));
    out.push(blank());
    out.push(section("Submit", t));
    out.push(help_line(
        "Enter",
        "Login / unlock — or apply Server URL when on Server",
        t,
    ));
    out.push(blank());
    out.push(section("Checkboxes", t));
    out.push(help_line(
        "Save email",
        "Pre-fill the e-mail field on next launch",
        t,
    ));
    out.push(help_line(
        "Auto-lock",
        "Lock the vault after the configured idle window",
        t,
    ));
    out.push(help_line(
        "Keep session",
        "Skip unlock on relaunch — only while this terminal lives",
        t,
    ));
    out.push(blank());
    out.push(section("Alternative login methods", t));
    out.push(help_line(
        "Alt+K",
        "API-key login (BW_CLIENTID / BW_CLIENTSECRET env)",
        t,
    ));
    out.push(help_line("Alt+S", "SSO login (opens browser)", t));
    out.push(blank());
    out.push(section("Two-factor (when prompted)", t));
    out.push(help_line(
        "← →",
        "Cycle method on the Two-step Code field (Authenticator / Email / YubiKey)",
        t,
    ));
    out.push(help_line(
        "Enter",
        "Submit the code with the active method",
        t,
    ));
}

fn vault_sections(out: &mut Vec<Line<'static>>, focus: &Focus, t: &Theme) {
    // Always-available shortcuts on the vault screen.
    out.push(section("Vault — global", t));
    out.push(help_line("/", "Focus search bar", t));
    out.push(help_line(
        "url:<text>",
        "Search bar prefix — narrows match to login URIs only",
        t,
    ));
    out.push(help_line(
        "0 .. 4",
        "Jump panel: Status / Folders / Items / List / Log",
        t,
    ));
    out.push(help_line("Tab", "Cycle to the next panel", t));
    out.push(help_line(
        "Ctrl+P",
        "Command palette — fuzzy-search & run any action",
        t,
    ));
    out.push(blank());
    out.push(section("Vault — actions (Alt)", t));
    out.push(help_line("Alt+S", "Sync vault with the server", t));
    out.push(help_line("Alt+L / Alt+Q", "Lock vault", t));
    out.push(help_line("Alt+O", "Log out (remove account from CLI)", t));
    out.push(help_line("Alt+I", "Show fingerprint phrase (toast)", t));
    out.push(help_line("Alt+G", "Open password generator (popup)", t));
    out.push(help_line("Alt+E", "Export vault (popup)", t));
    out.push(help_line("Alt+M", "Import vault (popup)", t));
    out.push(help_line("Alt+W", "Create text Send (popup)", t));
    out.push(help_line(
        "Alt+B",
        "Memberships: organisations + collections",
        t,
    ));
    out.push(blank());
    out.push(section("List indicators (per item row)", t));
    out.push(help_line("★", "Favorite (toggle with Alt+F)", t));
    out.push(help_line(
        "🔒",
        "Reprompt-protected — secret-exposing actions ask for master password",
        t,
    ));
    out.push(help_line(
        "👥",
        "Belongs to an organisation (shared via collections)",
        t,
    ));
    out.push(blank());

    // Panel-scoped extras.
    match focus {
        Focus::Status => {
            out.push(section("Status panel [0]", t));
            out.push(help_line(
                "Tab / Esc",
                "Move focus away (no other interaction)",
                t,
            ));
        }
        Focus::Folders => {
            out.push(section("Folders panel [1]", t));
            out.push(help_line("j / k  ↑ ↓  PgUp/PgDn", "Move selection", t));
            out.push(help_line("Enter", "Apply folder filter", t));
            out.push(help_line("n", "New folder (popup)", t));
            out.push(help_line("r", "Rename focused folder (popup)", t));
            out.push(help_line("d", "Delete focused folder (confirm)", t));
            out.push(help_line("Tab / Esc", "Cycle focus away", t));
        }
        Focus::Items => {
            out.push(section("Items filter [2]", t));
            out.push(help_line(
                "j / k  ↑ ↓  PgUp/PgDn",
                "Move filter selection",
                t,
            ));
            out.push(help_line("Enter", "Apply highlighted filter", t));
            out.push(help_line("Tab / Esc", "Cycle focus away", t));
        }
        Focus::Search => {
            out.push(section("Search bar [/]", t));
            out.push(help_line(
                "(any char)",
                "Append to query — list re-ranks live",
                t,
            ));
            out.push(help_line("Backspace", "Pop last character", t));
            out.push(help_line("Esc", "Clear query and return to list", t));
            out.push(help_line(
                "j / k  ↑ ↓  PgUp/PgDn",
                "Move selection in the list",
                t,
            ));
            out.push(help_line("Enter", "Open detail of the selected item", t));
            out.push(help_line("Tab", "Cycle focus to next panel", t));
            out.push(blank());
            // The Search box types, so row actions ride on Alt here.
            out.push(section("Item shortcuts (Alt — the box is typing)", t));
            push_item_alt_shortcuts(out, t);
        }
        Focus::List => {
            out.push(section("Item list [3]", t));
            out.push(help_line("j / k  ↑ ↓", "Move selection", t));
            out.push(help_line("PgUp / PgDn", "Page (10 rows)", t));
            out.push(help_line("Enter / l", "Open detail", t));
            out.push(help_line("Tab", "Cycle focus", t));
            out.push(blank());
            out.push(section("Item actions (bare letters)", t));
            push_item_row_shortcuts(out, t);
        }
        Focus::CmdLog => {
            out.push(section("Command log [4]", t));
            out.push(help_line("j / ↓", "Scroll up one line", t));
            out.push(help_line("k / ↑", "Scroll down one line", t));
            out.push(help_line("PgUp / PgDn", "Scroll 5 lines", t));
            out.push(help_line("Tab / Esc", "Cycle focus away", t));
        }
    }
}

/// The item row actions as **bare letters** — the List panel (which
/// doesn't type).
fn push_item_row_shortcuts(out: &mut Vec<Line<'static>>, t: &Theme) {
    out.push(help_line("n", "New item (not in trash)", t));
    out.push(help_line("e", "Edit the selected item", t));
    out.push(help_line("u", "Copy username", t));
    out.push(help_line("c", "Copy password", t));
    out.push(help_line("f", "Toggle favorite ★", t));
    out.push(help_line("x", "Check password against HIBP", t));
    out.push(help_line("d", "Delete (or permanent in trash)", t));
    out.push(help_line("r", "Restore (trash view only)", t));
}

/// The same item row actions as `Alt+` chords — for the Search box,
/// where bare letters are typed into the query.
fn push_item_alt_shortcuts(out: &mut Vec<Line<'static>>, t: &Theme) {
    out.push(help_line("Alt+N", "New item (not in trash)", t));
    out.push(help_line("Alt+U", "Copy username", t));
    out.push(help_line("Alt+C", "Copy password", t));
    out.push(help_line("Alt+F", "Toggle favorite ★", t));
    out.push(help_line("Alt+D", "Delete (or permanent in trash)", t));
    out.push(help_line("Alt+R", "Restore (trash view only)", t));
}

fn detail_sections(out: &mut Vec<Line<'static>>, t: &Theme) {
    out.push(section("Read mode", t));
    out.push(help_line(
        "j / k  ↑ ↓  Tab / Shift+Tab",
        "Move between fields",
        t,
    ));
    out.push(help_line("PgUp / PgDn", "Same as k / j", t));
    out.push(help_line("F2", "Reveal / hide hidden field", t));
    out.push(help_line("c", "Copy focused field to clipboard", t));
    out.push(help_line("e", "Enter edit mode (not in trash)", t));
    out.push(help_line(
        "m",
        "Move into your organisation (popup) — only when item is personal and you have exactly 1 org",
        t,
    ));
    out.push(help_line("d", "Delete item (confirm)", t));
    out.push(help_line("x", "Check password against HIBP breaches", t));
    out.push(help_line("a", "Upload attachment (popup)", t));
    out.push(help_line("s", "Download focused attachment (popup)", t));
    out.push(help_line(
        "Alt+Del",
        "Delete focused attachment (confirm)",
        t,
    ));
    out.push(help_line("r", "Restore (trash view only)", t));
    out.push(help_line("Esc / h", "Back to vault", t));
    out.push(blank());
    out.push(section("Edit mode", t));
    out.push(help_line(
        "Tab / Shift+Tab",
        "Next / previous field (wraps)",
        t,
    ));
    out.push(help_line("↑ ↓", "Next / previous field (clamps)", t));
    out.push(help_line("← →  Home  End", "Move cursor within field", t));
    out.push(help_line("Backspace / Del", "Delete character", t));
    out.push(help_line(
        "F2",
        "Reveal / hide hidden field while editing",
        t,
    ));
    out.push(help_line("Enter", "Save (calls bw edit item)", t));
    out.push(help_line("Esc", "Cancel — back to read mode", t));
    out.push(help_line(
        "Alt+G",
        "Generate password into focused hidden field",
        t,
    ));
    out.push(help_line("Alt+N", "Add custom field", t));
    out.push(help_line("Alt+R", "Rename focused custom field (popup)", t));
    out.push(help_line(
        "Alt+T",
        "Cycle custom field type (text/hidden/bool/linked)",
        t,
    ));
    out.push(help_line("Alt+U", "Add URL row (login items)", t));
    out.push(help_line(
        "Alt+L",
        "Assign collections (popup) — only on the read-only Collections row of an org item",
        t,
    ));
    out.push(help_line(
        "Alt+Del",
        "Remove focused custom field or URL row",
        t,
    ));
    out.push(blank());
    out.push(section("Reprompt-protected items (🔒 in list)", t));
    out.push(help_line(
        "Alt+C / F2",
        "Master-password popup before exposing the secret (no caching)",
        t,
    ));
}

fn create_sections(out: &mut Vec<Line<'static>>, t: &Theme) {
    out.push(section("Type picker", t));
    out.push(help_line("j / k  ↑ ↓", "Select item type", t));
    out.push(help_line("Tab / Shift+Tab", "Select item type (wraps)", t));
    out.push(help_line("Enter", "Confirm type and go to fields", t));
    out.push(help_line("Esc", "Cancel — back to vault", t));
    out.push(blank());
    out.push(section("Fill fields", t));
    out.push(help_line(
        "Tab / Shift+Tab",
        "Next / previous field (wraps)",
        t,
    ));
    out.push(help_line("↑ ↓", "Next / previous field (clamps)", t));
    out.push(help_line("← →  Home  End", "Move cursor within field", t));
    out.push(help_line("Backspace / Del", "Delete character", t));
    out.push(help_line("F2", "Reveal / hide hidden field", t));
    out.push(help_line(
        "Alt+G",
        "Generate password into focused hidden field",
        t,
    ));
    out.push(help_line(
        "← →",
        "Cycle Organization (Personal / Org A / …) — only on the Organization row",
        t,
    ));
    out.push(help_line(
        "Alt+L",
        "Assign collections (popup) — only on the Collections row of an org item",
        t,
    ));
    out.push(help_line("Enter", "Create (calls bw create item)", t));
    out.push(help_line("Esc", "Cancel", t));
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn section(title: &str, t: &Theme) -> Line<'static> {
    Line::from(Span::styled(
        format!("  {title}"),
        Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
    ))
}

fn blank() -> Line<'static> {
    Line::from("")
}

fn global_footer(t: &Theme) -> Line<'static> {
    Line::from(Span::styled(
        "  F10: settings (theme…)  ·  j/k ↑↓ PgUp/PgDn: scroll  ·  h/l ←→: pan  ·  Home/End: top/bottom  ·  F1/Esc: close",
        Style::default().fg(t.dim),
    ))
}
