use crate::ports::BwError;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::export::{ExportFocus, ExportState};
use crate::tui::screens::Screen;
use crate::tui::worker::{InFlight, WorkerRequest};

pub fn open(app: &mut App) {
    app.export = Some(ExportState::new());
    app.screen = Screen::Export;
}

pub fn cancel(app: &mut App) {
    app.export = None;
    app.screen = Screen::Vault;
}

pub fn focus_step(app: &mut App, dir: i32) {
    let Some(state) = app.export.as_mut() else {
        return;
    };
    state.focus = match (state.focus, dir.signum()) {
        (ExportFocus::Format, _) => ExportFocus::Path,
        (ExportFocus::Path, _) => ExportFocus::Format,
    };
}

pub fn cycle_format(app: &mut App) {
    let Some(state) = app.export.as_mut() else {
        return;
    };
    let text = state.path.text();
    let old_default_prefix = text.starts_with(
        &text
            .rsplit_once('-')
            .map(|(p, _)| p.to_string())
            .unwrap_or_default(),
    );
    let dot = text.rfind('.');
    let looks_default = text.contains("bytewarden-export-");
    state.format = state.format.next();

    if old_default_prefix
        && looks_default
        && let Some(dot) = dot
    {
        let mut new_path = state.path.text()[..dot + 1].to_string();
        new_path.push_str(state.format.extension());
        state.path.set(new_path);
    }
}

pub fn commit(app: &mut App) {
    let Some(state) = app.export.as_ref() else {
        return;
    };
    let path = state.path.text().trim().to_string();
    if path.is_empty() {
        app.set_action(ActionState::Error("Output path cannot be empty.".into()));
        return;
    }
    let pb = std::path::PathBuf::from(&path);

    if pb.exists() {
        app.set_action(ActionState::Error(format!(
            "File already exists: {} — pick another name or remove it first.",
            short_path(&path)
        )));
        return;
    }

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
    app.submit(
        InFlight::Export,
        "Exporting…",
        WorkerRequest::Export {
            format: format.cli_arg().to_string(),
            path,
        },
    );
}

pub fn handle(app: &mut App, r: Result<(), BwError>) {
    let (path, format) = match app.export.as_ref() {
        Some(s) => (s.path.text().trim().to_string(), s.format),
        None => return,
    };
    let cmd = format!("bw export --format {} --output <path>", format.cli_arg());
    match r {
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

fn short_path(path: &str) -> String {
    if path.chars().count() <= 50 {
        return path.to_string();
    }
    let chars: Vec<char> = path.chars().collect();
    let head: String = chars[..20].iter().collect();
    let tail: String = chars[chars.len() - 27..].iter().collect();
    format!("{head}…{tail}")
}
