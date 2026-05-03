//! JSON-payload builders used by the create / edit flows.
//!
//! Building the payload via [`serde_json::Value`] is safer than
//! `format!`-stringly-typed assembly: the library handles all escaping
//! correctly, so user input cannot break the document or be used to
//! inject extra fields.

use serde_json::{Value, json};

use crate::domain::UriMatch;
use crate::domain::filter::CreateItemType;
use crate::domain::item::{
    ITEM_TYPE_CARD, ITEM_TYPE_IDENTITY, ITEM_TYPE_LOGIN, ITEM_TYPE_SECURE_NOTE, ITEM_TYPE_SSH_KEY,
};
use crate::tui::edit_field::{EditField, EditFieldKind, UriRole};

/// Returns the value of the field whose `label` matches `label`, or an
/// empty string if no such field exists.
fn get<'a>(fields: &'a [EditField], label: &str) -> &'a str {
    fields
        .iter()
        .find(|f| f.label == label)
        .map(|f| f.value.as_str())
        .unwrap_or("")
}

/// Resolves an "URL Match" form-field value to the JSON value to write
/// under `uris[0].match`. `Value::Null` means "use the account-wide
/// default", which matches what bw does for an absent / null field.
fn match_json(s: &str) -> Value {
    UriMatch::parse(s)
        .map(|m| json!(m as u8))
        .unwrap_or(Value::Null)
}

/// Reconstructs the `uris` JSON array from the [`EditField`] rows
/// tagged with [`EditFieldKind::Uri`].
///
/// Rows are grouped by their slot `index`; each group emits one
/// `{ uri, match }` object. Slots are sorted numerically so the
/// resulting array preserves the form's visual order even if the user
/// added rows out-of-order.
///
/// Returns an empty `Vec` when the form has no URI rows — the caller
/// should treat that as "do not touch `uris`".
fn build_uris_array(fields: &[EditField]) -> Vec<Value> {
    use std::collections::BTreeMap;

    let mut by_slot: BTreeMap<usize, (Option<&str>, Option<&str>)> = BTreeMap::new();
    for f in fields {
        if let EditFieldKind::Uri { index, role } = f.kind {
            let slot = by_slot.entry(index).or_insert((None, None));
            match role {
                UriRole::Url => slot.0 = Some(f.value.as_str()),
                UriRole::Match => slot.1 = Some(f.value.as_str()),
            }
        }
    }
    by_slot
        .into_values()
        .map(|(url, m)| {
            json!({
                "uri":   url.unwrap_or(""),
                "match": match_json(m.unwrap_or("")),
            })
        })
        .collect()
}

/// Builds the JSON payload for a "create item" call given the form
/// values.
pub fn build_create_payload(item_type: &CreateItemType, fields: &[EditField]) -> String {
    let v: Value = match item_type {
        CreateItemType::Login => json!({
            "type": ITEM_TYPE_LOGIN,
            "name":  get(fields, "Name"),
            "notes": get(fields, "Notes"),
            "login": {
                "username": get(fields, "Username"),
                "password": get(fields, "Password"),
                "uris": [{
                    "uri":   get(fields, "URL"),
                    "match": match_json(get(fields, "URL Match")),
                }],
            },
        }),
        CreateItemType::SecureNote => json!({
            "type": ITEM_TYPE_SECURE_NOTE,
            "name":  get(fields, "Name"),
            "notes": get(fields, "Notes"),
            "secureNote": { "type": 0 },
        }),
        CreateItemType::Card => json!({
            "type": ITEM_TYPE_CARD,
            "name":  get(fields, "Name"),
            "notes": get(fields, "Notes"),
            "card": {
                "cardholderName": get(fields, "Cardholder"),
                "brand":    get(fields, "Brand"),
                "number":   get(fields, "Number"),
                "expMonth": get(fields, "Exp Month"),
                "expYear":  get(fields, "Exp Year"),
                "code":     get(fields, "CVV"),
            },
        }),
        CreateItemType::Identity => json!({
            "type": ITEM_TYPE_IDENTITY,
            "name":  get(fields, "Name"),
            "notes": get(fields, "Notes"),
            "identity": {
                "firstName":  get(fields, "First Name"),
                "lastName":   get(fields, "Last Name"),
                "email":      get(fields, "Email"),
                "phone":      get(fields, "Phone"),
                "company":    get(fields, "Company"),
                "address1":   get(fields, "Address"),
                "city":       get(fields, "City"),
                "state":      get(fields, "State"),
                "postalCode": get(fields, "ZIP"),
                "country":    get(fields, "Country"),
            },
        }),
        // `keyFingerprint` is intentionally absent — `bw` derives it
        // from `privateKey` server-side and overwriting it would just
        // be ignored.
        CreateItemType::SshKey => json!({
            "type": ITEM_TYPE_SSH_KEY,
            "name":  get(fields, "Name"),
            "notes": get(fields, "Notes"),
            "sshKey": {
                "privateKey": get(fields, "Private Key"),
                "publicKey":  get(fields, "Public Key"),
            },
        }),
    };
    v.to_string()
}

/// Patches an existing item's JSON in place, copying values from the
/// edit form. Unknown / unset fields are left untouched, so adapter-only
/// keys (like `"organizationId"`) survive a round-trip.
pub fn patch_edit_payload(base_json: &str, fields: &[EditField]) -> String {
    let Ok(mut val) = serde_json::from_str::<Value>(base_json) else {
        return base_json.to_string();
    };
    let lookup = |label: &str| {
        fields
            .iter()
            .find(|f| f.label == label)
            .map(|f| f.value.as_str())
    };

    if let Some(v) = lookup("Name") {
        val["name"] = json!(v);
    }
    if let Some(v) = lookup("Notes") {
        val["notes"] = json!(v);
    }
    // The "Folder" field already carries the resolved folder id (the
    // flow translates the user-typed name → id before calling us).
    // Empty value means "no folder" → write null.
    if let Some(v) = lookup("Folder") {
        val["folderId"] = if v.is_empty() { Value::Null } else { json!(v) };
    }

    if val["type"] == ITEM_TYPE_LOGIN {
        if let Some(v) = lookup("Username") {
            val["login"]["username"] = json!(v);
        }
        if let Some(v) = lookup("Password") {
            val["login"]["password"] = json!(v);
        }
        // Rebuild the entire `uris` array from the EditField rows
        // tagged with `EditFieldKind::Uri`. Doing it as a full
        // replace (rather than per-row patching) means add/remove
        // multi-URI flows round-trip correctly via a single branch.
        let new_uris = build_uris_array(fields);
        // Only replace when at least one URL row exists, so login
        // items that the form never showed URIs for (edge case) don't
        // get their `uris` blanked.
        if !new_uris.is_empty() {
            val["login"]["uris"] = Value::Array(new_uris);
        }
        if let Some(v) = lookup("TOTP seed") {
            val["login"]["totp"] = json!(v);
        }
    }

    if val["type"] == ITEM_TYPE_CARD {
        if let Some(v) = lookup("Cardholder") {
            val["card"]["cardholderName"] = json!(v);
        }
        if let Some(v) = lookup("Brand") {
            val["card"]["brand"] = json!(v);
        }
        if let Some(v) = lookup("Number") {
            val["card"]["number"] = json!(v);
        }
        if let Some(v) = lookup("Exp Month") {
            val["card"]["expMonth"] = json!(v);
        }
        if let Some(v) = lookup("Exp Year") {
            val["card"]["expYear"] = json!(v);
        }
        if let Some(v) = lookup("CVV") {
            val["card"]["code"] = json!(v);
        }
    }

    if val["type"] == ITEM_TYPE_IDENTITY {
        for (key, label) in [
            ("firstName", "First Name"),
            ("lastName", "Last Name"),
            ("email", "Email"),
            ("phone", "Phone"),
            ("company", "Company"),
            ("address1", "Address"),
            ("city", "City"),
            ("state", "State"),
            ("postalCode", "ZIP"),
            ("country", "Country"),
            ("ssn", "SSN"),
            ("passportNumber", "Passport"),
            ("licenseNumber", "License"),
        ] {
            if let Some(v) = lookup(label) {
                val["identity"][key] = json!(v);
            }
        }
    }

    if val["type"] == ITEM_TYPE_SSH_KEY {
        if let Some(v) = lookup("Private Key") {
            val["sshKey"]["privateKey"] = json!(v);
        }
        if let Some(v) = lookup("Public Key") {
            val["sshKey"]["publicKey"] = json!(v);
        }
        // `Fingerprint` is read-only — `bw` recomputes it from the
        // (possibly updated) private key, so we never write it here.
    }

    // Custom fields — rebuild the array from the EditField rows
    // tagged as Custom, **but** preserve any `linked` (type 3) entries
    // verbatim from the base JSON. The TUI cannot edit linked fields
    // (no UI to pick the target), so re-emitting them with
    // `linkedId: null` from the form would silently drop the
    // reference on every save. Linked fields go first so their
    // ordering is stable across saves.
    let preserved_linked: Vec<Value> = val["fields"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|f| f.get("type").and_then(|t| t.as_u64()) == Some(3))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let editable_custom: Vec<Value> = fields
        .iter()
        .filter(|f| f.is_custom() && f.custom_type() != Some(3))
        .map(|f| {
            json!({
                "name":     f.label,
                "value":    f.value,
                "type":     f.custom_type().unwrap_or(0),
                "linkedId": Value::Null,
            })
        })
        .collect();
    let mut all_custom = preserved_linked;
    all_custom.extend(editable_custom);
    val["fields"] = Value::Array(all_custom);

    serde_json::to_string(&val).unwrap_or_else(|_| base_json.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::edit_field::EditField;

    fn ef(label: &str, value: &str) -> EditField {
        EditField::new(label, value, false)
    }

    #[test]
    fn create_login_payload_escapes_quotes() {
        let fields = vec![
            ef("Name", "my \"site\""),
            ef("Username", "user"),
            ef("Password", "p\"a\"s"),
            ef("URL", "https://x"),
            ef("Notes", ""),
        ];
        let json = build_create_payload(&CreateItemType::Login, &fields);
        // Round-trip: the resulting string must parse back into a Value.
        let parsed: Value = serde_json::from_str(&json).expect("must parse");
        assert_eq!(parsed["name"], "my \"site\"");
        assert_eq!(parsed["login"]["password"], "p\"a\"s");
    }

    #[test]
    fn patch_preserves_unknown_keys() {
        let base =
            r#"{"type":1,"name":"old","customKey":42,"login":{"username":"u","password":"p"}}"#;
        let fields = vec![ef("Name", "new")];
        let patched = patch_edit_payload(base, &fields);
        let parsed: Value = serde_json::from_str(&patched).unwrap();
        assert_eq!(parsed["name"], "new");
        assert_eq!(parsed["customKey"], 42);
    }

    #[test]
    fn create_ssh_payload_omits_fingerprint() {
        let fields = vec![
            ef("Name", "deploy key"),
            ef("Private Key", "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n"),
            ef("Public Key", "ssh-ed25519 AAAA…"),
            ef("Notes", ""),
        ];
        let json = build_create_payload(&CreateItemType::SshKey, &fields);
        let parsed: Value = serde_json::from_str(&json).expect("must parse");
        assert_eq!(parsed["type"], 5);
        assert_eq!(parsed["sshKey"]["privateKey"], fields[1].value);
        assert_eq!(parsed["sshKey"]["publicKey"], fields[2].value);
        // bw computes the fingerprint server-side — we never send it.
        assert!(parsed["sshKey"].get("keyFingerprint").is_none());
    }

    #[test]
    fn patch_ssh_keeps_fingerprint() {
        let base = r#"{"type":5,"name":"old","sshKey":{"privateKey":"old-priv","publicKey":"old-pub","keyFingerprint":"SHA256:abc"}}"#;
        let fields = vec![
            ef("Name", "new"),
            ef("Private Key", "new-priv"),
            ef("Public Key", "new-pub"),
        ];
        let patched = patch_edit_payload(base, &fields);
        let parsed: Value = serde_json::from_str(&patched).unwrap();
        assert_eq!(parsed["name"], "new");
        assert_eq!(parsed["sshKey"]["privateKey"], "new-priv");
        assert_eq!(parsed["sshKey"]["publicKey"], "new-pub");
        // Fingerprint preserved verbatim — bw will recompute it on save
        // but we don't blow it away locally first.
        assert_eq!(parsed["sshKey"]["keyFingerprint"], "SHA256:abc");
    }

    #[test]
    fn create_login_url_match_label_translates_to_enum() {
        let fields = vec![
            ef("Name", "site"),
            ef("Username", "u"),
            ef("Password", "p"),
            ef("URL", "https://example.com"),
            ef("URL Match", "Exact"),
            ef("Notes", ""),
        ];
        let json = build_create_payload(&CreateItemType::Login, &fields);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["login"]["uris"][0]["match"], 3);
    }

    #[test]
    fn create_login_url_match_empty_yields_null() {
        let fields = vec![
            ef("Name", "site"),
            ef("Username", "u"),
            ef("Password", "p"),
            ef("URL", "https://example.com"),
            ef("URL Match", ""),
            ef("Notes", ""),
        ];
        let json = build_create_payload(&CreateItemType::Login, &fields);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["login"]["uris"][0]["match"].is_null());
    }

    #[test]
    fn patch_login_url_match_round_trip() {
        let base = r#"{"type":1,"name":"s","login":{"username":"u","password":"p","uris":[{"uri":"https://x","match":3}]}}"#;
        // Uses the post-multi-URI API — patcher now reads URL rows by
        // their `kind`, not by label, so the constructor matters.
        let fields = vec![
            EditField::uri_url("URL", "https://y", 0),
            EditField::uri_match("URL Match", "Regex", 0),
        ];
        let patched = patch_edit_payload(base, &fields);
        let parsed: Value = serde_json::from_str(&patched).unwrap();
        assert_eq!(parsed["login"]["uris"][0]["uri"], "https://y");
        assert_eq!(parsed["login"]["uris"][0]["match"], 4);
    }

    fn ef_custom(label: &str, value: &str, field_type: u8) -> EditField {
        EditField::custom(label, value, field_type)
    }

    #[test]
    fn patch_writes_custom_fields_back() {
        // Existing item has one custom field "API_KEY" — user edits it
        // and adds a second.
        let base = r#"{"type":1,"name":"s","login":{"username":"u","password":"p"},"fields":[{"name":"API_KEY","value":"old","type":1,"linkedId":null}]}"#;
        let fields = vec![
            ef("Name", "s"),
            ef_custom("API_KEY", "new-secret", 1),
            ef_custom("Region", "us-east-1", 0),
        ];
        let patched = patch_edit_payload(base, &fields);
        let parsed: Value = serde_json::from_str(&patched).unwrap();
        let arr = parsed["fields"].as_array().expect("fields is an array");
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "API_KEY");
        assert_eq!(arr[0]["value"], "new-secret");
        assert_eq!(arr[0]["type"], 1);
        assert_eq!(arr[1]["name"], "Region");
        assert_eq!(arr[1]["value"], "us-east-1");
        assert_eq!(arr[1]["type"], 0);
    }

    #[test]
    fn patch_drops_removed_custom_fields() {
        // Item had two custom fields, user removed one — only the
        // remaining field should be in the output.
        let base = r#"{"type":1,"name":"s","fields":[{"name":"A","value":"1","type":0},{"name":"B","value":"2","type":0}]}"#;
        let fields = vec![ef("Name", "s"), ef_custom("A", "1", 0)];
        let patched = patch_edit_payload(base, &fields);
        let parsed: Value = serde_json::from_str(&patched).unwrap();
        let arr = parsed["fields"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "A");
    }

    #[test]
    fn patch_clears_fields_when_all_removed() {
        let base = r#"{"type":1,"name":"s","fields":[{"name":"A","value":"1","type":0}]}"#;
        let fields = vec![ef("Name", "s")];
        let patched = patch_edit_payload(base, &fields);
        let parsed: Value = serde_json::from_str(&patched).unwrap();
        assert!(parsed["fields"].as_array().unwrap().is_empty());
    }

    #[test]
    fn patch_preserves_linked_fields_verbatim() {
        // Item came from the official GUI with one linked field
        // (type=3, linkedId=42) and one regular text field. The TUI
        // does not surface linked fields as editable, so the form
        // only carries the regular one — the linked entry should
        // survive the round-trip with its `linkedId` intact.
        let base = r#"{
            "type": 1,
            "name": "s",
            "fields": [
                {"name":"Mirror","value":"","type":3,"linkedId":42},
                {"name":"Note","value":"hello","type":0,"linkedId":null}
            ]
        }"#;
        let fields = vec![ef("Name", "s"), ef_custom("Note", "hello updated", 0)];
        let patched = patch_edit_payload(base, &fields);
        let parsed: Value = serde_json::from_str(&patched).unwrap();
        let arr = parsed["fields"].as_array().expect("fields array");
        assert_eq!(arr.len(), 2, "linked + regular both present");
        // Linked fields go first.
        assert_eq!(arr[0]["name"], "Mirror");
        assert_eq!(arr[0]["type"], 3);
        assert_eq!(arr[0]["linkedId"], 42);
        // Regular field reflects the user's edit.
        assert_eq!(arr[1]["name"], "Note");
        assert_eq!(arr[1]["value"], "hello updated");
        assert_eq!(arr[1]["type"], 0);
    }

    #[test]
    fn patch_preserves_linked_fields_even_when_form_has_no_custom_rows() {
        // User opens an item that only has a linked field, never adds
        // any custom field of their own — the linked entry must still
        // survive the save.
        let base = r#"{
            "type": 1,
            "name": "s",
            "fields": [
                {"name":"Mirror","value":"","type":3,"linkedId":7}
            ]
        }"#;
        let fields = vec![ef("Name", "s")];
        let patched = patch_edit_payload(base, &fields);
        let parsed: Value = serde_json::from_str(&patched).unwrap();
        let arr = parsed["fields"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["type"], 3);
        assert_eq!(arr[0]["linkedId"], 7);
    }

    fn ef_url(label: &str, value: &str, idx: usize) -> EditField {
        EditField::uri_url(label, value, idx)
    }
    fn ef_url_match(label: &str, value: &str, idx: usize) -> EditField {
        EditField::uri_match(label, value, idx)
    }

    #[test]
    fn patch_writes_multiple_uris() {
        let base = r#"{"type":1,"name":"s","login":{"username":"u","password":"p","uris":[{"uri":"https://a","match":null}]}}"#;
        let fields = vec![
            ef("Name", "s"),
            ef("Username", "u"),
            ef("Password", "p"),
            ef_url("URL 1", "https://a", 0),
            ef_url_match("URL 1 Match", "Domain", 0),
            ef_url("URL 2", "https://b", 1),
            ef_url_match("URL 2 Match", "Exact", 1),
        ];
        let patched = patch_edit_payload(base, &fields);
        let parsed: Value = serde_json::from_str(&patched).unwrap();
        let arr = parsed["login"]["uris"].as_array().expect("uris is array");
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["uri"], "https://a");
        assert_eq!(arr[0]["match"], 0);
        assert_eq!(arr[1]["uri"], "https://b");
        assert_eq!(arr[1]["match"], 3);
    }

    #[test]
    fn patch_drops_removed_uri_pair() {
        let base = r#"{"type":1,"name":"s","login":{"username":"u","password":"p","uris":[{"uri":"https://a","match":null},{"uri":"https://b","match":null}]}}"#;
        // User removed slot 1 — only slot 0 should be saved.
        let fields = vec![
            ef("Name", "s"),
            ef("Username", "u"),
            ef("Password", "p"),
            ef_url("URL", "https://a", 0),
            ef_url_match("URL Match", "", 0),
        ];
        let patched = patch_edit_payload(base, &fields);
        let parsed: Value = serde_json::from_str(&patched).unwrap();
        let arr = parsed["login"]["uris"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["uri"], "https://a");
    }

    #[test]
    fn patch_writes_folder_id_when_present() {
        let base = r#"{"type":1,"name":"s","folderId":null}"#;
        // The flow has already resolved the typed folder name into
        // its id before reaching the patcher.
        let fields = vec![ef("Name", "s"), ef("Folder", "abc-123-uuid")];
        let patched = patch_edit_payload(base, &fields);
        let parsed: Value = serde_json::from_str(&patched).unwrap();
        assert_eq!(parsed["folderId"], "abc-123-uuid");
    }

    #[test]
    fn patch_clears_folder_id_when_empty() {
        let base = r#"{"type":1,"name":"s","folderId":"old-uuid"}"#;
        let fields = vec![ef("Name", "s"), ef("Folder", "")];
        let patched = patch_edit_payload(base, &fields);
        let parsed: Value = serde_json::from_str(&patched).unwrap();
        assert!(parsed["folderId"].is_null());
    }
}
