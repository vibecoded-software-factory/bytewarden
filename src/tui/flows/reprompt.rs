use crate::ports::BwError;
use zeroize::Zeroizing;

use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::reprompt::{ProtectedAction, RepromptState};
use crate::tui::screens::Screen;
use crate::tui::worker::{InFlight, WorkerRequest};

pub fn maybe_open(app: &mut App, action: ProtectedAction) -> bool {
    if app.reprompt_verified {
        app.reprompt_verified = false;
        return false;
    }
    let needs = app
        .vault
        .selected_item()
        .is_some_and(|i| i.needs_reprompt());
    if !needs {
        return false;
    }
    let origin = app.screen.clone();
    app.reprompt = Some(RepromptState::new(action, origin));
    app.screen = Screen::RepromptUnlock;
    true
}

pub fn cancel(app: &mut App) {
    let origin = app
        .reprompt
        .as_ref()
        .map(|s| s.origin.clone())
        .unwrap_or(Screen::Vault);
    app.reprompt = None;
    app.screen = origin;
}

pub fn verify_and_run(app: &mut App) {
    let Some(state) = app.reprompt.as_ref() else {
        return;
    };
    if state.input.is_empty() {
        if let Some(s) = app.reprompt.as_mut() {
            s.error = true;
        }
        return;
    }

    let password = Zeroizing::new(state.input.as_str().to_string());
    app.submit(
        InFlight::RepromptUnlock,
        "Verifying…",
        WorkerRequest::Unlock { password },
    );
}

pub fn handle_unlock(app: &mut App, r: Result<String, BwError>) {
    match r {
        Ok(key) => {
            app.session_marker = Some(Zeroizing::new(key));
            app.push_cmd("bw unlock *** (reprompt)", true, "verified");
            let Some((action, origin)) = app
                .reprompt
                .as_ref()
                .map(|s| (s.after.clone(), s.origin.clone()))
            else {
                return;
            };
            app.reprompt = None;
            app.screen = origin;
            run_protected_action(app, action);
        }
        Err(_) => {
            app.push_cmd("bw unlock *** (reprompt)", false, "verification failed");
            app.set_action(ActionState::Idle);
            if let Some(s) = app.reprompt.as_mut() {
                s.error = true;
                s.input.clear();
            }
        }
    }
}

fn run_protected_action(app: &mut App, action: ProtectedAction) {
    use crate::tui::flows::copy;
    app.reprompt_verified = true;
    match action {
        ProtectedAction::CopyPassword => copy::copy_password_to_clipboard(app),
        ProtectedAction::CopyTotp(item_id) => {
            copy::request_copy_totp(app, item_id);
        }
        ProtectedAction::CopySelectedDetailField => copy::copy_selected_field(app),
        ProtectedAction::RevealDetail => {
            app.show_password = !app.show_password;
        }
        ProtectedAction::RevealEditField => {
            app.edit.toggle_reveal();
        }
    }

    app.reprompt_verified = false;
}
