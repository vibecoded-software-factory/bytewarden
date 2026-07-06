//! Mouse event handler — translates clicks/scrolls into focus changes
//! and selection moves.

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::domain::filter::{ITEM_FILTERS, ItemFilter};
use crate::tui::app::App;
use crate::tui::screens::{Focus, LoginField, Screen};

/// Dispatches a mouse event.
pub fn handle(app: &mut App, mouse: MouseEvent) {
    let (col, row) = (mouse.column, mouse.row);

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            app.last_click = Some((col, row));
            match app.screen {
                Screen::Login => mouse_login(app, col, row),
                Screen::Vault => mouse_vault(app, col, row),
                Screen::Detail => mouse_detail(app, col, row),
                _ => {}
            }
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

fn mouse_login(app: &mut App, col: u16, row: u16) {
    let Some(form) = app.mouse_areas.login else {
        return;
    };
    if col < form.x || col >= form.x + form.width || row < form.y || row >= form.y + form.height {
        return;
    }

    // Login form layout (must match `tui/view/login.rs`):
    //
    //   no OTP (20 inner rows incl. border):
    //     0     padding
    //     1     server label       │  5    email label   │  9    pass label    │ 13 save
    //     2-4   server input       │  6-8  email input   │ 10-12 pass input    │ 14 auto-lock
    //                                                                          │ 15 keep-session
    //
    //   OTP shown (24 inner rows incl. border): 4 extra rows for OTP
    //     0     padding
    //     1     server label       │  5    email label   │  9    pass label    │ 17 save
    //     2-4   server input       │  6-8  email input   │ 10-12 pass input    │ 18 auto-lock
    //     13    otp label          │ 14-16 otp input                           │ 19 keep-session
    let inner_row = row.saturating_sub(form.y + 1);
    let row = inner_row;

    // Server / email / password / [otp] rows are positioned identically
    // in both layouts (OTP appears after password, before save).
    if row < 5 {
        app.login.active_field = LoginField::Server;
        return;
    }
    if row < 9 {
        app.login.active_field = LoginField::Email;
        return;
    }
    if row < 13 {
        app.login.active_field = LoginField::Password;
        return;
    }

    // After password, the offsets diverge depending on whether the
    // OTP block is shown. Branch explicitly so a click on the OTP
    // input doesn't accidentally toggle the save-email / auto-lock /
    // keep-session checkboxes.
    if app.login.awaiting_code() {
        if row < 17 {
            app.login.active_field = LoginField::Otp;
        } else if row < 18 {
            app.login.active_field = LoginField::SaveEmail;
            app.toggle_save_email();
        } else if row < 19 {
            app.login.active_field = LoginField::AutoLock;
            app.auto_lock.enabled = !app.auto_lock.enabled;
            app.settings.write_auto_lock(app.auto_lock.enabled);
        } else {
            app.login.active_field = LoginField::KeepSession;
            app.toggle_keep_session();
        }
    } else if row < 14 {
        app.login.active_field = LoginField::SaveEmail;
        app.toggle_save_email();
    } else if row < 15 {
        app.login.active_field = LoginField::AutoLock;
        app.auto_lock.enabled = !app.auto_lock.enabled;
        app.settings.write_auto_lock(app.auto_lock.enabled);
    } else {
        app.login.active_field = LoginField::KeepSession;
        app.toggle_keep_session();
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

fn mouse_detail(app: &mut App, col: u16, row: u16) {
    if row < 2 {
        app.show_password = false;
        app.detail_field = 0;
        app.go_back();
        return;
    }
    let Some(area) = app.mouse_areas.detail else {
        return;
    };
    if col < area.x || col >= area.x + area.width || row < area.y || row >= area.y + area.height {
        return;
    }

    let field_idx = (row.saturating_sub(area.y) / 4) as usize;
    let total = app.detail_field_count();
    if field_idx < total {
        if field_idx == app.detail_field {
            app.show_password = !app.show_password;
        } else {
            app.show_password = false;
            app.detail_field = field_idx;
        }
    }
}

fn mouse_scroll(app: &mut App, col: u16, row: u16, dir: i8, shift: bool) {
    if app.screen == Screen::Help {
        // Wheel scrolls vertically; Shift+Wheel scrolls horizontally.
        // Renderer clamps both axes once it knows the viewport size.
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
        return;
    }

    match app.screen {
        Screen::Vault => match app.mouse_areas.focus_for(col, row) {
            Some(Focus::Items) => {
                if dir > 0 {
                    app.vault.filter_move_down()
                } else {
                    app.vault.filter_move_up()
                }
            }
            Some(Focus::CmdLog) => {
                if dir > 0 {
                    app.cmd_log.scroll_up(1)
                } else {
                    app.cmd_log.scroll_down(1)
                }
            }
            _ => {
                if dir > 0 {
                    app.vault.move_down()
                } else {
                    app.vault.move_up()
                }
            }
        },
        Screen::Detail => {
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
        _ => {}
    }
}
