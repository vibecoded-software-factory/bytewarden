pub mod assign_collections;
pub mod attachment_download;
pub mod attachment_upload;
pub mod common;
pub mod confirm;
pub mod confirm_delete_attachment;
pub mod create;
pub mod detail;
pub mod export;
pub mod folder_delete_confirm;
pub mod folder_name;
pub mod generator;
pub mod import;
pub mod item_actions;
pub mod login;
pub mod logout_confirm;
pub mod memberships;
pub mod mouse;
pub mod nav;
pub mod palette;
pub mod rename_field;
pub mod reprompt;
pub mod send_create;
pub mod settings;
pub mod vault;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::tui::app::App;
use crate::tui::screens::Screen;

#[inline]
pub fn is_alt(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::ALT)
}

#[inline]
pub fn is_bare_action(key: &KeyEvent) -> bool {
    is_alt(key) || !key.modifiers.contains(KeyModifiers::CONTROL)
}

#[inline]
pub fn busy_blocks(is_busy: bool, key: &KeyEvent) -> bool {
    is_busy && key.code != KeyCode::Esc
}

const HELP_PAGE_ROWS: u16 = 8;
const HELP_PAGE_COLS: u16 = 16;

fn handle_help(app: &mut App, key: KeyEvent) {
    let (y, x) = app.help_scroll;
    match key.code {
        KeyCode::Esc | KeyCode::F(1) | KeyCode::Char('q') => app.go_back(),
        KeyCode::Char('j') | KeyCode::Down => app.help_scroll.0 = y.saturating_add(1),
        KeyCode::Char('k') | KeyCode::Up => app.help_scroll.0 = y.saturating_sub(1),
        KeyCode::Char('h') | KeyCode::Left => app.help_scroll.1 = x.saturating_sub(2),
        KeyCode::Char('l') | KeyCode::Right => app.help_scroll.1 = x.saturating_add(2),
        KeyCode::PageDown => app.help_scroll.0 = y.saturating_add(HELP_PAGE_ROWS),
        KeyCode::PageUp => app.help_scroll.0 = y.saturating_sub(HELP_PAGE_ROWS),
        KeyCode::Home => app.help_scroll = (0, 0),
        KeyCode::End => app.help_scroll.0 = u16::MAX,

        _ if key.modifiers.contains(KeyModifiers::SHIFT) => match key.code {
            KeyCode::Char('H') => app.help_scroll.1 = x.saturating_sub(HELP_PAGE_COLS),
            KeyCode::Char('L') => app.help_scroll.1 = x.saturating_add(HELP_PAGE_COLS),
            _ => {}
        },
        _ => {}
    }
}

fn f1_opens_help(screen: &Screen) -> bool {
    matches!(
        screen,
        Screen::Vault | Screen::Login | Screen::Detail | Screen::Create
    )
}

fn f10_opens_settings(screen: &Screen) -> bool {
    matches!(
        screen,
        Screen::Vault | Screen::Login | Screen::Detail | Screen::Create | Screen::Generator
    )
}

pub fn handle_events(app: &mut App, ev: Event) {
    match ev {
        Event::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return;
            }

            if matches!(app.action_state, crate::tui::action::ActionState::Error(_)) {
                app.set_action(crate::tui::action::ActionState::Idle);
            }

            if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
                app.should_quit = true;
                return;
            }

            if key.code == KeyCode::F(1) && f1_opens_help(&app.screen) {
                app.help_from = Some(app.screen.clone());
                app.help_scroll = (0, 0);
                app.screen = Screen::Help;
                return;
            }

            if key.code == KeyCode::F(10) {
                if app.screen == Screen::Settings {
                    app.settings_cancel();
                    return;
                } else if f10_opens_settings(&app.screen) {
                    app.open_settings();
                    return;
                }
            }

            if key.code == KeyCode::Char('p') && key.modifiers == KeyModifiers::CONTROL {
                if app.screen == Screen::CommandPalette {
                    crate::tui::flows::palette::cancel(app);
                    return;
                } else if matches!(
                    app.screen,
                    Screen::Vault | Screen::Detail | Screen::Create | Screen::Generator
                ) {
                    crate::tui::flows::palette::open(app);
                    return;
                }
            }

            if busy_blocks(app.is_busy(), &key) {
                return;
            }
            dispatch_screen_key(app, key);
        }
        Event::Mouse(mouse) => {
            if app.is_busy() && !mouse::is_escape_click(&mouse) {
                return;
            }
            mouse::handle(app, mouse)
        }
        _ => {}
    }
}

pub(crate) fn dispatch_screen_key(app: &mut App, key: KeyEvent) {
    match app.screen.clone() {
        Screen::Splash => {}
        Screen::Login => login::handle(app, key),
        Screen::Vault => vault::handle(app, key),
        Screen::Detail => detail::handle(app, key),
        Screen::Help => handle_help(app, key),
        Screen::Settings => settings::handle(app, key),
        Screen::Create => create::handle(app, key),
        Screen::ConfirmDelete => confirm::handle(app, key),
        Screen::ConfirmLogout => logout_confirm::handle(app, key),
        Screen::Generator => generator::handle(app, key),
        Screen::RenameField => rename_field::handle(app, key),
        Screen::FolderName => folder_name::handle(app, key),
        Screen::ConfirmDeleteFolder => folder_delete_confirm::handle(app, key),
        Screen::Export => export::handle(app, key),
        Screen::Import => import::handle(app, key),
        Screen::AttachmentUpload => attachment_upload::handle(app, key),
        Screen::AttachmentDownload => attachment_download::handle(app, key),
        Screen::ConfirmDeleteAttachment => confirm_delete_attachment::handle(app, key),
        Screen::SendCreate => send_create::handle(app, key),
        Screen::Memberships => memberships::handle(app, key),
        Screen::RepromptUnlock => reprompt::handle(app, key),
        Screen::AssignCollections => assign_collections::handle(app, key),
        Screen::CommandPalette => palette::handle(app, key),
        Screen::ItemActions => item_actions::handle(app, key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn busy_blocks_swallows_keys_except_esc_while_busy() {
        assert!(busy_blocks(true, &key(KeyCode::Char('a'))));
        assert!(busy_blocks(true, &key(KeyCode::Enter)));
        assert!(busy_blocks(true, &key(KeyCode::Down)));

        assert!(!busy_blocks(true, &key(KeyCode::Esc)));
    }

    #[test]
    fn bare_action_declines_ctrl_letters() {
        for c in ['n', 'r', 'd', 'c', 'e', 'x'] {
            let k = KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
            assert!(!is_bare_action(&k), "Ctrl+{c} must not run a row action");
        }
    }

    #[test]
    fn bare_action_accepts_plain_and_shifted_letters() {
        assert!(is_bare_action(&key(KeyCode::Char('n'))));
        assert!(is_bare_action(&KeyEvent::new(
            KeyCode::Char('D'),
            KeyModifiers::SHIFT
        )));
    }

    #[test]
    fn bare_action_keeps_the_alt_aliases_including_altgr() {
        assert!(is_bare_action(&KeyEvent::new(
            KeyCode::Char('n'),
            KeyModifiers::ALT
        )));
        assert!(is_bare_action(&KeyEvent::new(
            KeyCode::Char('n'),
            KeyModifiers::ALT | KeyModifiers::CONTROL
        )));
    }

    #[test]
    fn busy_blocks_passes_everything_when_idle() {
        assert!(!busy_blocks(false, &key(KeyCode::Char('a'))));
        assert!(!busy_blocks(false, &key(KeyCode::Enter)));
        assert!(!busy_blocks(false, &key(KeyCode::Esc)));
    }
}
