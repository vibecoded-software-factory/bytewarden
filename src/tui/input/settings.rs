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

/// A click inside the settings overlay: select a sidebar section, select a
/// panel row (a second click on the selected row cycles it), or preview/apply a
/// theme preset — the mouse twin of the keyboard navigation.
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
                // Clicking the already-selected row cycles / toggles it.
                app.settings_adjust(r, true);
            }
        }
        SettingsHit::Theme(i) => {
            let reselect =
                app.settings_ui.focus == SettingsFocus::Panel && app.settings_ui.theme_idx == i;
            app.settings_ui.focus = SettingsFocus::Panel;
            app.settings_ui.theme_idx = i;
            if reselect {
                // Clicking the already-previewed preset applies + saves it.
                app.settings_confirm_theme();
            } else {
                app.settings_preview_theme();
            }
        }
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
