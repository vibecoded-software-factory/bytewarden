//! Settings overlay input.
//!
//! Two panes: a section sidebar (left) and the active section's panel
//! (right). `Tab` / arrows move between and within them. `Esc`/`F10`
//! cancel (restoring any live preview); `Enter` confirms. Today the only
//! section is Theme — a live-previewing preset picker.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::settings_overlay::{SettingsFocus, SettingsSection};
use crate::tui::theme;

/// Resets the per-section row cursor whenever the highlighted section
/// changes, so a value-list section always opens on its first row.
fn set_section(app: &mut App, section: usize) {
    app.settings_ui.section = section;
    app.settings_ui.row = 0;
}

pub fn handle(app: &mut App, key: KeyEvent) {
    match app.settings_ui.focus {
        SettingsFocus::Sidebar => handle_sidebar(app, key),
        SettingsFocus::Panel => handle_panel(app, key),
    }
}

fn handle_sidebar(app: &mut App, key: KeyEvent) {
    let len = SettingsSection::ALL.len();
    match key.code {
        KeyCode::Esc | KeyCode::F(10) => app.settings_cancel(),
        KeyCode::Char('j') | KeyCode::Down if app.settings_ui.section + 1 < len => {
            set_section(app, app.settings_ui.section + 1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            set_section(app, app.settings_ui.section.saturating_sub(1));
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
        section => handle_rows_panel(app, key, section),
    }
}

/// Value-list panel: `↑/↓` move between rows, `←/→` change the focused
/// value (toggling bools / stepping numbers, persisted live), `Tab` /
/// `BackTab` return to the sidebar, `Esc`/`F10` close the overlay.
fn handle_rows_panel(app: &mut App, key: KeyEvent, section: SettingsSection) {
    let rows = section.rows();
    match key.code {
        KeyCode::Esc | KeyCode::F(10) => app.settings_cancel(),
        KeyCode::Tab | KeyCode::BackTab => app.settings_ui.focus = SettingsFocus::Sidebar,
        KeyCode::Char('j') | KeyCode::Down if app.settings_ui.row + 1 < rows.len() => {
            app.settings_ui.row += 1;
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.settings_ui.row = app.settings_ui.row.saturating_sub(1);
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if let Some(&row) = rows.get(app.settings_ui.row) {
                app.settings_adjust(row, true);
            }
        }
        KeyCode::Char('h') | KeyCode::Left => {
            if let Some(&row) = rows.get(app.settings_ui.row) {
                app.settings_adjust(row, false);
            }
        }
        _ => {}
    }
}

fn handle_theme_panel(app: &mut App, key: KeyEvent) {
    let len = theme::Preset::ALL.len();
    match key.code {
        KeyCode::Esc | KeyCode::F(10) => app.settings_cancel(),
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
