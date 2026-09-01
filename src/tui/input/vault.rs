use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::App;
use crate::tui::flows::{
    auth, copy, export, folders, generator, import, items, memberships, send, vault,
};
use crate::tui::input::common;
use crate::tui::input::{is_alt, is_bare_action};
use crate::tui::screens::{Focus, Screen};

pub fn handle(app: &mut App, key: KeyEvent) {
    if key.modifiers == KeyModifiers::NONE && app.focus != Focus::Search {
        match key.code {
            KeyCode::Char('0') => {
                app.focus_panel(0);
                return;
            }
            KeyCode::Char('1') => {
                app.focus_panel(1);
                return;
            }
            KeyCode::Char('2') => {
                app.focus_panel(2);
                return;
            }
            KeyCode::Char('3') => {
                app.focus_panel(3);
                return;
            }
            KeyCode::Char('4') => {
                app.focus_panel(4);
                return;
            }
            _ => {}
        }
    }

    if key.code == KeyCode::Char('s') && is_alt(&key) && !app.vault.is_trash_view() {
        vault::request_sync(app);
        return;
    }
    if key.code == KeyCode::Char('l') && is_alt(&key) {
        auth::lock_vault(app);
        return;
    }
    if key.code == KeyCode::Char('o') && is_alt(&key) {
        auth::open_confirm_logout(app);
        return;
    }
    if key.code == KeyCode::Char('g') && is_alt(&key) {
        generator::open_standalone(app);
        return;
    }
    if key.code == KeyCode::Char('e') && is_alt(&key) {
        export::open(app);
        return;
    }
    if key.code == KeyCode::Char('i') && is_alt(&key) {
        auth::show_fingerprint(app);
        return;
    }
    if key.code == KeyCode::Char('m') && is_alt(&key) {
        import::open(app);
        return;
    }
    if key.code == KeyCode::Char('w') && is_alt(&key) {
        send::open(app);
        return;
    }
    if key.code == KeyCode::Char('b') && is_alt(&key) {
        memberships::open(app);
        return;
    }
    if key.code == KeyCode::Char('/') && key.modifiers == KeyModifiers::NONE {
        app.focus = Focus::Search;
        return;
    }

    match app.focus.clone() {
        Focus::Status => match key.code {
            KeyCode::Tab | KeyCode::Esc => app.cycle_focus(),
            _ => {}
        },

        Focus::Folders => match key.code {
            KeyCode::Char('j') | KeyCode::Down | KeyCode::PageDown => folders::move_down(app),
            KeyCode::Char('k') | KeyCode::Up | KeyCode::PageUp => folders::move_up(app),
            KeyCode::Enter => folders::apply_filter(app),
            KeyCode::Tab | KeyCode::Esc => app.cycle_focus(),

            KeyCode::Char('n') if is_bare_action(&key) => folders::open_create(app),
            KeyCode::Char('r') if is_bare_action(&key) => folders::open_rename(app),
            KeyCode::Char('d') if is_bare_action(&key) => folders::open_confirm_delete(app),
            _ => {}
        },

        Focus::Items => match key.code {
            KeyCode::Char('j') | KeyCode::Down | KeyCode::PageDown => app.vault.filter_move_down(),
            KeyCode::Char('k') | KeyCode::Up | KeyCode::PageUp => app.vault.filter_move_up(),

            KeyCode::Enter if app.apply_filter() => vault::request_load_trash(app),
            KeyCode::Tab | KeyCode::Esc => app.cycle_focus(),
            _ => {}
        },

        Focus::Search => match key.code {
            KeyCode::Esc => app.clear_search(),
            KeyCode::Tab => app.cycle_focus(),
            KeyCode::Down => app.vault.move_down(),
            KeyCode::Up => app.vault.move_up(),
            KeyCode::PageDown => app.vault.move_down_page(),
            KeyCode::PageUp => app.vault.move_up_page(),
            KeyCode::Enter if !app.vault.filtered_items().is_empty() => {
                app.screen = Screen::Detail;
                app.show_password = false;
            }
            _ if is_alt(&key) => handle_alt_shortcuts(app, key),
            _ => {
                if common::search_key(&mut app.vault.search_query, key) {
                    app.vault.perform_search();
                }
            }
        },

        Focus::List => match key.code {
            KeyCode::Char('j') | KeyCode::Down => app.vault.move_down(),
            KeyCode::Char('k') | KeyCode::Up => app.vault.move_up(),
            KeyCode::PageDown => app.vault.move_down_page(),
            KeyCode::PageUp => app.vault.move_up_page(),
            KeyCode::Enter | KeyCode::Char('l') => app.go_to_detail(),
            KeyCode::Tab => app.cycle_focus(),

            _ if is_alt(&key) => handle_alt_shortcuts(app, key),

            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE => list_row_action(app, c),
            _ => {}
        },

        Focus::CmdLog => match key.code {
            KeyCode::Char('j') | KeyCode::Down => app.cmd_log.scroll_up(1),
            KeyCode::Char('k') | KeyCode::Up => app.cmd_log.scroll_down(1),
            KeyCode::PageDown => app.cmd_log.scroll_down(5),
            KeyCode::PageUp => app.cmd_log.scroll_up(5),
            KeyCode::Tab | KeyCode::Esc => app.cycle_focus(),
            _ => {}
        },
    }
}

fn list_row_action(app: &mut App, c: char) {
    let trash = app.vault.is_trash_view();
    match c {
        'n' if !trash => items::open_create(app),
        'e' if !trash => list_edit(app),
        'c' if !trash => copy::copy_password_to_clipboard(app),
        'u' if !trash => copy::copy_username_to_clipboard(app),
        'f' if !trash => items::toggle_favorite(app),
        'x' if !trash => items::queue_check_exposed(app),
        'd' => items::open_confirm_delete(app),
        'r' if trash => items::queue_restore_item(app),
        _ => {}
    }
}

fn list_edit(app: &mut App) {
    if app.vault.selected_item().is_some() {
        app.go_to_detail();
        items::enter_edit_mode(app);
    }
}

fn handle_alt_shortcuts(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') => auth::lock_vault(app),
        KeyCode::Char('d') => items::open_confirm_delete(app),
        KeyCode::Char('r') if app.vault.is_trash_view() => items::queue_restore_item(app),
        KeyCode::Char('u') if !app.vault.is_trash_view() => copy::copy_username_to_clipboard(app),
        KeyCode::Char('c') if !app.vault.is_trash_view() => copy::copy_password_to_clipboard(app),
        KeyCode::Char('f') if !app.vault.is_trash_view() => items::toggle_favorite(app),
        KeyCode::Char('n') if !app.vault.is_trash_view() => items::open_create(app),
        _ => {}
    }
}
