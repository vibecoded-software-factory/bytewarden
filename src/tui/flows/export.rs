//! Vault-export popup flow.

use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::export::{ExportFocus, ExportState};
use crate::tui::screens::Screen;

/// Opens the export popup with default values (JSON format, default
/// download path).
pub fn open(app: &mut App) {
    app.export = Some(ExportState::new());
    app.screen = Screen::Export;
}

/// Closes the popup and returns to the vault list.
pub fn cancel(app: &mut App) {
    app.export = None;
    app.screen = Screen::Vault;
}

/// Cycles focus between the format picker and the path field.
pub fn focus_step(app: &mut App, dir: i32) {
    let Some(state) = app.export.as_mut() else {
        return;
    };
    state.focus = match (state.focus, dir.signum()) {
        (ExportFocus::Format, _) => ExportFocus::Path,
        (ExportFocus::Path, _) => ExportFocus::Format,
    };
}

/// Cycles the format forward and refreshes the default path so the
/// extension matches the new format. No-op if the path field has
/// been edited away from the default.
pub fn cycle_format(app: &mut App) {
    let Some(state) = app.export.as_mut() else {
        return;
    };
    let old_default_prefix = state.path.starts_with(
        &state
            .path
            .rsplit_once('-')
            .map(|(p, _)| p.to_string())
            .unwrap_or_default(),
    );
    state.format = state.format.next();
    // Only auto-refresh the path when the user hasn't edited it.
    // Heuristic: keep the prefix `bytewarden-export-` and the unix
    // timestamp; just swap the extension.
    if old_default_prefix
        && state.path.contains("bytewarden-export-")
        && let Some(dot) = state.path.rfind('.')
    {
        state.path.truncate(dot + 1);
        state.path.push_str(state.format.extension());
        state.path_cursor = state.path.chars().count();
    }
}

/// Runs `bw export` and reports the outcome as a toast. Closes the
/// popup on success; keeps it open on failure so the user can fix
/// the path and retry.
pub fn commit(app: &mut App) {
    let Some(state) = app.export.as_ref() else {
        return;
    };
    let path = state.path.trim().to_string();
    if path.is_empty() {
        app.set_action(ActionState::Error("Output path cannot be empty.".into()));
        return;
    }
    let pb = std::path::PathBuf::from(&path);
    // Refuse to overwrite — exports usually contain plaintext
    // credentials and a silent overwrite of an unrelated file would
    // be a nasty surprise.
    if pb.exists() {
        app.set_action(ActionState::Error(format!(
            "File already exists: {} — pick another name or remove it first.",
            short_path(&path)
        )));
        return;
    }
    // The destination directory must exist; bw is happy to create the
    // file but not the directory tree above it.
    if let Some(parent) = pb.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        app.set_action(ActionState::Error(format!(
            "Output directory does not exist: {}",
            parent.display()
        )));
        return;
    }
    let format = state.format;
    let cmd = format!("bw export --format {} --output <path>", format.cli_arg());
    app.set_action(ActionState::Running("Exporting…".into()));
    match app.vault.export(format.cli_arg(), &path) {
        Ok(()) => {
            app.push_cmd(&cmd, true, &format!("exported to {path}"));
            app.set_action(ActionState::Done(format!(
                "Exported to {} ✓",
                short_path(&path)
            )));
            app.export = None;
            app.screen = Screen::Vault;
        }
        Err(e) => app.cmd_err(&cmd, &e, "Export failed"),
    }
}

/// Truncates a long path for the toast — keeps the start and the end
/// so the user can see the filename.
fn short_path(path: &str) -> String {
    if path.chars().count() <= 50 {
        return path.to_string();
    }
    let chars: Vec<char> = path.chars().collect();
    let head: String = chars[..20].iter().collect();
    let tail: String = chars[chars.len() - 27..].iter().collect();
    format!("{head}…{tail}")
}
