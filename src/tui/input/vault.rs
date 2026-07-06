//! Key handler for the vault list (and the `Help` overlay, by go-back).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::App;
use crate::tui::flows::{
    auth, copy, export, folders, generator, import, items, memberships, send, vault,
};
use crate::tui::input::is_alt;
use crate::tui::screens::{Focus, Screen};

/// Dispatches a single key event on the vault screen.
pub fn handle(app: &mut App, key: KeyEvent) {
    // Number keys 0-4 jump between panels (disabled while typing in the
    // search box).
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

    // Global vault shortcuts.
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
            // Folder CRUD — bare letters act on the focused Folders panel
            // (the gradient); the `Alt+` form still works as a transition
            // alias since it hits the same arm.
            KeyCode::Char('n') => folders::open_create(app),
            KeyCode::Char('r') => folders::open_rename(app),
            KeyCode::Char('d') => folders::open_confirm_delete(app),
            _ => {}
        },

        Focus::Items => match key.code {
            KeyCode::Char('j') | KeyCode::Down | KeyCode::PageDown => app.vault.filter_move_down(),
            KeyCode::Char('k') | KeyCode::Up | KeyCode::PageUp => app.vault.filter_move_up(),
            // Switching to Trash fetches the trash list on demand;
            // `apply_filter` runs (and applies the filter) in the guard.
            KeyCode::Enter if app.apply_filter() => vault::request_load_trash(app),
            KeyCode::Tab | KeyCode::Esc => app.cycle_focus(),
            _ => {}
        },

        Focus::Search => match key.code {
            KeyCode::Esc => app.clear_search(),
            KeyCode::Tab => app.cycle_focus(),
            KeyCode::Char('j') | KeyCode::Down => app.vault.move_down(),
            KeyCode::Char('k') | KeyCode::Up => app.vault.move_up(),
            KeyCode::PageDown => app.vault.move_down_page(),
            KeyCode::PageUp => app.vault.move_up_page(),
            KeyCode::Enter if !app.vault.filtered_items().is_empty() => {
                app.screen = Screen::Detail;
                app.show_password = false;
            }
            KeyCode::Backspace => {
                app.vault.search_query.pop();
                app.vault.perform_search();
            }
            _ if is_alt(&key) => handle_alt_shortcuts(app, key),
            // Plain char only feeds search when no modifiers are active.
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE => {
                app.vault.search_query.push(c);
                app.vault.perform_search();
            }
            _ => {}
        },

        Focus::List => match key.code {
            KeyCode::Char('j') | KeyCode::Down => app.vault.move_down(),
            KeyCode::Char('k') | KeyCode::Up => app.vault.move_up(),
            KeyCode::PageDown => app.vault.move_down_page(),
            KeyCode::PageUp => app.vault.move_up_page(),
            KeyCode::Enter | KeyCode::Char('l') => app.go_to_detail(),
            KeyCode::Tab => app.cycle_focus(),
            // Alt+letter still runs the row actions (transition alias);
            // the Alt globals were already handled above.
            _ if is_alt(&key) => handle_alt_shortcuts(app, key),
            // Bare letters act on the focused row (the gradient). The
            // List panel never typed, so this is purely additive.
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE => list_row_action(app, c),
            _ => {}
        },

        Focus::CmdLog => match key.code {
            KeyCode::Char('j') | KeyCode::Down => app.cmd_log_scroll_up(1),
            KeyCode::Char('k') | KeyCode::Up => app.cmd_log_scroll_down(1),
            KeyCode::PageDown => app.cmd_log_scroll_down(5),
            KeyCode::PageUp => app.cmd_log_scroll_up(5),
            KeyCode::Tab | KeyCode::Esc => app.cycle_focus(),
            _ => {}
        },
    }
}

/// Bare-letter row actions on the focused vault List (the gradient):
/// the frequent, safe operations on the highlighted item. `j`/`k`/`l`
/// (navigate / open) and `0`–`4` (focus) are handled by the caller
/// before this runs, so they never reach here. Destructive `d` always
/// goes through the confirm popup (which offers permanent-delete via
/// `D` when not already in trash).
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

/// Opens the highlighted item's detail screen straight in edit mode —
/// the list-level `e` shortcut. No-op when the list is empty.
fn list_edit(app: &mut App) {
    if app.vault.selected_item().is_some() {
        app.go_to_detail();
        items::enter_edit_mode(app);
    }
}

/// Alt+key vault actions shared between the Search and List panels.
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
