//! Key handler for the "create item" screen (type-picker + form).

use crossterm::event::{KeyCode, KeyEvent};

use crate::domain::filter::CREATE_ITEM_TYPES;
use crate::tui::app::App;
use crate::tui::flows::{generator, items};
use crate::tui::input::is_alt;
use crate::tui::input::nav::{nav_clamp, nav_wrap, text_input};

/// Dispatches a single key event on the create screen.
pub fn handle(app: &mut App, key: KeyEvent) {
    if app.create_choosing_type {
        let n = CREATE_ITEM_TYPES.len();
        match key.code {
            KeyCode::Esc => app.go_back(),
            KeyCode::Enter => items::create_select_type(app),
            KeyCode::Tab => nav_wrap(&mut app.create_type_idx, n, 1),
            KeyCode::BackTab => nav_wrap(&mut app.create_type_idx, n, -1),
            KeyCode::Char('j') | KeyCode::Down => nav_clamp(&mut app.create_type_idx, n, 1),
            KeyCode::Char('k') | KeyCode::Up => nav_clamp(&mut app.create_type_idx, n, -1),
            _ => {}
        }
        return;
    }

    let n = app.create_fields.len();

    // Alt+G opens the generator pre-targeted at the focused row when
    // it is a hidden (i.e. password-like) field.
    if key.code == KeyCode::Char('g')
        && is_alt(&key)
        && app
            .create_fields
            .get(app.create_field_idx)
            .is_some_and(|f| f.hidden)
    {
        generator::open_for_create_field(app, app.create_field_idx);
        return;
    }

    match key.code {
        KeyCode::Esc => app.go_back(),
        KeyCode::Enter => items::queue_create_item(app),
        KeyCode::Tab => nav_wrap(&mut app.create_field_idx, n, 1),
        KeyCode::BackTab => nav_wrap(&mut app.create_field_idx, n, -1),
        KeyCode::Down => nav_clamp(&mut app.create_field_idx, n, 1),
        KeyCode::Up => nav_clamp(&mut app.create_field_idx, n, -1),
        KeyCode::F(2) => {
            if let Some(f) = app.create_fields.get_mut(app.create_field_idx)
                && f.hidden
            {
                f.revealed = !f.revealed;
            }
        }
        _ => text_input(app.create_field_mut(), key),
    }
}
