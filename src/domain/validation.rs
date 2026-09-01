pub fn validate_email(input: &str) -> Result<(), &'static str> {
    let s = input.trim();
    if s.is_empty() {
        return Err("Email cannot be empty.");
    }
    let Some((local, domain)) = s.split_once('@') else {
        return Err("Email is missing '@'.");
    };
    if local.is_empty() {
        return Err("Email is missing the local part before '@'.");
    }
    if domain.is_empty() {
        return Err("Email is missing the domain after '@'.");
    }
    if !domain.contains('.') {
        return Err("Email domain is missing a dot (e.g. example.com).");
    }
    Ok(())
}

pub fn validate_server_url(input: &str) -> Result<(), &'static str> {
    let s = input.trim();
    if s.is_empty() {
        return Err("Server URL cannot be empty.");
    }
    let host = if let Some(rest) = s.strip_prefix("https://") {
        rest
    } else if let Some(rest) = s.strip_prefix("http://") {
        rest
    } else {
        return Err("Server URL must start with http:// or https://.");
    };
    if host.is_empty() {
        return Err("Server URL is missing a host after the scheme.");
    }
    Ok(())
}

pub fn check_name_unique(
    name: &str,
    existing: impl IntoIterator<Item = impl AsRef<str>>,
    current: Option<&str>,
) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Name cannot be empty.".into());
    }
    let current_lower = current.map(|s| s.trim().to_lowercase());
    for other in existing {
        let other = other.as_ref();
        if other.eq_ignore_ascii_case(trimmed)
            && current_lower.as_deref() != Some(other.to_lowercase().as_str())
        {
            return Err(format!("\"{trimmed}\" is already used."));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_accepts_well_formed_addresses() {
        assert!(validate_email("alice@example.com").is_ok());
        assert!(validate_email("a.b+tag@sub.example.co.uk").is_ok());

        assert!(validate_email("  alice@example.com  ").is_ok());
    }

    #[test]
    fn email_rejects_empty_or_missing_pieces() {
        assert!(validate_email("").is_err());
        assert!(validate_email("   ").is_err());
        assert!(validate_email("alice").is_err());
        assert!(validate_email("@example.com").is_err());
        assert!(validate_email("alice@").is_err());
        assert!(validate_email("alice@localhost").is_err());
    }

    #[test]
    fn email_error_messages_are_specific() {
        assert_eq!(
            validate_email("alice").unwrap_err(),
            "Email is missing '@'."
        );
        assert_eq!(
            validate_email("@example.com").unwrap_err(),
            "Email is missing the local part before '@'."
        );
        assert_eq!(
            validate_email("alice@").unwrap_err(),
            "Email is missing the domain after '@'.",
        );
        assert_eq!(
            validate_email("alice@localhost").unwrap_err(),
            "Email domain is missing a dot (e.g. example.com).",
        );
    }

    #[test]
    fn server_url_accepts_http_and_https() {
        assert!(validate_server_url("http://localhost:8000").is_ok());
        assert!(validate_server_url("https://vault.bitwarden.com").is_ok());
        assert!(validate_server_url("https://my-vault.example.com").is_ok());

        assert!(validate_server_url("  https://x.y  ").is_ok());
    }

    #[test]
    fn server_url_rejects_missing_scheme_or_host() {
        assert!(validate_server_url("").is_err());
        assert!(validate_server_url("vault.bitwarden.com").is_err());
        assert!(validate_server_url("ftp://example.com").is_err());
        assert!(validate_server_url("https://").is_err());
        assert!(validate_server_url("http://").is_err());
    }

    #[test]
    fn name_unique_rejects_collisions_case_insensitive() {
        let existing = ["Work", "Personal"];
        assert!(check_name_unique("Tools", existing, None).is_ok());
        assert!(check_name_unique("WORK", existing, None).is_err());
        assert!(check_name_unique("personal", existing, None).is_err());
    }

    #[test]
    fn name_unique_allows_renaming_to_same_value() {
        let existing = ["Work", "Personal"];
        assert!(check_name_unique("Work", existing, Some("Work")).is_ok());

        assert!(check_name_unique("Personal", existing, Some("Work")).is_err());
    }

    #[test]
    fn name_unique_rejects_empty() {
        let existing: [&str; 0] = [];
        assert!(check_name_unique("", existing, None).is_err());
        assert!(check_name_unique("   ", existing, None).is_err());
    }
}
