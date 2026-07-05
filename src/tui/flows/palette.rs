//! Command palette (`Ctrl+P`) — a fuzzy-searchable, context-aware list
//! of the actions valid where you are, each of which dispatches the very
//! same `flows::*` its keybinding would. Doubles as an executable
//! cheat-sheet: every row shows its shortcut.
//!
//! Keep in sync (the fifth surface): footer hints · `view/help.rs` ·
//! `README.md` tables · `UX.md` · **this `palette_commands` list**.

use crate::domain::LineEditor;
use crate::tui::app::App;
use crate::tui::screens::Screen;

/// One entry in the command palette. `run` is a plain `fn(&mut App)`
/// pointing at an existing flow, so the palette can never diverge from
/// what the keybinding does.
#[derive(Clone, Copy)]
pub struct PaletteCommand {
    /// What the row says.
    pub label: &'static str,
    /// The keybinding shown right-aligned (the cheat-sheet half).
    pub keys: &'static str,
    /// The action, run on the screen the palette was opened from.
    pub run: fn(&mut App),
}

/// In-flight command-palette state. `None` outside the palette.
pub struct PaletteState {
    /// The fuzzy query.
    pub query: LineEditor,
    /// Every command available in this context, captured at open time.
    pub all: Vec<PaletteCommand>,
    /// Indices into `all` matching the query (substring over the label).
    pub filtered: Vec<usize>,
    /// Highlight, indexing `filtered`.
    pub selected: usize,
    /// Screen the palette was opened from — restored before the command
    /// runs so the action lands on the right context.
    pub origin: Screen,
}

/// Wrapper: open the selected item's detail straight in edit mode.
fn edit_selected(app: &mut App) {
    if app.selected_item().is_some() {
        app.go_to_detail();
        super::items::enter_edit_mode(app);
    }
}

/// Builds the context-aware command list for the palette. App-wide
/// commands always show; the item verbs only when an item is selected
/// (and not in the trash), matching the keybindings' own guards.
pub fn palette_commands(app: &App) -> Vec<PaletteCommand> {
    let cmd = |label, keys, run| PaletteCommand { label, keys, run };
    let mut v = vec![
        cmd("New item", "n", super::items::open_create as fn(&mut App)),
        cmd("Sync vault", "Alt+S", super::vault::request_sync),
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
        cmd("Settings", "F9", App::open_settings),
        cmd("Lock vault", "Alt+L", super::auth::lock_vault),
        cmd("Log out", "Alt+O", super::auth::open_confirm_logout),
    ];
    // Item verbs — only meaningful with a selected, non-trashed item.
    if app.selected_item().is_some() && !app.is_trash_view() {
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
            cmd("Delete item", "d", super::items::open_confirm_delete),
        ]);
    }
    v
}

/// Opens the palette over the current screen.
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

/// Closes the palette without running anything.
pub fn cancel(app: &mut App) {
    if let Some(state) = app.palette.take() {
        app.screen = state.origin;
    }
}

/// Recomputes the filtered list from the query (case-insensitive
/// substring over the label) and clamps the highlight.
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

/// Moves the highlight by `delta`, clamped to the filtered list.
pub fn move_selection(app: &mut App, delta: isize) {
    if let Some(state) = app.palette.as_mut() {
        let len = state.filtered.len();
        if len == 0 {
            return;
        }
        let cur = state.selected as isize;
        state.selected = (cur + delta).clamp(0, len as isize - 1) as usize;
    }
}

/// Restores the origin screen and runs the highlighted command — the
/// exact same `flows::*` the keybinding would call.
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
