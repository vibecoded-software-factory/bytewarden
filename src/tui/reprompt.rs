//! Reprompt popup state.
//!
//! Surfaces when the user attempts a secret-exposing action (copy
//! password / TOTP / hidden custom field, F2 reveal) on an item with
//! the Bitwarden `reprompt` flag set. The popup asks for the master
//! password, runs `bw unlock` to verify, and on success runs the
//! deferred [`ProtectedAction`].
//!
//! Verification is **not** cached — every protected action triggers a
//! fresh prompt, which matches the official Bitwarden GUI behaviour
//! and the user's stated preference.

use crate::domain::LineEditor;

/// What action to run after the user successfully reverifies their
/// master password. The popup carries one of these so it knows which
/// flow to resume.
///
/// Each variant maps to one of the secret-exposing call paths in
/// [`crate::tui::flows::copy`] / [`crate::tui::input::detail`].
#[derive(Debug, Clone)]
pub enum ProtectedAction {
    /// Copy the selected item's password (`Alt+C` from the vault).
    CopyPassword,
    /// Copy the selected item's TOTP code (delegated to
    /// `bw get totp`).
    CopyTotp(String),
    /// Re-run the detail screen's "copy focused field" path. The
    /// focused field is captured implicitly by `app.detail_field` —
    /// no explicit index here because the user can't navigate the
    /// list while the popup is open.
    CopySelectedDetailField,
    /// Toggle `app.show_password` (the F2 reveal flag) on the detail
    /// screen.
    RevealDetail,
    /// Toggle the focused edit-form field's `revealed` flag.
    RevealEditField,
}

/// Buffer for the in-flight reprompt popup.
///
/// `input` is a [`LineEditor`], which is `ZeroizeOnDrop`, so the
/// master-password buffer gets scrubbed when the popup drops — same
/// hygiene as the login form's password field.
#[derive(Debug)]
pub struct RepromptState {
    /// Typed master password. Scrubbed when the popup closes (Esc,
    /// success, or failure clearing the buffer to retry).
    pub input: LineEditor,
    /// What to run once verification succeeds.
    pub after: ProtectedAction,
    /// `true` after a failed verify — the view shows an error strip
    /// and we keep the popup open so the user can retry.
    pub error: bool,
    /// Screen the user was on when the popup opened. The renderer
    /// uses it to draw the right context underneath (Vault for
    /// `Alt+C`, Detail for F2 / Alt+C on a hidden row), and the
    /// success/cancel paths use it to put the user back where they
    /// were.
    pub origin: crate::tui::screens::Screen,
}

impl RepromptState {
    /// Builds a fresh popup state for the given protected action.
    pub fn new(after: ProtectedAction, origin: crate::tui::screens::Screen) -> Self {
        Self {
            input: LineEditor::new(),
            after,
            error: false,
            origin,
        }
    }
}
