use crate::ports::BwError;
use crate::ports::GeneratorMode;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::generator::{
    GeneratorFocus, GeneratorState, ReturnTarget, focus_index, focusable_for,
};
use crate::tui::screens::Screen;
use crate::tui::worker::{InFlight, WorkerRequest};

pub fn open_standalone(app: &mut App) {
    app.generator = GeneratorState::default();
    app.screen = Screen::Generator;
}

pub fn open_for_edit_field(app: &mut App, idx: usize) {
    app.generator = GeneratorState {
        return_target: Some(ReturnTarget::EditField(idx)),
        ..GeneratorState::default()
    };
    app.screen = Screen::Generator;
}

pub fn open_for_create_field(app: &mut App, idx: usize) {
    app.generator = GeneratorState {
        return_target: Some(ReturnTarget::CreateField(idx)),
        ..GeneratorState::default()
    };
    app.screen = Screen::Generator;
}

pub fn request_generate(app: &mut App) {
    let opts = app.generator.options.clone();
    app.submit(
        InFlight::Generate,
        "Generating…",
        WorkerRequest::Generate { opts },
    );
}

pub fn handle(app: &mut App, r: Result<String, BwError>) {
    match r {
        Ok(value) => {
            let cmd = describe_cmd(&app.generator.options);
            app.push_cmd(&cmd, true, "generated [hidden]");

            app.generator.result = zeroize::Zeroizing::new(value);
            app.set_action(ActionState::Done("Generated ✓".into()));
        }
        Err(e) => app.cmd_err("bw generate", &e, "Generate failed"),
    }
}

pub fn copy_result(app: &mut App) {
    if app.generator.result.is_empty() {
        app.set_action(ActionState::Error("Nothing to copy yet.".into()));
        return;
    }
    let value = app.generator.result.clone();
    let ttl = app.clipboard_clear_secs;
    match app.clipboard.write_with_clear(&value, ttl) {
        Ok(()) => {
            app.push_cmd("clipboard", true, "generated value [hidden]");
            let toast = if ttl == 0 {
                "Copied ✓".to_string()
            } else {
                format!("Copied ✓ (clears in {ttl}s)")
            };
            app.set_action(ActionState::Done(toast));
        }
        Err(e) => {
            app.push_cmd("clipboard", false, &e);
            app.set_action(ActionState::Error(format!("Clipboard error: {e}")));
        }
    }
}

pub fn use_result(app: &mut App) {
    if app.generator.result.is_empty() {
        app.set_action(ActionState::Error("Nothing to use yet.".into()));
        return;
    }
    let Some(target) = app.generator.return_target else {
        app.set_action(ActionState::Error(
            "Open the generator from a Password field to use a result.".into(),
        ));
        return;
    };

    let value = app.generator.result.clone();
    match target {
        ReturnTarget::EditField(idx) => {
            if let Some(field) = app.edit.fields.get_mut(idx) {
                field.editor.set(value.as_str());
            }
            app.screen = Screen::Detail;
            app.edit.active = true;
        }
        ReturnTarget::CreateField(idx) => {
            if let Some(field) = app.create.fields.get_mut(idx) {
                field.editor.set(value.as_str());
            }
            app.screen = Screen::Create;
        }
    }
    app.set_action(ActionState::Done("Used ✓".into()));
}

pub fn cancel(app: &mut App) {
    match app.generator.return_target {
        Some(ReturnTarget::EditField(_)) => {
            app.screen = Screen::Detail;
            app.edit.active = true;
        }
        Some(ReturnTarget::CreateField(_)) => {
            app.screen = Screen::Create;
        }
        None => {
            app.screen = Screen::Vault;
        }
    }
    app.set_action(ActionState::Idle);
}

pub fn toggle_mode(app: &mut App) {
    app.generator.options.mode = match app.generator.options.mode {
        GeneratorMode::Password => GeneratorMode::Passphrase,
        GeneratorMode::Passphrase => GeneratorMode::Password,
    };

    app.generator.focus = focusable_for(app.generator.options.mode)
        .iter()
        .copied()
        .find(|f| *f != GeneratorFocus::Mode)
        .unwrap_or(GeneratorFocus::Mode);

    app.generator.result.clear();
}

pub fn focus_step(app: &mut App, dir: i32) {
    let list = focusable_for(app.generator.options.mode);
    if list.is_empty() {
        return;
    }
    let cur = focus_index(app.generator.options.mode, app.generator.focus);
    let next = (cur as i32 + dir).rem_euclid(list.len() as i32) as usize;
    app.generator.focus = list[next];
}

fn describe_cmd(opts: &crate::ports::GeneratorOptions) -> String {
    let mut parts: Vec<String> = vec!["bw generate".into()];
    match opts.mode {
        GeneratorMode::Password => {
            if opts.uppercase {
                parts.push("-u".into());
            }
            if opts.lowercase {
                parts.push("-l".into());
            }
            if opts.numbers {
                parts.push("-n".into());
            }
            if opts.special {
                parts.push("-s".into());
            }
            if opts.avoid_ambiguous {
                parts.push("--ambiguous".into());
            }
            parts.push(format!("--length {}", opts.length));
        }
        GeneratorMode::Passphrase => {
            parts.push("-p".into());
            parts.push(format!("--words {}", opts.words));
            if !opts.separator.is_empty() {
                parts.push(format!("--separator {}", opts.separator));
            }
            if opts.capitalize {
                parts.push("-c".into());
            }
            if opts.include_number {
                parts.push("--includeNumber".into());
            }
        }
    }
    parts.join(" ")
}
