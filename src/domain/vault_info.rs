#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwoFactorMethod {
    Authenticator,

    Email,

    YubiKey,
}

impl TwoFactorMethod {
    pub fn label(self) -> &'static str {
        match self {
            TwoFactorMethod::Authenticator => "Authenticator",
            TwoFactorMethod::Email => "Email",
            TwoFactorMethod::YubiKey => "YubiKey",
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            TwoFactorMethod::Authenticator => 0,
            TwoFactorMethod::Email => 1,
            TwoFactorMethod::YubiKey => 3,
        }
    }

    pub fn next(self) -> Self {
        match self {
            TwoFactorMethod::Authenticator => TwoFactorMethod::Email,
            TwoFactorMethod::Email => TwoFactorMethod::YubiKey,
            TwoFactorMethod::YubiKey => TwoFactorMethod::Authenticator,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            TwoFactorMethod::Authenticator => TwoFactorMethod::YubiKey,
            TwoFactorMethod::Email => TwoFactorMethod::Authenticator,
            TwoFactorMethod::YubiKey => TwoFactorMethod::Email,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultStatus {
    Unlocked,

    Locked,

    Unauthenticated,
}

#[derive(Debug, Clone)]
pub struct VaultInfo {
    pub status: VaultStatus,

    pub user_email: Option<String>,

    pub last_sync: Option<String>,

    pub server_url: Option<String>,
}

#[derive(Debug)]
pub enum LoginOutcome {
    Success(String),

    NeedsDeviceVerification,

    NeedsTwoFactor,

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
