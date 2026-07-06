//! Crossterm event dispatching.
//!
//! [`handle_events`] is the single entry point called from the run loop.
//! It first handles global keys (Ctrl+C → quit) and then routes to a
//! per-screen handler.

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

/// Returns `true` when the key was pressed with the Alt modifier.
///
/// On Linux, AltGr arrives as `ALT | CONTROL`; we accept any modifier
/// set that *contains* `ALT` so AltGr-only keyboards still work.
#[inline]
pub fn is_alt(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::ALT)
}

/// Whether a key should be swallowed because a worker request is in
/// flight. While busy we accept only `Esc` (cancel / back) — every other
/// key could queue a second request or race the pending one. `Ctrl+C`
/// quits and is handled before this gate, so it always works.
///
/// Pure helper (no `App`) so it's unit-testable.
#[inline]
pub fn busy_blocks(is_busy: bool, key: &KeyEvent) -> bool {
    is_busy && key.code != KeyCode::Esc
}

/// Per-axis step constants for help-popup scrolling.
const HELP_PAGE_ROWS: u16 = 8;
const HELP_PAGE_COLS: u16 = 16;

/// Key handler for the help popup itself. Only Esc and F1 close it —
/// any other navigation key scrolls the popup so it can show content
/// taller or wider than its viewport.
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
        KeyCode::End => app.help_scroll.0 = u16::MAX, // renderer clamps
        // Shift+Left / Shift+Right page horizontally.
        _ if key.modifiers.contains(KeyModifiers::SHIFT) => match key.code {
            KeyCode::Char('H') => app.help_scroll.1 = x.saturating_sub(HELP_PAGE_COLS),
            KeyCode::Char('L') => app.help_scroll.1 = x.saturating_add(HELP_PAGE_COLS),
            _ => {}
        },
        _ => {}
    }
}

/// Returns the screens where pressing F1 should open the help popup.
///
/// Popups (Generator, Export, RenameField, …) are deliberately excluded:
/// each carries its own self-contained instructions and overlaying yet
/// another popup on top would lose the user's in-progress state. The
/// user must Esc out of any popup first.
fn f1_opens_help(screen: &Screen) -> bool {
    matches!(
        screen,
        Screen::Vault | Screen::Login | Screen::Detail | Screen::Create
    )
}

/// Returns the screens where pressing F10 should open the Settings
/// overlay. Like [`f1_opens_help`] plus the standalone Generator;
/// excluded on the modal popups, which own their input.
fn f10_opens_settings(screen: &Screen) -> bool {
    matches!(
        screen,
        Screen::Vault | Screen::Login | Screen::Detail | Screen::Create | Screen::Generator
    )
}

/// Dispatches a pre-read crossterm event to the right per-screen handler.
pub fn handle_events(app: &mut App, ev: Event) {
    match ev {
        Event::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return;
            }
            // Sticky errors clear on the next keypress (mutt/lazygit): a
            // failure is a condition the user must read, not a 1.5 s
            // event. The clearing key still does its thing below. (The
            // `⚠ WORKER DEAD` condition badge is separate and persists.)
            if matches!(app.action_state, crate::tui::action::ActionState::Error(_)) {
                app.set_action(crate::tui::action::ActionState::Idle);
            }
            // Global quit shortcut.
            if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
                app.should_quit = true;
                return;
            }
            // Global help shortcut — F1 opens the help popup from any
            // main screen. From the help screen itself it closes (any
            // key does), and from popups it falls through (the popup
            // handler decides). The originating screen is stashed so
            // the help renderer can show the correct background and
            // scope its content.
            if key.code == KeyCode::F(1) && f1_opens_help(&app.screen) {
                app.help_from = Some(app.screen.clone());
                app.help_scroll = (0, 0);
                app.screen = Screen::Help;
                return;
            }
            // Global Settings shortcut — F10 toggles the Settings overlay.
            // It opens from the main screens (never stacked on a popup)
            // and closes (cancel) when already open. Pure UI overlay, so
            // it sits before the busy gate like F1.
            if key.code == KeyCode::F(10) {
                if app.screen == Screen::Settings {
                    app.settings_cancel();
                    return;
                } else if f10_opens_settings(&app.screen) {
                    app.open_settings();
                    return;
                }
            }
            // Global command palette (Ctrl+P) — toggles the fuzzy,
            // context-aware action list. Opens from the vault / detail
            // (where the actions apply) and closes when already open. A
            // UI overlay, so it sits before the busy gate like F1 / F10.
            if key.code == KeyCode::Char('p') && key.modifiers == KeyModifiers::CONTROL {
                if app.screen == Screen::CommandPalette {
                    crate::tui::flows::palette::cancel(app);
                    return;
                } else if matches!(app.screen, Screen::Vault | Screen::Detail) {
                    crate::tui::flows::palette::open(app);
                    return;
                }
            }
            // While a worker request is in flight, swallow every key but
            // Esc so a second request can't be queued mid-flight.
            if busy_blocks(app.is_busy(), &key) {
                return;
            }
            dispatch_screen_key(app, key);
        }
        Event::Mouse(mouse) => mouse::handle(app, mouse),
        _ => {}
    }
}

/// Routes a key to the active screen's handler. Extracted from
/// [`handle_events`] so the mouse layer can synthesize an `Esc` to dismiss the
/// active overlay (click-outside-to-close) through the same per-screen logic.
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
        // Esc always passes so the user can cancel / navigate away.
        assert!(!busy_blocks(true, &key(KeyCode::Esc)));
    }

    #[test]
    fn busy_blocks_passes_everything_when_idle() {
        assert!(!busy_blocks(false, &key(KeyCode::Char('a'))));
        assert!(!busy_blocks(false, &key(KeyCode::Enter)));
        assert!(!busy_blocks(false, &key(KeyCode::Esc)));
    }
}
