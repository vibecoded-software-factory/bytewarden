//! Key handler for the confirm-delete popup.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::items;
use crate::tui::screens::Screen;

/// Dispatches a single key event on the confirm-delete popup.
///
/// In the trash view the popup is for permanent deletion (the item is
/// already trashed). In the regular vault, Enter sends to trash and
/// Shift+D performs a permanent delete.
pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('n') => app.screen = Screen::Vault,
        KeyCode::Enter => items::queue_delete_item(app, app.vault.is_trash_view()),
        KeyCode::Char('D') if !app.vault.is_trash_view() => items::queue_delete_item(app, true),
        _ => {}
    }
}
