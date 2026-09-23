use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::{assign_collections, copy, generator, items, reprompt};
use crate::tui::input::nav::{nav_clamp, nav_wrap, text_input};
use crate::tui::input::{is_alt, is_bare_action};
use crate::tui::reprompt::ProtectedAction;

pub fn handle(app: &mut App, key: KeyEvent) {
    if app.edit.active {
        let n = app.edit.fields.len();

        if key.code == KeyCode::Char('g')
            && is_alt(&key)
            && app
                .edit
                .fields
                .get(app.edit.field_idx)
                .is_some_and(|f| f.hidden)
        {
            generator::open_for_edit_field(app, app.edit.field_idx);
            return;
        }

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
            KeyCode::Esc => app.edit.active = false,
            KeyCode::Enter => items::queue_save_edit(app),
            KeyCode::Tab => nav_wrap(&mut app.edit.field_idx, n, 1),
            KeyCode::BackTab => nav_wrap(&mut app.edit.field_idx, n, -1),
            KeyCode::Down => nav_clamp(&mut app.edit.field_idx, n, 1),
            KeyCode::Up => nav_clamp(&mut app.edit.field_idx, n, -1),
            KeyCode::F(2) => {
                let needs_gate = app
                    .edit
                    .fields
                    .get(app.edit.field_idx)
                    .is_some_and(|f| f.hidden && !f.revealed)
                    && app
                        .vault
                        .selected_item()
                        .is_some_and(|i| i.needs_reprompt());
                if needs_gate && reprompt::maybe_open(app, ProtectedAction::RevealEditField) {
                    return;
                }
                app.edit.toggle_reveal();
            }
            _ => text_input(app.edit.field_mut(), key),
        }
        return;
    }

    let n = app.detail_field_count();

    if matches!(key.code, KeyCode::Char(_)) && !is_bare_action(&key) {
        return;
    }
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
            if !app.show_password
                && app
                    .vault
                    .selected_item()
                    .is_some_and(|i| i.needs_reprompt())
                && reprompt::maybe_open(app, ProtectedAction::RevealDetail)
            {
                return;
            }
            app.show_password = !app.show_password;
        }

        KeyCode::Char('c') => copy::copy_selected_field(app),
        KeyCode::Char('e') if !app.vault.is_trash_view() => items::enter_edit_mode(app),
        KeyCode::Char('m') if !app.vault.is_trash_view() => assign_collections::open_for_move(app),
        KeyCode::Char('r') if app.vault.is_trash_view() => items::queue_restore_item(app),
        KeyCode::Char('d') => items::open_confirm_delete(app),
        KeyCode::Char('x') if !app.vault.is_trash_view() => items::queue_check_exposed(app),
        KeyCode::Char('a') if !app.vault.is_trash_view() => items::open_attachment_upload(app),

        KeyCode::Char('s') => items::open_attachment_download(app),
        KeyCode::Delete if is_alt(&key) && !app.vault.is_trash_view() => {
            items::open_confirm_delete_attachment(app)
        }
        _ => {}
    }
}
