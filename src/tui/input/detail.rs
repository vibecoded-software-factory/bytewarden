//! Key handler for the item detail view (read-only and edit modes).

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::{assign_collections, copy, generator, items, reprompt};
use crate::tui::input::is_alt;
use crate::tui::input::nav::{nav_clamp, nav_wrap, text_input};
use crate::tui::reprompt::ProtectedAction;

/// Dispatches a single key event on the detail screen.
pub fn handle(app: &mut App, key: KeyEvent) {
    if app.edit_mode {
        let n = app.edit_fields.len();

        // Alt+G: open the generator targeting the focused row when it
        // is a hidden (i.e. password-like) field.
        if key.code == KeyCode::Char('g')
            && is_alt(&key)
            && app
                .edit_fields
                .get(app.edit_field_idx)
                .is_some_and(|f| f.hidden)
        {
            generator::open_for_edit_field(app, app.edit_field_idx);
            return;
        }

        // Edit-mode structural shortcuts:
        //   Alt+N        → append new custom field
        //   Alt+U        → append new URL row (login items)
        //   Alt+Delete   → remove focused custom field OR URL row
        //   Alt+T        → cycle focused custom field's type
        //   Alt+R        → rename focused custom field (popup)
        //   Alt+L        → open collections multi-select (only on
        //                  the read-only "Collections" row of an
        //                  org item)
        if is_alt(&key) {
            match key.code {
                KeyCode::Char('n') => return items::add_custom_field(app),
                KeyCode::Char('u') => return items::add_uri_row(app),
                KeyCode::Delete => return items::remove_current_field(app),
                KeyCode::Char('t') => return items::cycle_field_type(app),
                KeyCode::Char('r') => return items::open_rename_field(app),
                KeyCode::Char('l') => return assign_collections::open(app),
                _ => {}
            }
        }

        match key.code {
            KeyCode::Esc => app.edit_mode = false,
            KeyCode::Enter => items::queue_save_edit(app),
            KeyCode::Tab => nav_wrap(&mut app.edit_field_idx, n, 1),
            KeyCode::BackTab => nav_wrap(&mut app.edit_field_idx, n, -1),
            KeyCode::Down => nav_clamp(&mut app.edit_field_idx, n, 1),
            KeyCode::Up => nav_clamp(&mut app.edit_field_idx, n, -1),
            KeyCode::F(2) => {
                // F2 in edit mode toggles `revealed` on the focused
                // hidden field. Going `false → true` is the case
                // that exposes a secret on screen and therefore
                // gates behind the reprompt popup. The reverse
                // direction (re-hiding) is always free.
                let needs_gate = app
                    .edit_fields
                    .get(app.edit_field_idx)
                    .is_some_and(|f| f.hidden && !f.revealed)
                    && app.selected_item().is_some_and(|i| i.needs_reprompt());
                if needs_gate && reprompt::maybe_open(app, ProtectedAction::RevealEditField) {
                    return;
                }
                app.edit_toggle_reveal();
            }
            _ => text_input(app.edit_field_mut(), key),
        }
        return;
    }

    let n = app.detail_field_count();
    match key.code {
        KeyCode::Esc | KeyCode::Char('h') => {
            app.show_password = false;
            app.detail_field = 0;
            app.go_back();
        }
        KeyCode::Tab => {
            app.show_password = false;
            nav_wrap(&mut app.detail_field, n, 1);
        }
        KeyCode::BackTab => {
            app.show_password = false;
            nav_wrap(&mut app.detail_field, n, -1);
        }
        KeyCode::Char('j') | KeyCode::Down | KeyCode::PageDown => {
            app.show_password = false;
            nav_clamp(&mut app.detail_field, n, 1);
        }
        KeyCode::Char('k') | KeyCode::Up | KeyCode::PageUp => {
            app.show_password = false;
            nav_clamp(&mut app.detail_field, n, -1);
        }
        KeyCode::F(2) => {
            // F2 in read-mode toggles the global `show_password`
            // flag. Like in edit-mode, gate the false→true transition
            // (the one that exposes secrets) for reprompt-protected
            // items.
            if !app.show_password
                && app.selected_item().is_some_and(|i| i.needs_reprompt())
                && reprompt::maybe_open(app, ProtectedAction::RevealDetail)
            {
                return;
            }
            app.show_password = !app.show_password;
        }
        // Read mode is a viewer, so the row actions are **bare letters**
        // (the gradient); the `Alt+` form still works as a transition
        // alias because these arms don't require the modifier. `h`/`j`/`k`
        // (back / navigate) are matched above and never reach here.
        KeyCode::Char('c') => copy::copy_selected_field(app),
        KeyCode::Char('e') if !app.is_trash_view() => items::enter_edit_mode(app),
        KeyCode::Char('m') if !app.is_trash_view() => assign_collections::open_for_move(app),
        KeyCode::Char('r') if app.is_trash_view() => items::queue_restore_item(app),
        KeyCode::Char('d') => items::open_confirm_delete(app),
        KeyCode::Char('x') if !app.is_trash_view() => items::queue_check_exposed(app),
        KeyCode::Char('a') if !app.is_trash_view() => items::open_attachment_upload(app),
        // Attachment rows — `s` downloads, `Alt+Del` deletes (kept on a
        // modifier so a stray Del can't wipe an attachment). The flows
        // toast when the focused row isn't an attachment, so they're safe
        // to press anywhere.
        KeyCode::Char('s') => items::open_attachment_download(app),
        KeyCode::Delete if is_alt(&key) && !app.is_trash_view() => {
            items::open_confirm_delete_attachment(app)
        }
        _ => {}
    }
}
