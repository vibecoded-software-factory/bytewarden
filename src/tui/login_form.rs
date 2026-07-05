//! Login-screen form state.
//!
//! The buffers, focus, toggles and transient auth-challenge flags that
//! only matter while the user is on the login screen, split out of the
//! [`crate::tui::app::App`] god-struct into one cohesive, screen-local
//! unit. It follows the same shape the popup states already use — a
//! state container that owns its own small helpers — so `App` sheds a
//! cluster of flat fields without changing any behaviour.
//!
//! Session/auth *state* (`authenticated`) deliberately stays on `App`:
//! it outlives the login screen (it tracks logged-in vs signed-out for
//! the whole session), so it isn't login-*form* state.

use crate::domain::{LineEditor, TwoFactorMethod};
use crate::tui::screens::LoginField;

/// The login screen's form buffers, focus and toggles.
pub struct LoginForm {
    /// Current Bitwarden server URL — populated from `bw status` at
    /// boot and editable from the login screen.
    pub server_input: LineEditor,
    /// Server URL as last persisted by `bw config server`. Used to
    /// decide whether the field is dirty and needs a re-config call.
    pub server_committed: String,
    /// E-mail buffer.
    pub email_input: LineEditor,
    /// Master password buffer. [`LineEditor`] is `ZeroizeOnDrop`, so the
    /// bytes are overwritten when the field is cleared (or when the form
    /// drops) — no plaintext copy lingers in the heap after login.
    pub password_input: LineEditor,
    /// One-time-code buffer. Same zeroizing rationale as
    /// `password_input` — short-lived but worth scrubbing.
    pub otp_input: LineEditor,
    /// Toggled on after the backend reports a "new device" challenge —
    /// the user has to paste the code bw e-mailed them.
    pub otp_required: bool,
    /// Toggled on after the backend reports the account has a
    /// permanent second factor enrolled (Authenticator / Email /
    /// YubiKey). The same `otp_input` buffer is reused, but the
    /// submit path branches to
    /// [`crate::ports::VaultPort::login_with_two_factor`] instead of
    /// the device-verification path. Mutually exclusive with
    /// `otp_required`.
    pub two_factor_required: bool,
    /// Currently selected 2FA method. Defaults to
    /// [`TwoFactorMethod::Authenticator`] (the most common case); the
    /// user cycles it from the login form when the popup is up. Only
    /// meaningful when `two_factor_required` is `true`.
    pub two_factor_method: TwoFactorMethod,
    /// Which login row currently holds focus.
    pub active_field: LoginField,
    /// Latched after a failed login — the view renders an error strip.
    pub login_error: bool,
    /// Whether the "save e-mail" box is ticked (persisted to settings).
    pub save_email: bool,
    /// Whether the unlocked session key should be persisted to a
    /// per-PPID runtime file (cleaned up when the parent shell dies).
    pub keep_session: bool,
    /// Whether the master password on the login screen is shown in plain
    /// text.
    pub password_visible: bool,
}

impl LoginForm {
    /// Builds the initial form. `saved_email` seeds the e-mail field
    /// (empty when the user never opted to save it); `save_email` /
    /// `keep_session` come from settings. Focus starts on the password
    /// field when an e-mail is already remembered, otherwise on e-mail.
    pub fn new(saved_email: String, save_email: bool, keep_session: bool) -> Self {
        Self {
            server_input: LineEditor::new(),
            server_committed: String::new(),
            email_input: LineEditor::with_text(saved_email),
            password_input: LineEditor::new(),
            otp_input: LineEditor::new(),
            otp_required: false,
            two_factor_required: false,
            two_factor_method: TwoFactorMethod::Authenticator,
            active_field: if save_email {
                LoginField::Password
            } else {
                LoginField::Email
            },
            login_error: false,
            save_email,
            keep_session,
            password_visible: false,
        }
    }

    /// `true` while bw is asking the user for an interactive code —
    /// either a device-verification OTP (e-mailed on first login from
    /// a new device) or a permanent second-factor code (Authenticator).
    /// Layout, click hit-testing and tab order use this rather than
    /// branching on the two flags individually.
    pub fn awaiting_code(&self) -> bool {
        self.otp_required || self.two_factor_required
    }

    /// Latches the login-error strip and resets the form back to the
    /// password field, wiping the secret buffers so a stale code /
    /// password can't linger after a failed attempt.
    pub fn set_error(&mut self) {
        self.login_error = true;
        self.password_input.clear();
        self.otp_input.clear();
        self.otp_required = false;
        self.two_factor_required = false;
        self.active_field = LoginField::Password;
    }

    /// Clears the login-error strip.
    pub fn clear_error(&mut self) {
        self.login_error = false;
    }

    /// Returns the [`LineEditor`] for the focused login text field, or
    /// `None` for the checkbox fields. The caller drives it through
    /// [`crate::tui::input::common::route_line_editor`] like every other
    /// input.
    pub fn editor_mut(&mut self) -> Option<&mut LineEditor> {
        match self.active_field {
            LoginField::Server => Some(&mut self.server_input),
            LoginField::Email => Some(&mut self.email_input),
            LoginField::Password => Some(&mut self.password_input),
            LoginField::Otp => Some(&mut self.otp_input),
            _ => None,
        }
    }
}
