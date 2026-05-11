//! Reprompt popup flow: opening, verifying, dispatching the deferred
//! protected action.
//!
//! Verification reuses [`crate::ports::VaultPort::unlock`] — the only
//! way bw exposes "is this password the master password?" without
//! recreating the session. On success bw issues a fresh session key
//! that replaces the in-memory one; the previous key is discarded.
//!
//! There is **no** caching of the verified state: every protected
//! action triggers its own popup. That matches the official Bitwarden
//! GUI and the user's stated preference.

use zeroize::Zeroizing;

use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::reprompt::{ProtectedAction, RepromptState};
use crate::tui::screens::Screen;

/// Opens the popup if the currently selected item carries the
/// `reprompt` flag, otherwise runs `action` immediately.
///
/// Returns `true` when the popup was opened (the caller should
/// consider the action *deferred* — do not also run it inline);
/// `false` means there was no reprompt and the caller should fall
/// through to the regular execution path.
///
/// Honors [`App::reprompt_verified`] — if the flag is set we
/// consume it and return `false`, so the deferred action that
/// follows a successful verify doesn't loop straight back into the
/// popup.
pub fn maybe_open(app: &mut App, action: ProtectedAction) -> bool {
    if app.reprompt_verified {
        app.reprompt_verified = false;
        return false;
    }
    let needs = app.selected_item().is_some_and(|i| i.needs_reprompt());
    if !needs {
        return false;
    }
    let origin = app.screen.clone();
    app.reprompt = Some(RepromptState::new(action, origin));
    app.screen = Screen::RepromptUnlock;
    true
}

/// Cancels the popup and returns to the originating screen without
/// running the deferred action. The buffer's `Zeroizing<String>`
/// scrubs the typed password as it drops with the state.
pub fn cancel(app: &mut App) {
    let origin = app
        .reprompt
        .as_ref()
        .map(|s| s.origin.clone())
        .unwrap_or(Screen::Vault);
    app.reprompt = None;
    app.screen = origin;
}

/// Calls `bw unlock <password>` and, on success, runs the deferred
/// action. On failure leaves the popup open with the error strip
/// turned on so the user can retry.
///
/// `bw unlock` issues a brand-new session key when it succeeds; the
/// adapter swaps it in transparently. The old key is invalidated by
/// bw on the next sync but is no longer referenced by anything in
/// our process either way.
pub fn verify_and_run(app: &mut App) {
    let Some(state) = app.reprompt.as_ref() else {
        return;
    };
    if state.input.is_empty() {
        // Empty input — turn on the error strip so the user knows
        // the press registered, without paying for a `bw unlock`.
        if let Some(s) = app.reprompt.as_mut() {
            s.error = true;
        }
        return;
    }

    // Snapshot what we need before the borrow chain forces us to
    // drop the reference. Wrap the password in `Zeroizing` so the
    // intermediate copy used by `vault.unlock` gets scrubbed.
    let password = Zeroizing::new(state.input.as_str().to_string());
    let action = state.after.clone();
    let origin = state.origin.clone();

    match app.vault.unlock(&password) {
        Ok(_) => {
            // bw issued a fresh session key — the adapter has it now.
            // Drop the popup state (scrubs the password buffer) and
            // execute the deferred action.
            app.push_cmd("bw unlock *** (reprompt)", true, "verified");
            app.reprompt = None;
            app.screen = origin;
            run_protected_action(app, action);
        }
        Err(_) => {
            // Don't push the underlying bw error to the cmd log —
            // the wording can be surprising ("Invalid master
            // password.") and we already surface it via the popup's
            // own error strip.
            app.push_cmd("bw unlock *** (reprompt)", false, "verification failed");
            if let Some(s) = app.reprompt.as_mut() {
                s.error = true;
                s.input.clear();
                s.cursor = 0;
            }
        }
    }
}

/// Dispatches the action that the user originally tried to run.
///
/// Sets [`App::reprompt_verified`] so the guards re-checked inside
/// the deferred path treat this single dispatch as already verified.
/// The flag is consumed by the first `maybe_open` it crosses.
fn run_protected_action(app: &mut App, action: ProtectedAction) {
    use crate::tui::flows::copy;
    app.reprompt_verified = true;
    match action {
        ProtectedAction::CopyPassword => copy::copy_password_to_clipboard(app),
        ProtectedAction::CopyTotp(item_id) => {
            // Reuse the same wiring the regular Alt+C-on-TOTP path
            // takes — queue a CopyTotp pending action so the user
            // sees the spinner, just like an unprotected copy.
            app.set_action(ActionState::Running("Copying TOTP…".into()));
            app.pending_action = crate::tui::action::PendingAction::CopyTotp(item_id);
        }
        ProtectedAction::CopySelectedDetailField => copy::copy_selected_field(app),
        ProtectedAction::RevealDetail => {
            app.show_password = !app.show_password;
        }
        ProtectedAction::RevealEditField => {
            app.edit_toggle_reveal();
        }
    }
    // Safety net: if the action's path didn't trip a guard (e.g. it
    // queued a pending action without re-entering `maybe_open`), the
    // flag is no longer needed. Clear it so a later unrelated action
    // doesn't get a free pass.
    app.reprompt_verified = false;
}
