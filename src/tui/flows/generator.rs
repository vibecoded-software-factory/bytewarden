//! Open / regenerate / copy / use flows for the password generator.

use crate::ports::GeneratorMode;
use crate::tui::action::{ActionState, PendingAction};
use crate::tui::app::App;
use crate::tui::generator::{
    GeneratorFocus, GeneratorState, ReturnTarget, focus_index, focusable_for,
};
use crate::tui::screens::Screen;

/// Opens the generator screen in standalone mode (no return target).
///
/// The result box stays empty until the user explicitly presses Enter —
/// auto-generating on open would surprise the user with an output they
/// did not configure.
pub fn open_standalone(app: &mut App) {
    app.generator = GeneratorState::default();
    app.screen = Screen::Generator;
}

/// Opens the generator screen with a return target pointing at one of
/// the rows of the edit form. Pressing "Use" populates that row.
pub fn open_for_edit_field(app: &mut App, idx: usize) {
    app.generator = GeneratorState {
        return_target: Some(ReturnTarget::EditField(idx)),
        ..GeneratorState::default()
    };
    app.screen = Screen::Generator;
}

/// Opens the generator screen with a return target pointing at one of
/// the rows of the create form.
pub fn open_for_create_field(app: &mut App, idx: usize) {
    app.generator = GeneratorState {
        return_target: Some(ReturnTarget::CreateField(idx)),
        ..GeneratorState::default()
    };
    app.screen = Screen::Generator;
}

/// Queues a [`PendingAction::GeneratePassword`] — the actual `bw
/// generate` call runs on the next tick so the spinner is visible
/// before the (typically <300 ms) blocking call.
pub fn queue_regenerate(app: &mut App) {
    app.set_action(ActionState::Running("Generating…".into()));
    app.pending_action = PendingAction::GeneratePassword;
}

/// Pending-action executor for [`PendingAction::GeneratePassword`].
pub fn do_generate(app: &mut App) {
    let opts = app.generator.options.clone();
    match app.generator_port.generate(&opts) {
        Ok(value) => {
            let cmd = describe_cmd(&opts);
            app.push_cmd(&cmd, true, "generated [hidden]");
            app.generator.result = value;
            app.set_action(ActionState::Done("Generated ✓".into()));
        }
        Err(e) => app.cmd_err("bw generate", &e, "Generate failed"),
    }
}

/// Copies the current result to the clipboard via the injected port.
pub fn copy_result(app: &mut App) {
    if app.generator.result.is_empty() {
        app.set_action(ActionState::Error("Nothing to copy yet.".into()));
        return;
    }
    let value = app.generator.result.clone();
    match app.clipboard.write(&value) {
        Ok(()) => {
            app.push_cmd("clipboard", true, "generated value [hidden]");
            app.set_action(ActionState::Done("Copied ✓".into()));
        }
        Err(e) => {
            app.push_cmd("clipboard", false, &e);
            app.set_action(ActionState::Error(format!("Clipboard error: {e}")));
        }
    }
}

/// Writes the current result into the form field referenced by the
/// stored [`ReturnTarget`] and switches back to that screen.
///
/// No-op when there's no return target (standalone mode) or no result.
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
            if let Some(field) = app.edit_fields.get_mut(idx) {
                field.value = value;
                field.cursor = field.value.chars().count();
            }
            app.screen = Screen::Detail;
            app.edit_mode = true;
        }
        ReturnTarget::CreateField(idx) => {
            if let Some(field) = app.create_fields.get_mut(idx) {
                field.value = value;
                field.cursor = field.value.chars().count();
            }
            app.screen = Screen::Create;
        }
    }
    app.set_action(ActionState::Done("Used ✓".into()));
}

/// Closes the generator and returns to the calling screen, discarding
/// the in-flight result.
pub fn cancel(app: &mut App) {
    match app.generator.return_target {
        Some(ReturnTarget::EditField(_)) => {
            app.screen = Screen::Detail;
            app.edit_mode = true;
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

// ── Mode + focus navigation helpers ───────────────────────────────────────

/// Switches between Password and Passphrase mode and snaps the focus
/// back onto a valid control for the new mode.
///
/// Does *not* auto-generate — the user explicitly triggers generation
/// with Enter once they're done configuring.
pub fn toggle_mode(app: &mut App) {
    app.generator.options.mode = match app.generator.options.mode {
        GeneratorMode::Password => GeneratorMode::Passphrase,
        GeneratorMode::Passphrase => GeneratorMode::Password,
    };
    // Pick the first non-Mode focus so the user is parked on a
    // meaningful row (Mode itself is already correctly highlighted by
    // virtue of `focus == Mode`, but we move on).
    app.generator.focus = focusable_for(app.generator.options.mode)
        .iter()
        .copied()
        .find(|f| *f != GeneratorFocus::Mode)
        .unwrap_or(GeneratorFocus::Mode);
    // Stale result is cleared so the user does not see a value that
    // no longer matches the new mode.
    app.generator.result.clear();
}

/// Cycles focus by `dir` (+1 down, -1 up) inside the focusable list
/// for the active mode.
pub fn focus_step(app: &mut App, dir: i32) {
    let list = focusable_for(app.generator.options.mode);
    if list.is_empty() {
        return;
    }
    let cur = focus_index(app.generator.options.mode, app.generator.focus);
    let next = (cur as i32 + dir).rem_euclid(list.len() as i32) as usize;
    app.generator.focus = list[next];
}

/// Builds a redacted shell representation of the current generator
/// invocation, for the cmd-log panel.
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
