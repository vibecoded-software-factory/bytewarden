use serde::Deserialize;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const ITEM_TYPE_LOGIN: u8 = 1;

pub const ITEM_TYPE_SECURE_NOTE: u8 = 2;

pub const ITEM_TYPE_CARD: u8 = 3;

pub const ITEM_TYPE_IDENTITY: u8 = 4;

pub const ITEM_TYPE_SSH_KEY: u8 = 5;

#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Item {
    pub id: String,

    pub name: String,

    #[serde(rename = "type")]
    pub item_type: u8,

    pub login: Option<LoginData>,

    pub card: Option<CardData>,

    pub identity: Option<IdentityData>,

    #[serde(rename = "sshKey")]
    pub ssh_key: Option<SshKeyData>,

    pub notes: Option<String>,

    #[serde(rename = "folderId")]
    pub folder_id: Option<String>,

    #[serde(rename = "organizationId", default)]
    pub organization_id: Option<String>,

    #[serde(rename = "collectionIds", default)]
    pub collection_ids: Vec<String>,

    #[serde(default)]
    pub favorite: bool,

    #[serde(default)]
    pub reprompt: u8,

    #[serde(default)]
    pub fields: Vec<Field>,

    pub attachments: Option<Vec<Attachment>>,
}

#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Attachment {
    pub id: String,

    #[serde(rename = "fileName")]
    pub file_name: String,

    #[serde(default)]
    pub size: Option<String>,

    #[serde(rename = "sizeName")]
    pub size_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct LoginData {
    pub username: Option<String>,

    pub password: Option<String>,

    pub uris: Option<Vec<UriData>>,

    pub totp: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct UriData {
    pub uri: Option<String>,

    #[serde(rename = "match")]
    pub match_type: Option<u8>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UriMatch {
    Domain = 0,

    Host = 1,

    StartsWith = 2,

    Exact = 3,

    RegularExpression = 4,

    Never = 5,
}

impl UriMatch {
    pub fn label(self) -> &'static str {
        match self {
            UriMatch::Domain => "Domain",
            UriMatch::Host => "Host",
            UriMatch::StartsWith => "Starts With",
            UriMatch::Exact => "Exact",
            UriMatch::RegularExpression => "Regex",
            UriMatch::Never => "Never",
        }
    }

    pub fn from_u8(n: u8) -> Option<Self> {
        match n {
            0 => Some(UriMatch::Domain),
            1 => Some(UriMatch::Host),
            2 => Some(UriMatch::StartsWith),
            3 => Some(UriMatch::Exact),
            4 => Some(UriMatch::RegularExpression),
            5 => Some(UriMatch::Never),
            _ => None,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Ok(n) = trimmed.parse::<u8>() {
            return Self::from_u8(n);
        }
        let lower = trimmed.to_lowercase();
        match lower.as_str() {
            "domain" => Some(UriMatch::Domain),
            "host" => Some(UriMatch::Host),
            "starts with" | "startswith" | "starts" => Some(UriMatch::StartsWith),
            "exact" => Some(UriMatch::Exact),
            "regex" | "regular expression" | "regularexpression" => {
                Some(UriMatch::RegularExpression)
            }
            "never" => Some(UriMatch::Never),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Field {
    pub name: Option<String>,

    pub value: Option<String>,

    #[serde(rename = "type")]
    pub field_type: u8,
}

#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct CardData {
    #[serde(rename = "cardholderName")]
    pub cardholder_name: Option<String>,

    pub brand: Option<String>,

    pub number: Option<String>,

    #[serde(rename = "expMonth")]
    pub exp_month: Option<String>,

    #[serde(rename = "expYear")]
    pub exp_year: Option<String>,

    pub code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct SshKeyData {
    #[serde(rename = "privateKey")]
    pub private_key: Option<String>,

    #[serde(rename = "publicKey")]
    pub public_key: Option<String>,

    #[serde(rename = "keyFingerprint")]
    pub key_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct IdentityData {
    pub title: Option<String>,
    #[serde(rename = "firstName")]
    pub first_name: Option<String>,
    #[serde(rename = "middleName")]
    pub middle_name: Option<String>,
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub company: Option<String>,

    pub ssn: Option<String>,
    #[serde(rename = "passportNumber")]
    pub passport: Option<String>,
    #[serde(rename = "licenseNumber")]
    pub license: Option<String>,
    pub address1: Option<String>,
    pub address2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    #[serde(rename = "postalCode")]
    pub postal_code: Option<String>,
    pub country: Option<String>,
}

impl Item {
    pub fn needs_reprompt(&self) -> bool {
        self.reprompt != 0
    }
}

pub fn item_type_label(t: u8) -> &'static str {
    match t {
        ITEM_TYPE_LOGIN => "Login",
        ITEM_TYPE_SECURE_NOTE => "Secure Note",
        ITEM_TYPE_CARD => "Card",
        ITEM_TYPE_IDENTITY => "Identity",
        ITEM_TYPE_SSH_KEY => "SSH Key",
        _ => "Other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_match_parses_labels_and_digits() {
        assert_eq!(UriMatch::parse("Domain"), Some(UriMatch::Domain));
        assert_eq!(UriMatch::parse("host"), Some(UriMatch::Host));
        assert_eq!(UriMatch::parse("Starts With"), Some(UriMatch::StartsWith));
        assert_eq!(UriMatch::parse("startswith"), Some(UriMatch::StartsWith));
        assert_eq!(UriMatch::parse("Exact"), Some(UriMatch::Exact));
        assert_eq!(UriMatch::parse("regex"), Some(UriMatch::RegularExpression));
        assert_eq!(
            UriMatch::parse("Regular Expression"),
            Some(UriMatch::RegularExpression)
        );
        assert_eq!(UriMatch::parse("Never"), Some(UriMatch::Never));
        assert_eq!(UriMatch::parse("0"), Some(UriMatch::Domain));
        assert_eq!(UriMatch::parse("5"), Some(UriMatch::Never));
    }

    #[test]
    fn uri_match_rejects_garbage_and_empty() {
        assert_eq!(UriMatch::parse(""), None);
        assert_eq!(UriMatch::parse("   "), None);
        assert_eq!(UriMatch::parse("foo"), None);
        assert_eq!(UriMatch::parse("6"), None);
        assert_eq!(UriMatch::parse("99"), None);
    }

    #[test]
    fn uri_match_from_u8_round_trip() {
        for n in 0..=5u8 {
            let m = UriMatch::from_u8(n).expect("0..=5 should be Some");
            assert_eq!(m as u8, n);
        }
        assert_eq!(UriMatch::from_u8(6), None);
        assert_eq!(UriMatch::from_u8(255), None);
    }

    #[test]
    fn uri_match_label_is_human_readable() {
        assert_eq!(UriMatch::Domain.label(), "Domain");
        assert_eq!(UriMatch::StartsWith.label(), "Starts With");
        assert_eq!(UriMatch::RegularExpression.label(), "Regex");
    }

    #[test]
    fn item_type_label_known_values() {
        assert_eq!(item_type_label(ITEM_TYPE_LOGIN), "Login");
        assert_eq!(item_type_label(ITEM_TYPE_SECURE_NOTE), "Secure Note");
        assert_eq!(item_type_label(ITEM_TYPE_CARD), "Card");
        assert_eq!(item_type_label(ITEM_TYPE_IDENTITY), "Identity");
        assert_eq!(item_type_label(ITEM_TYPE_SSH_KEY), "SSH Key");
    }

    #[test]
    fn item_type_label_unknown_falls_back() {
        assert_eq!(item_type_label(99), "Other");
        assert_eq!(item_type_label(0), "Other");
    }

    #[test]
    fn deserialize_minimal_login_item() {
        let json = r#"{
            "id": "uuid-1",
            "name": "GitHub",
            "type": 1,
            "login": {
                "username": "alice",
                "password": "secret",
                "uris": [{"uri":"https://github.com","match":0}],
                "totp": null
            }
        }"#;
        let item: Item = serde_json::from_str(json).expect("parse");
        assert_eq!(item.id, "uuid-1");
        assert_eq!(item.name, "GitHub");
        assert_eq!(item.item_type, ITEM_TYPE_LOGIN);
        assert!(item.login.is_some());

        assert!(!item.favorite);
        assert!(item.fields.is_empty());
        let login = item.login.as_ref().unwrap();
        let uris = login.uris.as_ref().unwrap();
        assert_eq!(uris[0].uri.as_deref(), Some("https://github.com"));
        assert_eq!(uris[0].match_type, Some(0));
    }

    #[test]
    fn deserialize_card_uses_camelcase_keys() {
        let json = r#"{
            "id": "u",
            "name": "c",
            "type": 3,
            "card": {
                "cardholderName": "JD",
                "brand": "Visa",
                "number": "4242",
                "expMonth": "01",
                "expYear": "2030",
                "code": "123"
            }
        }"#;
        let item: Item = serde_json::from_str(json).expect("parse");
        let card = item.card.as_ref().expect("card payload");
        assert_eq!(card.cardholder_name.as_deref(), Some("JD"));
        assert_eq!(card.exp_month.as_deref(), Some("01"));
        assert_eq!(card.exp_year.as_deref(), Some("2030"));
    }

    #[test]
    fn deserialize_ssh_key_uses_camelcase() {
        let json = r#"{
            "id":"u","name":"k","type":5,
            "sshKey":{"privateKey":"PRIV","publicKey":"PUB","keyFingerprint":"FP"}
        }"#;
        let item: Item = serde_json::from_str(json).expect("parse");
        let ssh = item.ssh_key.as_ref().expect("ssh payload");
        assert_eq!(ssh.private_key.as_deref(), Some("PRIV"));
        assert_eq!(ssh.public_key.as_deref(), Some("PUB"));
        assert_eq!(ssh.key_fingerprint.as_deref(), Some("FP"));
    }

    #[test]
    fn deserialize_favorite_default_false() {
        let json = r#"{"id":"u","name":"n","type":2}"#;
        let item: Item = serde_json::from_str(json).expect("parse");
        assert!(!item.favorite);
    }

    #[test]
    fn every_domain_payload_implements_zeroize() {
        fn assert_zeroize<T: zeroize::Zeroize>() {}
        assert_zeroize::<Item>();
        assert_zeroize::<Attachment>();
        assert_zeroize::<LoginData>();
        assert_zeroize::<UriData>();
        assert_zeroize::<Field>();
        assert_zeroize::<CardData>();
        assert_zeroize::<SshKeyData>();
        assert_zeroize::<IdentityData>();
    }

    #[test]
    fn zeroize_clears_login_data_strings() {
        use zeroize::Zeroize;
        let mut login = LoginData {
            username: Some("alice@example.com".into()),
            password: Some("hunter2-supersecret".into()),
            uris: Some(vec![UriData {
                uri: Some("https://example.com".into()),
                match_type: Some(0),
            }]),
            totp: Some("OTPAUTHSECRETSEED".into()),
        };
        login.zeroize();

        assert!(login.username.is_none());
        assert!(login.password.is_none());
        assert!(login.totp.is_none());
        assert!(login.uris.is_none());
    }

    #[test]
    fn deserialize_reprompt_flag_round_trip() {
        let json = r#"{"id":"u","name":"n","type":1,"reprompt":1}"#;
        let item: Item = serde_json::from_str(json).expect("parse");
        assert_eq!(item.reprompt, 1);
        assert!(item.needs_reprompt());
    }

    #[test]
    fn deserialize_without_reprompt_defaults_to_zero() {
        let json = r#"{"id":"u","name":"n","type":1}"#;
        let item: Item = serde_json::from_str(json).expect("parse");
        assert_eq!(item.reprompt, 0);
        assert!(!item.needs_reprompt());
    }

    #[test]
    fn needs_reprompt_treats_any_nonzero_value_as_protected() {
        let mut item: Item = serde_json::from_str(r#"{"id":"u","name":"n","type":1}"#).unwrap();
        item.reprompt = 2;
        assert!(item.needs_reprompt());
    }

    #[test]
    fn deserialize_with_collection_ids_and_organization() {
        let json = r#"{
            "id":"u","name":"Shared","type":1,
            "organizationId":"org-1",
            "collectionIds":["c1","c2"]
        }"#;
        let item: Item = serde_json::from_str(json).expect("parse");
        assert_eq!(item.organization_id.as_deref(), Some("org-1"));
        assert_eq!(
            item.collection_ids,
            vec!["c1".to_string(), "c2".to_string()]
        );
    }

    #[test]
    fn deserialize_personal_item_has_empty_collection_ids() {
        let json = r#"{"id":"u","name":"Personal","type":1}"#;
        let item: Item = serde_json::from_str(json).expect("parse");
        assert!(item.organization_id.is_none());
        assert!(item.collection_ids.is_empty());
    }

    #[test]
    fn deserialize_with_attachments() {
        let json = r#"{
            "id":"u","name":"n","type":1,
            "attachments":[{"id":"a1","fileName":"f.pdf","sizeName":"45 KB"}]
        }"#;
        let item: Item = serde_json::from_str(json).expect("parse");
        let atts = item.attachments.as_ref().expect("attachments");
        assert_eq!(atts.len(), 1);
        assert_eq!(atts[0].id, "a1");
        assert_eq!(atts[0].file_name, "f.pdf");
        assert_eq!(atts[0].size_name.as_deref(), Some("45 KB"));
    }
}
