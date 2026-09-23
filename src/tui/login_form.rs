use crate::domain::{LineEditor, TwoFactorMethod};
use crate::tui::screens::LoginField;

pub struct LoginForm {
    pub server_input: LineEditor,

    pub server_committed: String,

    pub email_input: LineEditor,

    pub password_input: LineEditor,

    pub otp_input: LineEditor,

    pub otp_required: bool,

    pub two_factor_required: bool,

    pub two_factor_method: TwoFactorMethod,

    pub active_field: LoginField,

    pub login_error: bool,

    pub save_email: bool,

    pub keep_session: bool,

    pub password_visible: bool,
}

impl LoginForm {
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

    pub fn awaiting_code(&self) -> bool {
        self.otp_required || self.two_factor_required
    }

    pub fn set_error(&mut self) {
        self.login_error = true;
        self.password_input.clear();
        self.otp_input.clear();
        self.otp_required = false;
        self.two_factor_required = false;
        self.active_field = LoginField::Password;
    }

    pub fn clear_error(&mut self) {
        self.login_error = false;
    }

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
