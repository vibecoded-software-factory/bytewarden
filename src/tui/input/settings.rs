use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::input::nav::nav_clamp;
use crate::tui::settings_overlay::{SettingsFocus, SettingsSection};
use crate::tui::theme;

fn set_section(app: &mut App, section: usize) {
    app.settings_ui.section = section;
    app.settings_ui.row = 0;
}

fn nav_section(app: &mut App, delta: i8) {
    let before = app.settings_ui.section;
    let mut next = before;
    nav_clamp(&mut next, SettingsSection::ALL.len(), delta);
    if next != before {
        set_section(app, next);
    }
}

pub(crate) fn nav_preset(app: &mut App, delta: i8) {
    let before = app.settings_ui.theme_idx;
    nav_clamp(
        &mut app.settings_ui.theme_idx,
        theme::Preset::ALL.len(),
        delta,
    );
    if app.settings_ui.theme_idx != before {
        app.settings_preview_theme();
    }
}

pub fn handle(app: &mut App, key: KeyEvent) {
    match app.settings_ui.focus {
        SettingsFocus::Sidebar => handle_sidebar(app, key),
        SettingsFocus::Panel => handle_panel(app, key),
    }
}

pub fn mouse(app: &mut App, col: u16, row: u16) {
    use crate::tui::view::settings::{SettingsHit, settings_hit_at};
    let Some(hit) = settings_hit_at(col, row) else {
        return;
    };
    match hit {
        SettingsHit::Section(i) => {
            set_section(app, i);
            app.settings_ui.focus = SettingsFocus::Sidebar;
        }
        SettingsHit::Row(i) => {
            let section = SettingsSection::ALL[app.settings_ui.section];
            let reselect =
                app.settings_ui.focus == SettingsFocus::Panel && app.settings_ui.row == i;
            app.settings_ui.row = i;
            app.settings_ui.focus = SettingsFocus::Panel;
            if reselect && let Some(&r) = section.rows().get(i) {
                app.settings_adjust(r, true);
            }
        }
        SettingsHit::Theme(i) => {
            let reselect =
                app.settings_ui.focus == SettingsFocus::Panel && app.settings_ui.theme_idx == i;
            app.settings_ui.focus = SettingsFocus::Panel;
            app.settings_ui.theme_idx = i;
            if reselect {
                app.settings_confirm_theme();
            } else {
                app.settings_preview_theme();
            }
        }
    }
}

fn handle_sidebar(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::F(10) => app.settings_cancel(),
        KeyCode::Char('j') | KeyCode::Down => nav_section(app, 1),
        KeyCode::Char('k') | KeyCode::Up => nav_section(app, -1),
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

fn handle_rows_panel(app: &mut App, key: KeyEvent, section: SettingsSection) {
    let rows = section.rows();
    match key.code {
        KeyCode::Esc | KeyCode::F(10) => app.settings_cancel(),
        KeyCode::Tab | KeyCode::BackTab => app.settings_ui.focus = SettingsFocus::Sidebar,
        KeyCode::Char('j') | KeyCode::Down => nav_clamp(&mut app.settings_ui.row, rows.len(), 1),
        KeyCode::Char('k') | KeyCode::Up => nav_clamp(&mut app.settings_ui.row, rows.len(), -1),
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
    match key.code {
        KeyCode::Esc | KeyCode::F(10) => app.settings_cancel(),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Tab | KeyCode::BackTab => {
            app.settings_ui.focus = SettingsFocus::Sidebar;
        }
        KeyCode::Char('j') | KeyCode::Down => nav_preset(app, 1),
        KeyCode::Char('k') | KeyCode::Up => nav_preset(app, -1),
        KeyCode::Enter => app.settings_confirm_theme(),
        _ => {}
    }
}
