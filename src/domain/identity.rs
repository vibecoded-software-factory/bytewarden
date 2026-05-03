//! Helpers for [`crate::domain::item::IdentityData`].

use crate::domain::item::IdentityData;

/// Joins the four name parts with single spaces, skipping empties.
///
/// # Examples
///
/// ```
/// use bytewarden::domain::identity::build_full_name;
/// let s = build_full_name(Some("Mr"), Some("John"), None, Some("Doe"));
/// assert_eq!(s, "Mr John Doe");
/// ```
pub fn build_full_name(
    title: Option<&str>,
    first: Option<&str>,
    middle: Option<&str>,
    last: Option<&str>,
) -> String {
    [title, first, middle, last]
        .iter()
        .filter_map(|s| s.filter(|x| !x.is_empty()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Returns the (label, value) tuples for the secondary identity fields.
///
/// The order here drives the order they appear in the detail view and the
/// edit form. Name parts are intentionally absent — they are rendered as a
/// single composed "Full Name" line via [`build_full_name`].
pub fn identity_fields(id: &IdentityData) -> Vec<(&'static str, &Option<String>)> {
    vec![
        ("Email", &id.email),
        ("Phone", &id.phone),
        ("Company", &id.company),
        ("Address", &id.address1),
        ("Address 2", &id.address2),
        ("City", &id.city),
        ("State", &id.state),
        ("ZIP", &id.postal_code),
        ("Country", &id.country),
        ("SSN", &id.ssn),
        ("Passport", &id.passport),
        ("License", &id.license),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_name_skips_none_and_empty() {
        assert_eq!(
            build_full_name(Some("Mr"), Some("John"), None, Some("Doe")),
            "Mr John Doe"
        );
        assert_eq!(
            build_full_name(None, Some("Jane"), Some(""), Some("Doe")),
            "Jane Doe"
        );
        assert_eq!(build_full_name(None, None, None, None), "");
    }

    #[test]
    fn full_name_with_only_first_name() {
        assert_eq!(build_full_name(None, Some("Cher"), None, None), "Cher");
    }

    fn empty_identity() -> IdentityData {
        IdentityData {
            title: None,
            first_name: None,
            middle_name: None,
            last_name: None,
            email: None,
            phone: None,
            company: None,
            ssn: None,
            passport: None,
            license: None,
            address1: None,
            address2: None,
            city: None,
            state: None,
            postal_code: None,
            country: None,
        }
    }

    #[test]
    fn identity_fields_order_is_stable() {
        let id = empty_identity();
        let labels: Vec<&'static str> = identity_fields(&id).into_iter().map(|(l, _)| l).collect();
        assert_eq!(
            labels,
            vec![
                "Email",
                "Phone",
                "Company",
                "Address",
                "Address 2",
                "City",
                "State",
                "ZIP",
                "Country",
                "SSN",
                "Passport",
                "License",
            ]
        );
    }

    #[test]
    fn identity_fields_omits_name_parts() {
        let mut id = empty_identity();
        id.first_name = Some("X".into());
        id.last_name = Some("Y".into());
        let labels: Vec<&'static str> = identity_fields(&id).into_iter().map(|(l, _)| l).collect();
        assert!(!labels.contains(&"First Name"));
        assert!(!labels.contains(&"Last Name"));
    }
}
