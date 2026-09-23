use crate::domain::LineEditor;
use crate::tui::app::App;
use crate::tui::screens::Screen;

#[derive(Clone, Copy)]
pub struct PaletteCommand {
    pub label: &'static str,

    pub keys: &'static str,

    pub run: fn(&mut App),
}

pub struct PaletteState {
    pub query: LineEditor,

    pub all: Vec<PaletteCommand>,

    pub filtered: Vec<usize>,

    pub selected: usize,

    pub origin: Screen,
}

fn edit_selected(app: &mut App) {
    if app.vault.selected_item().is_some() {
        app.go_to_detail();
        super::items::enter_edit_mode(app);
    }
}

pub fn palette_commands(app: &App) -> Vec<PaletteCommand> {
    if matches!(app.screen, Screen::Create | Screen::Generator)
        || (app.screen == Screen::Detail && app.edit.active)
    {
        return form_commands(app);
    }

    let cmd = |label, keys, run: fn(&mut App)| PaletteCommand { label, keys, run };
    let trash = app.vault.is_trash_view();

    let mut v: Vec<PaletteCommand> = Vec::new();
    if !trash {
        v.push(cmd("New item", "n", super::items::open_create));
        v.push(cmd("Sync vault", "Alt+S", super::vault::request_sync));
    }
    v.extend([
        cmd(
            "Password generator",
            "Alt+G",
            super::generator::open_standalone,
        ),
        cmd("Export vault", "Alt+E", super::export::open),
        cmd("Import vault", "Alt+M", super::import::open),
        cmd("Create text Send", "Alt+W", super::send::open),
        cmd("Memberships", "Alt+B", super::memberships::open),
        cmd("Show fingerprint", "Alt+I", super::auth::show_fingerprint),
        cmd("Settings", "F10", App::open_settings),
        cmd("Lock vault", "Alt+L", super::auth::lock_vault),
        cmd("Log out", "Alt+O", super::auth::open_confirm_logout),
    ]);

    if app.vault.selected_item().is_some() {
        if trash {
            v.push(cmd("Restore item", "r", super::items::queue_restore_item));
        } else {
            v.extend([
                cmd(
                    "Copy password",
                    "c",
                    super::copy::copy_password_to_clipboard,
                ),
                cmd(
                    "Copy username",
                    "u",
                    super::copy::copy_username_to_clipboard,
                ),
                cmd("Edit item", "e", edit_selected),
                cmd("Toggle favorite", "f", super::items::toggle_favorite),
                cmd(
                    "Check HIBP breaches",
                    "x",
                    super::items::queue_check_exposed,
                ),
            ]);
        }

        v.push(cmd("Delete item", "d", super::items::open_confirm_delete));
    }
    v
}

fn form_commands(app: &App) -> Vec<PaletteCommand> {
    let cmd = |label, keys, run: fn(&mut App)| PaletteCommand { label, keys, run };
    let mut v: Vec<PaletteCommand> = Vec::new();

    match app.screen {
        Screen::Detail if app.edit.active => {
            if app
                .edit
                .fields
                .get(app.edit.field_idx)
                .is_some_and(|f| f.hidden)
            {
                v.push(cmd("Generate into this field", "Alt+G", |app| {
                    super::generator::open_for_edit_field(app, app.edit.field_idx)
                }));
            }
            v.extend([
                cmd("Add custom field", "Alt+N", super::items::add_custom_field),
                cmd("Add URL row", "Alt+U", super::items::add_uri_row),
                cmd(
                    "Rename custom field",
                    "Alt+R",
                    super::items::open_rename_field,
                ),
                cmd("Cycle field type", "Alt+T", super::items::cycle_field_type),
                cmd(
                    "Assign collections",
                    "Alt+L",
                    super::assign_collections::open,
                ),
                cmd(
                    "Remove field / URL row",
                    "Alt+Del",
                    super::items::remove_current_field,
                ),
            ]);
        }

        Screen::Create => {
            if app
                .create
                .fields
                .get(app.create.field_idx)
                .is_some_and(|f| f.hidden)
            {
                v.push(cmd("Generate into this field", "Alt+G", |app| {
                    super::generator::open_for_create_field(app, app.create.field_idx)
                }));
            }
            v.push(cmd(
                "Assign collections",
                "Alt+L",
                super::assign_collections::open,
            ));
        }

        Screen::Generator => {
            v.push(cmd("Copy result", "Alt+C", super::generator::copy_result));
            if app.generator.return_target.is_some() {
                v.push(cmd(
                    "Use result in form",
                    "Alt+U",
                    super::generator::use_result,
                ));
            }
        }
        _ => {}
    }
    v
}

pub fn open(app: &mut App) {
    let all = palette_commands(app);
    let filtered = (0..all.len()).collect();
    app.palette = Some(PaletteState {
        query: LineEditor::new(),
        all,
        filtered,
        selected: 0,
        origin: app.screen.clone(),
    });
    app.screen = Screen::CommandPalette;
}

pub fn cancel(app: &mut App) {
    if let Some(state) = app.palette.take() {
        app.screen = state.origin;
    }
}

pub fn rebuild_filter(app: &mut App) {
    let Some(state) = app.palette.as_mut() else {
        return;
    };
    let q = state.query.text().to_lowercase();
    state.filtered = state
        .all
        .iter()
        .enumerate()
        .filter(|(_, c)| q.is_empty() || c.label.to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .collect();
    if state.selected >= state.filtered.len() {
        state.selected = state.filtered.len().saturating_sub(1);
    }
}

pub fn move_selection(app: &mut App, delta: i8) {
    if let Some(state) = app.palette.as_mut() {
        let len = state.filtered.len();
        crate::tui::input::nav::nav_clamp(&mut state.selected, len, delta);
    }
}

pub fn run_selected(app: &mut App) {
    let Some(state) = app.palette.take() else {
        return;
    };
    app.screen = state.origin;
    if let Some(&idx) = state.filtered.get(state.selected)
        && let Some(cmd) = state.all.get(idx)
    {
        (cmd.run)(app);
    }
}
