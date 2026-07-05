//! Vault item types.
//!
//! These types mirror the JSON schema returned by `bw list items`, but live
//! in the domain layer so any future adapter (REST API, mock, etc.) must
//! produce the same shape. The `serde` derives are pragmatic: they avoid an
//! extra DTO/conversion layer at the cost of a tiny dependency bleed.
//!
//! ## In-memory hygiene
//!
//! Every struct in this module derives [`Zeroize`] and
//! [`ZeroizeOnDrop`]. When an `Item` (or any nested payload —
//! `LoginData`, `CardData`, `SshKeyData`, `IdentityData`, `Field`,
//! `UriData`, `Attachment`) is dropped, every byte of every owned
//! `String` is overwritten with zeroes by the compiler-generated
//! `Drop` impl. That includes:
//!
//! * the original items inside `App::items` / `App::trashed_items`
//!   when the vault is locked or the user logs out,
//! * every `Clone` of an item the flows pass around (favourite
//!   toggle, edit-mode entry, copy-to-clipboard staging…),
//! * temporary items materialised while parsing JSON or driving
//!   `bw edit item` / `bw create item`.
//!
//! It does **not** cover non-domain copies — a `String` that the
//! adapter pulls out of `bw`'s stdout, holds in `get_item_json`, and
//! returns to the caller is wrapped separately in [`zeroize::Zeroizing`]
//! at that boundary.

use serde::Deserialize;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Numeric type identifier for a [`LoginData`] item.
pub const ITEM_TYPE_LOGIN: u8 = 1;
/// Numeric type identifier for a Secure Note item.
pub const ITEM_TYPE_SECURE_NOTE: u8 = 2;
/// Numeric type identifier for a [`CardData`] item.
pub const ITEM_TYPE_CARD: u8 = 3;
/// Numeric type identifier for an [`IdentityData`] item.
pub const ITEM_TYPE_IDENTITY: u8 = 4;
/// Numeric type identifier for an SSH-key item.
pub const ITEM_TYPE_SSH_KEY: u8 = 5;

/// A single vault entry.
///
/// `item_type` follows the Bitwarden numeric enum (see the `ITEM_TYPE_*`
/// constants). Only the variant matching `item_type` will have its associated
/// payload populated; the others are `None`.
#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Item {
    /// Stable Bitwarden item identifier (UUID).
    pub id: String,

    /// User-visible name shown in the vault list.
    pub name: String,

    /// Numeric type discriminant — see the `ITEM_TYPE_*` constants.
    #[serde(rename = "type")]
    pub item_type: u8,

    /// Payload for `ITEM_TYPE_LOGIN` items.
    pub login: Option<LoginData>,

    /// Payload for `ITEM_TYPE_CARD` items.
    pub card: Option<CardData>,

    /// Payload for `ITEM_TYPE_IDENTITY` items.
    pub identity: Option<IdentityData>,

    /// Payload for `ITEM_TYPE_SSH_KEY` items.
    #[serde(rename = "sshKey")]
    pub ssh_key: Option<SshKeyData>,

    /// Free-form notes attached to any item.
    pub notes: Option<String>,

    /// Folder identifier. Drives the Folders-sidebar filter and the
    /// "(No folder)" bucket.
    #[serde(rename = "folderId")]
    pub folder_id: Option<String>,

    /// Bitwarden organisation that owns this item, when shared. `None`
    /// for personal-vault items. Read-only from the TUI's perspective:
    /// changing org membership requires a `bw move` follow-up that
    /// bytewarden does not yet drive.
    #[serde(rename = "organizationId", default)]
    pub organization_id: Option<String>,

    /// Collections inside the owning organisation that this item is
    /// shared into. Empty for personal items. Used by the sidebar
    /// filter to surface "Org / Collection" rows; assignment from the
    /// TUI is a follow-up — for now bytewarden round-trips whatever
    /// the official client set.
    #[serde(rename = "collectionIds", default)]
    pub collection_ids: Vec<String>,

    /// Whether the item is starred.
    #[serde(default)]
    pub favorite: bool,

    /// Reprompt flag from the Bitwarden schema. `0` (the default) means
    /// "no extra check"; any non-zero value (currently always `1`,
    /// "Password") means the client is expected to re-prompt the user
    /// for the master password before *exposing* the item's secrets —
    /// copying the password / TOTP / a hidden custom field, or
    /// revealing them on screen with F2.
    ///
    /// The check is enforced client-side: bw itself does not gate the
    /// data behind this flag, it just round-trips the value. See
    /// [`Self::needs_reprompt`] and the popup wired into the copy /
    /// reveal paths in `tui::flows::copy` and `tui::input::detail`.
    #[serde(default)]
    pub reprompt: u8,

    /// User-defined custom fields.
    #[serde(default)]
    pub fields: Vec<Field>,

    /// File attachments uploaded with this item. Always `None` for
    /// items that have never had an attachment (the bw JSON omits
    /// the key entirely in that case).
    pub attachments: Option<Vec<Attachment>>,
}

/// Single file attachment on an [`Item`].
///
/// Bytewarden can list and upload attachments today. Download is
/// supported via `bw get attachment` and delete via `bw delete
/// attachment` — those are TUI follow-ups.
#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Attachment {
    /// Stable Bitwarden attachment identifier.
    pub id: String,

    /// Display name of the file.
    #[serde(rename = "fileName")]
    pub file_name: String,

    /// Size in bytes (raw integer, useful for downloads / progress).
    /// Bw also returns a human-readable `sizeName` which we ignore.
    #[serde(default)]
    pub size: Option<String>,

    /// Pre-rendered size string (e.g. `"45 KB"`) — easier for the UI
    /// than reformatting `size` ourselves.
    #[serde(rename = "sizeName")]
    pub size_name: Option<String>,
}

/// Login-specific payload (username, password, URLs, TOTP seed).
#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct LoginData {
    /// Account username — usually an e-mail address or handle.
    pub username: Option<String>,

    /// Master password for this account.
    pub password: Option<String>,

    /// Zero or more URIs the credentials apply to.
    pub uris: Option<Vec<UriData>>,

    /// TOTP seed (base32) used to generate one-time codes.
    pub totp: Option<String>,
}

/// A single URI inside a [`LoginData`] entry.
#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct UriData {
    /// Absolute URL (or pattern) the credentials apply to.
    pub uri: Option<String>,

    /// URI match-detection mode. Controls how the Bitwarden clients
    /// (browser extension, mobile, autofill) decide whether the
    /// credentials apply to a candidate URL. `None` = use the user's
    /// account-wide default (Domain).
    ///
    /// See [`UriMatch`] for the enum.
    #[serde(rename = "match")]
    pub match_type: Option<u8>,
}

/// URI match-detection types accepted by the Bitwarden CLI's `match`
/// field.
///
/// The numeric values are the discriminants `bw` reads/writes — the
/// enum is just a thin labelled wrapper for the UI layer.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UriMatch {
    /// Match the registered domain (e.g. `example.com` matches
    /// `mail.example.com`). Default if the field is omitted.
    Domain = 0,
    /// Match the exact host (subdomain + port).
    Host = 1,
    /// Saved URI is a prefix of the candidate URI.
    StartsWith = 2,
    /// Strict equality.
    Exact = 3,
    /// Saved URI is a regular expression matched against the candidate.
    RegularExpression = 4,
    /// Never match — autofill is disabled for this URI.
    Never = 5,
}

impl UriMatch {
    /// Returns the human-readable label.
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

    /// Resolves a numeric discriminant back to the enum, or `None` for
    /// out-of-range values.
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

    /// Parses a free-form user input string into a [`UriMatch`].
    ///
    /// Accepts case-insensitive labels (`"Domain"`, `"host"`,
    /// `"Starts With"`, `"Exact"`, `"Regex"`, `"Regular Expression"`,
    /// `"Never"`) and the bare digits `"0"` through `"5"`. Empty or
    /// unrecognised input returns `None`, which the caller should
    /// interpret as "use the bw default" (i.e. omit the field).
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

/// User-defined custom field on any item.
///
/// `field_type` mirrors the Bitwarden enum:
/// * 0 — plain text,
/// * 1 — hidden (rendered masked),
/// * 2 — boolean,
/// * 3 — linked.
#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Field {
    /// Display label for the field.
    pub name: Option<String>,

    /// Stored value — may be empty for boolean fields.
    pub value: Option<String>,

    /// Numeric discriminant — see the docstring of [`Field`].
    #[serde(rename = "type")]
    pub field_type: u8,
}

/// Card-specific payload (cardholder, brand, number, expiry, CVV).
#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct CardData {
    /// Cardholder full name.
    #[serde(rename = "cardholderName")]
    pub cardholder_name: Option<String>,

    /// Card network (`Visa`, `Mastercard`, …).
    pub brand: Option<String>,

    /// PAN (Primary Account Number).
    pub number: Option<String>,

    /// Two-digit expiration month (`01`–`12`).
    #[serde(rename = "expMonth")]
    pub exp_month: Option<String>,

    /// Four-digit expiration year.
    #[serde(rename = "expYear")]
    pub exp_year: Option<String>,

    /// CVV / CVC security code.
    pub code: Option<String>,
}

/// SSH-key payload (private key, public key, key fingerprint).
///
/// `key_fingerprint` is computed by `bw` from `private_key` whenever
/// the item is created or edited, so the field is read-only from the
/// caller's perspective.
#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct SshKeyData {
    /// PEM-encoded private key (OpenSSH or PKCS#8 — `bw` accepts both).
    #[serde(rename = "privateKey")]
    pub private_key: Option<String>,

    /// `ssh-rsa AAAA…`-style public key derived from `private_key`.
    #[serde(rename = "publicKey")]
    pub public_key: Option<String>,

    /// SHA-256 fingerprint of the public key, computed by `bw`.
    #[serde(rename = "keyFingerprint")]
    pub key_fingerprint: Option<String>,
}

/// Identity-specific payload (name, address, phone, …).
#[derive(Debug, Clone, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct IdentityData {
    /// Honorific (`Mr`, `Ms`, …).
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
    /// Social Security Number / national identifier.
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
    /// `true` when the item carries the Bitwarden reprompt flag and
    /// the client is expected to re-verify the master password before
    /// exposing its secrets.
    ///
    /// The current Bitwarden schema only defines value `1` (Password
    /// reprompt), but we treat any non-zero value the same way so
    /// future variants don't silently downgrade to "no protection".
    pub fn needs_reprompt(&self) -> bool {
        self.reprompt != 0
    }
}

/// Returns the human-readable label for an `item_type` discriminant.
///
/// Unknown values resolve to `"Other"`.
///
/// # Examples
///
/// ```
/// use bytewarden::domain::item::item_type_label;
/// assert_eq!(item_type_label(1), "Login");
/// assert_eq!(item_type_label(99), "Other");
/// ```
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
        // Mirrors a stripped-down `bw list items` row.
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
        // Defaults are honoured even though the JSON omits the keys.
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
        // Item without "favorite" key — should default to false.
        let json = r#"{"id":"u","name":"n","type":2}"#;
        let item: Item = serde_json::from_str(json).expect("parse");
        assert!(!item.favorite);
    }

    /// Compile-time guard: every domain struct that holds a `String`
    /// derives `Zeroize`. If a future refactor drops the derive on any
    /// of them, this fails to compile and signals that the in-memory
    /// hygiene contract regressed.
    ///
    /// We require the trait via a generic helper so the assertion is
    /// purely structural — the bodies never execute.
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

    /// Verifies that the auto-generated `zeroize()` impl actually
    /// scrubs the data. Per the `zeroize` crate contract:
    ///
    /// * `Option<Z: Zeroize>::zeroize` first zeroizes the inner value
    ///   (overwriting the bytes in place) and then sets the
    ///   discriminant to `None`, so an attacker grepping the heap
    ///   sees neither the payload nor the "Some" tag.
    /// * `Vec<T: Zeroize>::zeroize` zeroizes every element and clears
    ///   the length to 0.
    ///
    /// We assert the post-conditions both report `None` / empty —
    /// that's the closest "no plaintext anywhere" check we can do
    /// without dumping memory.
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
        // `Option<String>::zeroize` overwrites the inner buffer and
        // then collapses the option to `None`.
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
        // Items from the official client omit `reprompt` when it's
        // not set; serde's `#[serde(default)]` should give us 0.
        let json = r#"{"id":"u","name":"n","type":1}"#;
        let item: Item = serde_json::from_str(json).expect("parse");
        assert_eq!(item.reprompt, 0);
        assert!(!item.needs_reprompt());
    }

    #[test]
    fn needs_reprompt_treats_any_nonzero_value_as_protected() {
        // The schema only defines value 1 today, but a hypothetical
        // future value (2 = WebAuthn step-up, say) must keep the
        // protection on rather than silently downgrade.
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
        // Personal-vault items omit `organizationId` and
        // `collectionIds`. The `#[serde(default)]` attribute should
        // give us `None` and `vec![]`.
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
