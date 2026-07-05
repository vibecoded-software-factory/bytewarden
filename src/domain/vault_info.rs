//! Vault status and authentication outcome types.

/// Permanent second-factor methods supported by `bw login --method`.
///
/// The numeric discriminants match the values bw expects on the
/// command line (`0` Authenticator, `1` Email, `3` YubiKey). Value
/// `2` (Duo) and the WebAuthn-family methods aren't covered because
/// they require a browser callback that bytewarden's blocking-CLI
/// flow can't drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwoFactorMethod {
    /// TOTP authenticator app (`bw login --method 0`).
    Authenticator,
    /// Email-delivered 2FA code (`bw login --method 1`).
    Email,
    /// YubiKey OTP (`bw login --method 3`).
    YubiKey,
}

impl TwoFactorMethod {
    /// Human-readable label used in the login form.
    pub fn label(self) -> &'static str {
        match self {
            TwoFactorMethod::Authenticator => "Authenticator",
            TwoFactorMethod::Email => "Email",
            TwoFactorMethod::YubiKey => "YubiKey",
        }
    }

    /// Numeric value passed to `bw login --method N`.
    pub fn as_u8(self) -> u8 {
        match self {
            TwoFactorMethod::Authenticator => 0,
            TwoFactorMethod::Email => 1,
            TwoFactorMethod::YubiKey => 3,
        }
    }

    /// Returns the next method in the cycle (Authenticator →
    /// Email → YubiKey → Authenticator). Used by the login screen's
    /// `← →` cycler.
    pub fn next(self) -> Self {
        match self {
            TwoFactorMethod::Authenticator => TwoFactorMethod::Email,
            TwoFactorMethod::Email => TwoFactorMethod::YubiKey,
            TwoFactorMethod::YubiKey => TwoFactorMethod::Authenticator,
        }
    }

    /// Returns the previous method in the cycle.
    pub fn prev(self) -> Self {
        match self {
            TwoFactorMethod::Authenticator => TwoFactorMethod::YubiKey,
            TwoFactorMethod::Email => TwoFactorMethod::Authenticator,
            TwoFactorMethod::YubiKey => TwoFactorMethod::Email,
        }
    }
}

/// Three-state authentication status returned by a vault backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultStatus {
    /// Authenticated and the master key is in memory — items can be read.
    Unlocked,
    /// Authenticated but no master key in memory — `unlock` is required.
    Locked,
    /// No active session — `login` is required.
    Unauthenticated,
}

/// Snapshot of the vault's authentication state plus context.
///
/// Returned by [`crate::ports::VaultPort::status`].
#[derive(Debug, Clone)]
pub struct VaultInfo {
    /// Current authentication status.
    pub status: VaultStatus,

    /// Logged-in user e-mail (if any).
    pub user_email: Option<String>,

    /// ISO timestamp of the last successful sync. Parsed from
    /// `bw status` for completeness; not surfaced in the UI yet.
    pub last_sync: Option<String>,

    /// Bitwarden server URL — non-default for self-hosted instances.
    /// Seeded into the Login form's Server field at boot.
    pub server_url: Option<String>,
}

/// Outcome of [`crate::ports::VaultPort::login`].
///
/// A login attempt can land on one of four outcomes. The two
/// interactive ones (`NeedsDeviceVerification` and `NeedsTwoFactor`)
/// look similar at the UI level — both ask the user for a six-ish
/// digit code — but they require *different* follow-up calls:
///
/// * **`NeedsDeviceVerification`** — the backend doesn't recognise the
///   device this user is logging in from and has e-mailed them a
///   one-time code. Resolve by calling
///   [`crate::ports::VaultPort::login_with_otp`]. No `--method` flag
///   is involved; bw matches the prompt automatically.
/// * **`NeedsTwoFactor`** — the user has a *permanent* second-factor
///   enrolled on their account (Authenticator app, YubiKey, …) and bw
///   needs both the code and the method (`--method N`). Resolve by
///   calling [`crate::ports::VaultPort::login_with_two_factor`].
///   Bytewarden currently only drives the Authenticator (TOTP) method;
///   accounts with YubiKey or Email 2FA need to log in from the
///   official client.
#[derive(Debug)]
pub enum LoginOutcome {
    /// Success — the inner string is the session key.
    Success(String),
    /// The backend detected a new device and sent an OTP to the user
    /// e-mail. Resolved via
    /// [`crate::ports::VaultPort::login_with_otp`].
    NeedsDeviceVerification,
    /// The account has a permanent second factor enrolled and bw is
    /// prompting for the code. Resolved via
    /// [`crate::ports::VaultPort::login_with_two_factor`].
    NeedsTwoFactor,
    /// Authentication failed — the inner string carries the underlying error.
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_status_equality_is_value_based() {
        assert_eq!(VaultStatus::Locked, VaultStatus::Locked);
        assert_ne!(VaultStatus::Locked, VaultStatus::Unlocked);
        assert_ne!(VaultStatus::Locked, VaultStatus::Unauthenticated);
    }

    #[test]
    fn two_factor_method_maps_to_bw_numeric() {
        assert_eq!(TwoFactorMethod::Authenticator.as_u8(), 0);
        assert_eq!(TwoFactorMethod::Email.as_u8(), 1);
        assert_eq!(TwoFactorMethod::YubiKey.as_u8(), 3);
    }

    #[test]
    fn two_factor_method_cycles_in_a_loop() {
        // next/prev must be true inverses and form a 3-cycle so the
        // login form's ← → cycler is always reversible.
        assert_eq!(
            TwoFactorMethod::Authenticator.next(),
            TwoFactorMethod::Email
        );
        assert_eq!(TwoFactorMethod::Email.next(), TwoFactorMethod::YubiKey);
        assert_eq!(
            TwoFactorMethod::YubiKey.next(),
            TwoFactorMethod::Authenticator
        );
        for m in [
            TwoFactorMethod::Authenticator,
            TwoFactorMethod::Email,
            TwoFactorMethod::YubiKey,
        ] {
            assert_eq!(m.next().prev(), m);
            assert_eq!(m.prev().next(), m);
        }
    }

    #[test]
    fn two_factor_method_label_is_human_readable() {
        assert_eq!(TwoFactorMethod::Authenticator.label(), "Authenticator");
        assert_eq!(TwoFactorMethod::Email.label(), "Email");
        assert_eq!(TwoFactorMethod::YubiKey.label(), "YubiKey");
    }

    #[test]
    fn login_outcome_carries_inner_strings() {
        match LoginOutcome::Success("KEY".into()) {
            LoginOutcome::Success(s) => assert_eq!(s, "KEY"),
            _ => panic!("expected Success"),
        }
        match LoginOutcome::Failed("nope".into()) {
            LoginOutcome::Failed(s) => assert_eq!(s, "nope"),
            _ => panic!("expected Failed"),
        }
        // The two unit-like variants just confirm they construct.
        let _ = LoginOutcome::NeedsDeviceVerification;
        let _ = LoginOutcome::NeedsTwoFactor;
    }

    #[test]
    fn vault_info_can_be_built_with_all_fields() {
        let info = VaultInfo {
            status: VaultStatus::Unlocked,
            user_email: Some("a@b.com".into()),
            last_sync: Some("2024-01-01".into()),
            server_url: Some("https://vault".into()),
        };
        assert_eq!(info.status, VaultStatus::Unlocked);
        assert_eq!(info.user_email.as_deref(), Some("a@b.com"));
    }
}
