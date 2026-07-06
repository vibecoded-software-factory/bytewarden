//! Settings overlay input.
//!
//! Two panes: a section sidebar (left) and the active section's panel
//! (right). `Tab` / arrows move between and within them. `Esc`/`F9`
//! cancel (restoring any live preview); `Enter` confirms. Today the only
//! section is Theme — a live-previewing preset picker.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::settings_overlay::{SettingsFocus, SettingsSection};
use crate::tui::theme;

pub fn handle(app: &mut App, key: KeyEvent) {
    match app.settings_ui.focus {
        SettingsFocus::Sidebar => handle_sidebar(app, key),
        SettingsFocus::Panel => handle_panel(app, key),
    }
}

fn handle_sidebar(app: &mut App, key: KeyEvent) {
    let len = SettingsSection::ALL.len();
    match key.code {
        KeyCode::Esc | KeyCode::F(9) => app.settings_cancel(),
        KeyCode::Char('j') | KeyCode::Down if app.settings_ui.section + 1 < len => {
            app.settings_ui.section += 1;
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.settings_ui.section = app.settings_ui.section.saturating_sub(1);
        }
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => {
            app.settings_ui.focus = SettingsFocus::Panel;
        }
        _ => {}
    }
}

fn handle_panel(app: &mut App, key: KeyEvent) {
    match SettingsSection::ALL[app.settings_ui.section] {
        SettingsSection::Theme => handle_theme_panel(app, key),
    }
}

fn handle_theme_panel(app: &mut App, key: KeyEvent) {
    let len = theme::Preset::ALL.len();
    match key.code {
        KeyCode::Esc | KeyCode::F(9) => app.settings_cancel(),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Tab | KeyCode::BackTab => {
            app.settings_ui.focus = SettingsFocus::Sidebar;
        }
        KeyCode::Char('j') | KeyCode::Down if app.settings_ui.theme_idx + 1 < len => {
            app.settings_ui.theme_idx += 1;
            app.settings_preview_theme();
        }
        KeyCode::Char('k') | KeyCode::Up if app.settings_ui.theme_idx > 0 => {
            app.settings_ui.theme_idx -= 1;
            app.settings_preview_theme();
        }
        KeyCode::Enter => app.settings_confirm_theme(),
        _ => {}
    }
}
