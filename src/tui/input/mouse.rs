//! Mouse event handler — translates clicks/scrolls into focus changes
//! and selection moves.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::domain::filter::{ITEM_FILTERS, ItemFilter};
use crate::tui::app::App;
use crate::tui::screens::{Focus, LoginField, Screen};
use crate::tui::view::widgets::{ClickAction, ScrollTarget};

/// Dispatches a mouse event.
pub fn handle(app: &mut App, mouse: MouseEvent) {
    let (col, row) = (mouse.column, mouse.row);

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            app.last_click = Some((col, row));
            // A click outside any centered overlay dismisses it — the mouse twin
            // of Esc, routed through the active screen's own Esc handler (which
            // knows how to cancel it). A click inside falls through below.
            if let Some(rect) = crate::tui::view::widgets::active_modal_rect()
                && !crate::tui::mouse_areas::rect_contains(rect, col, row)
            {
                crate::tui::input::dispatch_screen_key(
                    app,
                    KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                );
                return;
            }
            // Clickable command-bar chrome (the F1/F10 anchor) — the mouse twin
            // of the function keys.
            if let Some(action) = crate::tui::view::widgets::button_at(col, row) {
                apply_click_action(app, action);
                return;
            }
            match app.screen {
                Screen::Login => mouse_login(app, col, row),
                Screen::Vault => mouse_vault(app, col, row),
                Screen::Detail => mouse_detail(app, col, row),
                Screen::Settings => crate::tui::input::settings::mouse(app, col, row),
                Screen::ItemActions => crate::tui::input::item_actions::mouse(app, col, row),
                Screen::AssignCollections => {
                    crate::tui::input::assign_collections::mouse(app, col, row)
                }
                Screen::CommandPalette => crate::tui::input::palette::mouse(app, col, row),
                _ => {}
            }
        }
        // Right-click a vault row opens its secondary-action menu — the mouse
        // twin of the per-item shortcuts (copy / edit / favorite / delete).
        MouseEventKind::Down(MouseButton::Right) if app.screen == Screen::Vault => {
            mouse_vault_right(app, col, row)
        }
        MouseEventKind::ScrollDown => mouse_scroll(
            app,
            col,
            row,
            1,
            mouse.modifiers.contains(KeyModifiers::SHIFT),
        ),
        MouseEventKind::ScrollUp => mouse_scroll(
            app,
            col,
            row,
            -1,
            mouse.modifiers.contains(KeyModifiers::SHIFT),
        ),
        _ => {}
    }
}

/// Dispatches a click on a registered chrome button — the mouse twin of its
/// key (mirrors the global `F1` / `F10` handling in the key router).
fn apply_click_action(app: &mut App, action: ClickAction) {
    match action {
        ClickAction::OpenHelp => {
            app.help_from = Some(app.screen.clone());
            app.help_scroll = (0, 0);
            app.screen = Screen::Help;
        }
        ClickAction::OpenSettings => app.open_settings(),
    }
}

fn mouse_login(app: &mut App, col: u16, row: u16) {
    // The renderer records each field's exact rect as it draws; focus (and,
    // for the checkboxes, toggle) whatever the pointer is over. No row math
    // that can drift from the layout.
    let Some(field) = crate::tui::view::login::login_field_at(col, row) else {
        return;
    };
    app.login.active_field = field.clone();
    match field {
        LoginField::SaveEmail => app.toggle_save_email(),
        LoginField::AutoLock => {
            app.auto_lock.enabled = !app.auto_lock.enabled;
            app.settings.write_auto_lock(app.auto_lock.enabled);
        }
        LoginField::KeepSession => app.toggle_keep_session(),
        _ => {}
    }
}

fn mouse_vault(app: &mut App, col: u16, row: u16) {
    let Some(focus) = app.mouse_areas.focus_for(col, row) else {
        return;
    };
    app.focus = focus.clone();

    if focus == Focus::List
        && let Some(row_idx) = app.mouse_areas.list_row(row)
    {
        let visible_idx = app.vault.scroll_offset + row_idx;
        if visible_idx < app.vault.filtered_items().len() {
            if app.vault.selected_index == visible_idx {
                app.go_to_detail();
            } else {
                app.vault.selected_index = visible_idx;
            }
        }
    }

    if focus == Focus::Items
        && let Some(row_idx) = app.mouse_areas.items_row(row)
    {
        // A separator is injected before Trash (last filter), so any
        // row past the SSH-key entry is offset by +1.
        let trash_display_row = ITEM_FILTERS.len();
        let filter_idx = if row_idx >= trash_display_row {
            ITEM_FILTERS.len() - 1
        } else if row_idx == ITEM_FILTERS.len() - 1 {
            return; // The separator row itself — ignore.
        } else {
            row_idx
        };
        if filter_idx < ITEM_FILTERS.len() {
            app.vault.filter_selected = filter_idx;
            app.vault.active_filter = ITEM_FILTERS[filter_idx].clone();
            app.vault.selected_index = 0;
            app.vault.scroll_offset = 0;
            app.vault.rebuild_filtered_cache();
            if app.vault.active_filter == ItemFilter::Trash {
                crate::tui::flows::vault::request_load_trash(app);
            }
        }
    }
}

/// Right-click on a vault list row: seat the selection on that row and open
/// its per-item action menu, so every action targets the clicked item through
/// the ordinary `selected_item` path.
fn mouse_vault_right(app: &mut App, col: u16, row: u16) {
    if app.mouse_areas.focus_for(col, row) != Some(Focus::List) {
        return;
    }
    let Some(row_idx) = app.mouse_areas.list_row(row) else {
        return;
    };
    let visible_idx = app.vault.scroll_offset + row_idx;
    if visible_idx < app.vault.filtered_items().len() {
        app.focus = Focus::List;
        app.vault.selected_index = visible_idx;
        crate::tui::flows::item_actions::open(app);
    }
}

fn mouse_detail(app: &mut App, col: u16, row: u16) {
    if row < 2 {
        app.show_password = false;
        app.detail_field = 0;
        app.go_back();
        return;
    }
    // The renderer records each visible field card's exact rect; focus the
    // card the pointer is over.
    let Some(field_idx) = crate::tui::view::detail::detail_field_at(col, row) else {
        return;
    };
    if app.edit.active {
        // Edit mode: click focuses the field. Revealing a hidden field stays
        // on F2 so it goes through the reprompt gate.
        app.edit.field_idx = field_idx;
        return;
    }
    if field_idx == app.detail_field {
        // A repeat click on the focused card reveals/hides it — the mouse twin
        // of F2, and it honours the same reprompt gate on the exposing edge so
        // the mouse can't bypass the master-password re-check.
        if !app.show_password
            && app
                .vault
                .selected_item()
                .is_some_and(|i| i.needs_reprompt())
            && crate::tui::flows::reprompt::maybe_open(
                app,
                crate::tui::reprompt::ProtectedAction::RevealDetail,
            )
        {
            return;
        }
        app.show_password = !app.show_password;
    } else {
        app.show_password = false;
        app.detail_field = field_idx;
    }
}

/// One generic wheel path: scroll whatever registered region sits under the
/// pointer. The view layer records those regions each frame, so there is no
/// per-screen `match` here — a new scrollable list is one `register_scroll`
/// call at its draw site.
fn mouse_scroll(app: &mut App, col: u16, row: u16, dir: i8, shift: bool) {
    if let Some(target) = crate::tui::view::widgets::scroll_target_at(col, row) {
        apply_scroll(app, target, dir, shift);
    }
}

/// The single table mapping a [`ScrollTarget`] to the state its wheel moves —
/// the only place that knows how each surface scrolls.
fn apply_scroll(app: &mut App, target: ScrollTarget, dir: i8, shift: bool) {
    match target {
        ScrollTarget::Vault => {
            if dir > 0 {
                app.vault.move_down()
            } else {
                app.vault.move_up()
            }
        }
        ScrollTarget::Filters => {
            if dir > 0 {
                app.vault.filter_move_down()
            } else {
                app.vault.filter_move_up()
            }
        }
        ScrollTarget::CmdLog => {
            if dir > 0 {
                app.cmd_log.scroll_up(1)
            } else {
                app.cmd_log.scroll_down(1)
            }
        }
        ScrollTarget::Detail => {
            let total = app.detail_field_count();
            if dir > 0 {
                if app.detail_field + 1 < total {
                    app.show_password = false;
                    app.detail_field += 1;
                }
            } else if app.detail_field > 0 {
                app.show_password = false;
                app.detail_field -= 1;
            }
        }
        ScrollTarget::Palette => {
            if dir > 0 {
                crate::tui::flows::palette::move_selection(app, 1)
            } else {
                crate::tui::flows::palette::move_selection(app, -1)
            }
        }
        ScrollTarget::Help => {
            // Wheel scrolls vertically; Shift+Wheel pans horizontally. The
            // renderer clamps both axes once it knows the viewport size.
            if shift {
                if dir > 0 {
                    app.help_scroll.1 = app.help_scroll.1.saturating_add(2);
                } else {
                    app.help_scroll.1 = app.help_scroll.1.saturating_sub(2);
                }
            } else if dir > 0 {
                app.help_scroll.0 = app.help_scroll.0.saturating_add(1);
            } else {
                app.help_scroll.0 = app.help_scroll.0.saturating_sub(1);
            }
        }
    }
}
