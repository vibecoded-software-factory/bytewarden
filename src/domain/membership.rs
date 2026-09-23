use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Organization {
    pub id: String,

    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Collection {
    pub id: String,

    pub name: String,

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
