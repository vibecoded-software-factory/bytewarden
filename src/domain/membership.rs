//! Organization + Collection types (read-only in this iteration).
//!
//! Both come from the user's Bitwarden organisation memberships.
//! Personal-only accounts simply have empty lists.

use serde::Deserialize;

/// A Bitwarden organisation the user is a member of.
#[derive(Debug, Clone, Deserialize)]
pub struct Organization {
    /// Stable Bitwarden organisation identifier (UUID).
    pub id: String,

    /// User-visible organisation name.
    pub name: String,
}

/// A collection inside an organisation. Items can be shared by being
/// assigned to one or more collections.
#[derive(Debug, Clone, Deserialize)]
pub struct Collection {
    /// Stable Bitwarden collection identifier (UUID).
    pub id: String,

    /// Display name.
    pub name: String,

    /// Owning organisation's id. Joined against [`Organization::id`]
    /// to render the popup grouped by org.
    #[serde(rename = "organizationId")]
    pub organization_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_organization_list() {
        let json = r#"[{"id":"o1","name":"Acme"}]"#;
        let orgs: Vec<Organization> = serde_json::from_str(json).expect("parse");
        assert_eq!(orgs.len(), 1);
        assert_eq!(orgs[0].id, "o1");
        assert_eq!(orgs[0].name, "Acme");
    }

    #[test]
    fn deserialize_collection_with_org_id() {
        let json = r#"{"id":"c1","name":"Eng","organizationId":"o1"}"#;
        let c: Collection = serde_json::from_str(json).expect("parse");
        assert_eq!(c.organization_id.as_deref(), Some("o1"));
    }

    #[test]
    fn deserialize_orphan_collection_has_no_org() {
        let json = r#"{"id":"c1","name":"Loose","organizationId":null}"#;
        let c: Collection = serde_json::from_str(json).expect("parse");
        assert!(c.organization_id.is_none());
    }
}
