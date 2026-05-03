//! Vault status and authentication outcome types.

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

    /// ISO timestamp of the last successful sync.
    #[allow(dead_code)]
    pub last_sync: Option<String>,

    /// Bitwarden server URL — non-default for self-hosted instances.
    #[allow(dead_code)]
    pub server_url: Option<String>,
}

/// Outcome of [`crate::ports::VaultPort::login`].
///
/// `login` may succeed, fail, or trigger a "new device" challenge that
/// requires a one-time code — the latter is reported via [`Self::NeedsOtp`]
/// and resolved by calling
/// [`crate::ports::VaultPort::login_with_otp`].
#[derive(Debug)]
pub enum LoginOutcome {
    /// Success — the inner string is the session key.
    Success(String),
    /// The backend detected a new device and sent an OTP to the user e-mail.
    NeedsOtp,
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
    fn login_outcome_carries_inner_strings() {
        match LoginOutcome::Success("KEY".into()) {
            LoginOutcome::Success(s) => assert_eq!(s, "KEY"),
            _ => panic!("expected Success"),
        }
        match LoginOutcome::Failed("nope".into()) {
            LoginOutcome::Failed(s) => assert_eq!(s, "nope"),
            _ => panic!("expected Failed"),
        }
        // NeedsOtp is unit-like — just confirm it constructs.
        let _ = LoginOutcome::NeedsOtp;
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
