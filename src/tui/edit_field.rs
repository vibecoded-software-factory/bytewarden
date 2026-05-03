//! Single-line editable field used in the create + edit forms.

use crate::domain::filter::CreateItemType;
use crate::domain::item::{Item, item_type_label};

/// Discriminates an [`EditField`] between a known built-in row of the
/// item schema (Name, Username, Password, …), a user-defined custom
/// field that lives in `item.fields[]`, and a single URI row of a
/// multi-URI login.
///
/// The variant carries everything `patch_edit_payload` needs to
/// faithfully rebuild the corresponding JSON sections on save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditFieldKind {
    /// One of the named built-in rows. The patcher routes its value to
    /// the matching JSON key by label lookup.
    BuiltIn,
    /// A user-defined custom field. The inner `u8` is the bw
    /// `field_type` (0 = text, 1 = hidden, 2 = boolean, 3 = linked).
    Custom(u8),
    /// One row of a multi-URI login. `index` is the URI's slot in the
    /// `uris[]` array (0-based); `role` says whether this row is the
    /// URL itself or its match-detection type.
    Uri { index: usize, role: UriRole },
}

/// Which half of a URI row this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UriRole {
    /// The URL string.
    Url,
    /// The match-detection type label (parsed by
    /// [`crate::domain::UriMatch::parse`]).
    Match,
}

/// One labelled text input with an internal cursor.
///
/// Cursor positions are character indices (not byte offsets), so the
/// widget handles multi-byte characters correctly.
#[derive(Debug, Clone)]
pub struct EditField {
    /// Display label (also used as a key when reading values back to
    /// build a JSON payload).
    pub label: String,

    /// Current value of the field.
    pub value: String,

    /// Whether this field is rendered masked unless [`Self::revealed`] is
    /// `true`.
    pub hidden: bool,

    /// `true` after the user pressed F2 to temporarily reveal a hidden
    /// field.
    pub revealed: bool,

    /// Cursor position as a *character* index (not byte offset).
    pub cursor: usize,

    /// `true` for fields that should not be modifiable (e.g. the item
    /// "Type" pseudo-field on the edit form).
    pub read_only: bool,

    /// Whether this row maps to a built-in schema field or to a row
    /// of `item.fields[]`. See [`EditFieldKind`].
    pub kind: EditFieldKind,
}

impl EditField {
    /// Builds an editable built-in field.
    pub fn new(label: &str, value: &str, hidden: bool) -> Self {
        Self {
            label: label.to_string(),
            value: value.to_string(),
            hidden,
            revealed: false,
            cursor: value.chars().count(),
            read_only: false,
            kind: EditFieldKind::BuiltIn,
        }
    }

    /// Builds a read-only built-in "field" used to display computed
    /// values such as the item type.
    pub fn read_only(label: &str, value: &str) -> Self {
        Self {
            read_only: true,
            ..Self::new(label, value, false)
        }
    }

    /// Builds an editable custom field row that maps to one entry in
    /// `item.fields[]`. `field_type` is the bw discriminant
    /// (0 = text, 1 = hidden, 2 = boolean).
    pub fn custom(label: &str, value: &str, field_type: u8) -> Self {
        Self {
            kind: EditFieldKind::Custom(field_type),
            ..Self::new(label, value, field_type == 1)
        }
    }

    /// Builds an editable URL row for a multi-URI login at the given
    /// slot. The label includes the index when there are multiple
    /// URIs (caller's choice); the `kind` carries the index for the
    /// patcher to reconstruct `uris[]`.
    pub fn uri_url(label: &str, value: &str, index: usize) -> Self {
        Self {
            kind: EditFieldKind::Uri {
                index,
                role: UriRole::Url,
            },
            ..Self::new(label, value, false)
        }
    }

    /// Builds an editable URL-Match row for a multi-URI login.
    pub fn uri_match(label: &str, value: &str, index: usize) -> Self {
        Self {
            kind: EditFieldKind::Uri {
                index,
                role: UriRole::Match,
            },
            ..Self::new(label, value, false)
        }
    }

    /// `true` when this row is part of a multi-URI block.
    pub fn is_uri(&self) -> bool {
        matches!(self.kind, EditFieldKind::Uri { .. })
    }

    /// Returns `true` when this row is a custom field (i.e. came from
    /// or will be written into `item.fields[]`).
    pub fn is_custom(&self) -> bool {
        matches!(self.kind, EditFieldKind::Custom(_))
    }

    /// Returns the bw `field_type` for a custom row, or `None` for
    /// any other row kind (built-in or URI).
    pub fn custom_type(&self) -> Option<u8> {
        match self.kind {
            EditFieldKind::Custom(t) => Some(t),
            EditFieldKind::BuiltIn | EditFieldKind::Uri { .. } => None,
        }
    }

    /// Updates a custom row's type, refreshing the masking flag.
    /// No-op on built-in rows.
    pub fn set_custom_type(&mut self, t: u8) {
        if let EditFieldKind::Custom(_) = self.kind {
            self.kind = EditFieldKind::Custom(t);
            self.hidden = t == 1;
            // Reset reveal so a switch from text → hidden masks the
            // value immediately rather than carrying the stale flag.
            self.revealed = false;
        }
    }

    /// Inserts `c` at the cursor and advances the cursor by one.
    pub fn insert(&mut self, c: char) {
        if self.read_only {
            return;
        }
        let byte = self.char_byte(self.cursor);
        self.value.insert(byte, c);
        self.cursor += 1;
    }

    /// Backspace — deletes the character before the cursor.
    pub fn delete_before(&mut self) {
        if self.read_only || self.cursor == 0 {
            return;
        }
        let byte = self.char_byte(self.cursor - 1);
        self.value.remove(byte);
        self.cursor -= 1;
    }

    /// Delete — removes the character under the cursor.
    pub fn delete_at(&mut self) {
        if self.read_only || self.cursor >= self.value.chars().count() {
            return;
        }
        let byte = self.char_byte(self.cursor);
        self.value.remove(byte);
    }

    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }
    pub fn cursor_right(&mut self) {
        if self.cursor < self.value.chars().count() {
            self.cursor += 1;
        }
    }
    pub fn cursor_home(&mut self) {
        self.cursor = 0;
    }
    pub fn cursor_end(&mut self) {
        self.cursor = self.value.chars().count();
    }

    /// Resolves a character index to its byte offset in the underlying
    /// `String`.
    fn char_byte(&self, char_idx: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.value.len())
    }
}

// ── Builders ──────────────────────────────────────────────────────────────

/// Builds the field set for editing an existing [`Item`].
pub fn build_edit_fields(item: &Item) -> Vec<EditField> {
    let mut f = vec![
        EditField::new("Name", &item.name, false),
        EditField::read_only("Type", item_type_label(item.item_type)),
    ];
    if let Some(l) = &item.login {
        f.push(EditField::new(
            "Username",
            l.username.as_deref().unwrap_or(""),
            false,
        ));
        f.push(EditField::new(
            "Password",
            l.password.as_deref().unwrap_or(""),
            true,
        ));
        // One labelled (URL, URL Match) pair per URI. Labels carry
        // the slot number when there are 2+ URIs so the user can tell
        // them apart visually; the patcher uses the `kind` (with
        // index + role), not the label, to reconstruct `uris[]`.
        let uris: Vec<&crate::domain::UriData> = l.uris.iter().flatten().collect();
        let multi = uris.len() > 1;
        for (i, uri) in uris.iter().enumerate() {
            let url_label = if multi {
                format!("URL {}", i + 1)
            } else {
                "URL".to_string()
            };
            let match_label = if multi {
                format!("URL {} Match", i + 1)
            } else {
                "URL Match".to_string()
            };
            f.push(EditField::uri_url(
                &url_label,
                uri.uri.as_deref().unwrap_or(""),
                i,
            ));
            let match_str = uri
                .match_type
                .and_then(crate::domain::UriMatch::from_u8)
                .map(|m| m.label().to_string())
                .unwrap_or_default();
            f.push(EditField::uri_match(&match_label, &match_str, i));
        }
        if let Some(t) = &l.totp {
            f.push(EditField::new("TOTP seed", t, true));
        }
    }
    if let Some(c) = &item.card {
        f.push(EditField::new(
            "Cardholder",
            c.cardholder_name.as_deref().unwrap_or(""),
            false,
        ));
        f.push(EditField::new(
            "Brand",
            c.brand.as_deref().unwrap_or(""),
            false,
        ));
        f.push(EditField::new(
            "Number",
            c.number.as_deref().unwrap_or(""),
            true,
        ));
        f.push(EditField::new(
            "Exp Month",
            c.exp_month.as_deref().unwrap_or(""),
            false,
        ));
        f.push(EditField::new(
            "Exp Year",
            c.exp_year.as_deref().unwrap_or(""),
            false,
        ));
        f.push(EditField::new("CVV", c.code.as_deref().unwrap_or(""), true));
    }
    if let Some(ssh) = &item.ssh_key {
        f.push(EditField::new(
            "Private Key",
            ssh.private_key.as_deref().unwrap_or(""),
            true,
        ));
        f.push(EditField::new(
            "Public Key",
            ssh.public_key.as_deref().unwrap_or(""),
            false,
        ));
        // Fingerprint is computed by `bw` — show it as read-only so the
        // user understands they can't edit it directly.
        f.push(EditField::read_only(
            "Fingerprint",
            ssh.key_fingerprint.as_deref().unwrap_or(""),
        ));
    }
    if let Some(id) = &item.identity {
        for (lbl, val, hid) in [
            ("Title", id.title.as_deref(), false),
            ("First Name", id.first_name.as_deref(), false),
            ("Middle", id.middle_name.as_deref(), false),
            ("Last Name", id.last_name.as_deref(), false),
            ("Email", id.email.as_deref(), false),
            ("Phone", id.phone.as_deref(), false),
            ("Company", id.company.as_deref(), false),
            ("Address", id.address1.as_deref(), false),
            ("Address 2", id.address2.as_deref(), false),
            ("City", id.city.as_deref(), false),
            ("State", id.state.as_deref(), false),
            ("ZIP", id.postal_code.as_deref(), false),
            ("Country", id.country.as_deref(), false),
            ("SSN", id.ssn.as_deref(), true),
            ("Passport", id.passport.as_deref(), true),
            ("License", id.license.as_deref(), true),
        ] {
            f.push(EditField::new(lbl, val.unwrap_or(""), hid));
        }
    }
    for field in &item.fields {
        let mut row = EditField::custom(
            field.name.as_deref().unwrap_or("Field"),
            field.value.as_deref().unwrap_or(""),
            field.field_type,
        );
        // Linked fields (type 3) reference another field on the same
        // item via a `linkedId` we can't pick from the TUI. Show them
        // read-only so the user can see what was set in the official
        // GUI but can't accidentally edit them into a regular field
        // and silently drop the link on save.
        if field.field_type == 3 {
            row.read_only = true;
        }
        f.push(row);
    }
    f.push(EditField::new(
        "Notes",
        item.notes.as_deref().unwrap_or(""),
        false,
    ));
    f
}

/// Builds the edit-form field set for `item`, with the "Folder" row
/// pre-populated to the folder name (looked up by id) when possible.
///
/// This is a thin wrapper around [`build_edit_fields`] used by the
/// edit-mode entry flow — it knows the folder list and so can show
/// the human-readable name. The patcher writes `folder_id` back by
/// name lookup at save time.
pub fn build_edit_fields_with_folders(
    item: &Item,
    folders: &[crate::domain::Folder],
) -> Vec<EditField> {
    let mut fields = build_edit_fields(item);
    let folder_name = item
        .folder_id
        .as_deref()
        .and_then(|id| folders.iter().find(|f| f.id == id).map(|f| f.name.clone()))
        .unwrap_or_default();
    // Insert "Folder" right after "Notes" so it stays out of the way
    // for the common edit cases.
    fields.push(EditField::new("Folder", &folder_name, false));
    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::item::{
        CardData, Field, IdentityData, Item, LoginData, SshKeyData, UriData,
    };

    fn empty_item(item_type: u8) -> Item {
        Item {
            id: "u".into(),
            name: "n".into(),
            item_type,
            login: None,
            card: None,
            identity: None,
            ssh_key: None,
            notes: None,
            folder_id: None,
            favorite: false,
            fields: vec![],
            attachments: None,
        }
    }

    #[test]
    fn new_initialises_cursor_at_end() {
        let f = EditField::new("Name", "abc", false);
        assert_eq!(f.cursor, 3);
        assert!(!f.read_only);
        assert!(!f.hidden);
    }

    #[test]
    fn read_only_does_not_accept_edits() {
        let mut f = EditField::read_only("Type", "Login");
        f.cursor = 0;
        f.insert('x');
        f.delete_at();
        f.delete_before();
        assert_eq!(f.value, "Login");
    }

    #[test]
    fn insert_advances_cursor_and_uses_byte_offset() {
        let mut f = EditField::new("v", "ñé", false);
        // Cursor sits at end (char count 2).
        assert_eq!(f.cursor, 2);
        f.cursor_home();
        f.insert('Z');
        assert_eq!(f.value, "Zñé");
        assert_eq!(f.cursor, 1);
    }

    #[test]
    fn delete_before_and_at_respect_multibyte() {
        let mut f = EditField::new("v", "ñé", false);
        // Cursor at end. delete_before removes 'é'.
        f.delete_before();
        assert_eq!(f.value, "ñ");
        assert_eq!(f.cursor, 1);
        // delete_at at end is a no-op.
        f.delete_at();
        assert_eq!(f.value, "ñ");
        // Move home and delete_at removes 'ñ'.
        f.cursor_home();
        f.delete_at();
        assert_eq!(f.value, "");
    }

    #[test]
    fn cursor_movement_clamps_at_bounds() {
        let mut f = EditField::new("v", "abc", false);
        f.cursor_home();
        f.cursor_left();
        assert_eq!(f.cursor, 0);
        f.cursor_end();
        f.cursor_right();
        assert_eq!(f.cursor, 3);
    }

    #[test]
    fn custom_field_hidden_flag_tracks_field_type() {
        let text = EditField::custom("API", "v", 0);
        assert!(!text.hidden);
        assert_eq!(text.custom_type(), Some(0));
        assert!(text.is_custom());
        let hidden = EditField::custom("Secret", "v", 1);
        assert!(hidden.hidden);
        assert_eq!(hidden.custom_type(), Some(1));
    }

    #[test]
    fn set_custom_type_refreshes_masking() {
        let mut f = EditField::custom("Field", "v", 0);
        f.revealed = true; // simulate previously revealed
        f.set_custom_type(1);
        assert!(f.hidden);
        assert!(!f.revealed); // reveal is reset on type change
        f.set_custom_type(0);
        assert!(!f.hidden);
    }

    #[test]
    fn set_custom_type_is_noop_on_builtin() {
        let mut f = EditField::new("Name", "v", false);
        f.set_custom_type(1);
        assert_eq!(f.custom_type(), None);
        assert!(!f.is_custom());
    }

    #[test]
    fn uri_helpers_set_kind() {
        let url = EditField::uri_url("URL", "https://x", 0);
        assert!(url.is_uri());
        assert!(!url.is_custom());
        match url.kind {
            EditFieldKind::Uri { index, role } => {
                assert_eq!(index, 0);
                assert_eq!(role, UriRole::Url);
            }
            _ => panic!("expected Uri kind"),
        }
        let m = EditField::uri_match("URL Match", "Domain", 2);
        match m.kind {
            EditFieldKind::Uri { index, role } => {
                assert_eq!(index, 2);
                assert_eq!(role, UriRole::Match);
            }
            _ => panic!("expected Uri kind"),
        }
    }

    #[test]
    fn build_edit_fields_login_includes_password_uri_and_totp() {
        let mut item = empty_item(1);
        item.login = Some(LoginData {
            username: Some("alice".into()),
            password: Some("secret".into()),
            uris: Some(vec![UriData {
                uri: Some("https://x".into()),
                match_type: Some(0),
            }]),
            totp: Some("seed".into()),
        });
        let fields = build_edit_fields(&item);
        let labels: Vec<&str> = fields.iter().map(|f| f.label.as_str()).collect();
        assert!(labels.contains(&"Username"));
        assert!(labels.contains(&"Password"));
        assert!(labels.contains(&"URL"));
        assert!(labels.contains(&"URL Match"));
        assert!(labels.contains(&"TOTP seed"));
        assert!(labels.contains(&"Notes"));
    }

    #[test]
    fn build_edit_fields_login_with_multiple_uris_numbers_them() {
        let mut item = empty_item(1);
        item.login = Some(LoginData {
            username: None,
            password: None,
            uris: Some(vec![
                UriData {
                    uri: Some("a".into()),
                    match_type: None,
                },
                UriData {
                    uri: Some("b".into()),
                    match_type: Some(3),
                },
            ]),
            totp: None,
        });
        let fields = build_edit_fields(&item);
        let labels: Vec<&str> = fields.iter().map(|f| f.label.as_str()).collect();
        assert!(labels.contains(&"URL 1"));
        assert!(labels.contains(&"URL 1 Match"));
        assert!(labels.contains(&"URL 2"));
        assert!(labels.contains(&"URL 2 Match"));
    }

    #[test]
    fn build_edit_fields_card_includes_all_card_fields() {
        let mut item = empty_item(3);
        item.card = Some(CardData {
            cardholder_name: Some("JD".into()),
            brand: Some("Visa".into()),
            number: Some("4242".into()),
            exp_month: Some("01".into()),
            exp_year: Some("2030".into()),
            code: Some("123".into()),
        });
        let labels: Vec<String> = build_edit_fields(&item)
            .into_iter()
            .map(|f| f.label)
            .collect();
        for need in [
            "Cardholder",
            "Brand",
            "Number",
            "Exp Month",
            "Exp Year",
            "CVV",
        ] {
            assert!(labels.iter().any(|l| l == need), "missing {need}");
        }
    }

    #[test]
    fn build_edit_fields_ssh_marks_fingerprint_read_only() {
        let mut item = empty_item(5);
        item.ssh_key = Some(SshKeyData {
            private_key: Some("PRIV".into()),
            public_key: Some("PUB".into()),
            key_fingerprint: Some("FP".into()),
        });
        let fields = build_edit_fields(&item);
        let fp = fields
            .iter()
            .find(|f| f.label == "Fingerprint")
            .expect("fp row");
        assert!(fp.read_only);
    }

    #[test]
    fn build_edit_fields_identity_covers_every_label() {
        let mut item = empty_item(4);
        item.identity = Some(IdentityData {
            title: None,
            first_name: Some("Jane".into()),
            middle_name: None,
            last_name: Some("Doe".into()),
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
        });
        let labels: Vec<String> = build_edit_fields(&item)
            .into_iter()
            .map(|f| f.label)
            .collect();
        for need in [
            "Title",
            "First Name",
            "Middle",
            "Last Name",
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
        ] {
            assert!(labels.iter().any(|l| l == need), "missing {need}");
        }
    }

    #[test]
    fn build_edit_fields_includes_user_custom_fields() {
        let mut item = empty_item(2);
        item.fields = vec![Field {
            name: Some("API".into()),
            value: Some("xyz".into()),
            field_type: 1,
        }];
        let fields = build_edit_fields(&item);
        let custom = fields
            .iter()
            .find(|f| f.label == "API")
            .expect("custom row");
        assert!(custom.is_custom());
        assert_eq!(custom.custom_type(), Some(1));
        assert!(custom.hidden);
    }

    #[test]
    fn build_edit_fields_with_folders_resolves_id_to_name() {
        let mut item = empty_item(2);
        item.folder_id = Some("f1".into());
        let folders = vec![crate::domain::Folder {
            id: "f1".into(),
            name: "Work".into(),
        }];
        let fields = build_edit_fields_with_folders(&item, &folders);
        let folder_row = fields
            .iter()
            .find(|f| f.label == "Folder")
            .expect("folder row");
        assert_eq!(folder_row.value, "Work");
    }

    #[test]
    fn build_edit_fields_with_folders_unknown_id_yields_empty() {
        let mut item = empty_item(2);
        item.folder_id = Some("missing".into());
        let folders: Vec<crate::domain::Folder> = vec![];
        let fields = build_edit_fields_with_folders(&item, &folders);
        let folder_row = fields
            .iter()
            .find(|f| f.label == "Folder")
            .expect("folder row");
        assert_eq!(folder_row.value, "");
    }

    #[test]
    fn build_create_fields_per_type() {
        let login = build_create_fields(&CreateItemType::Login);
        let labels: Vec<&str> = login.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Name", "Username", "Password", "URL", "URL Match", "Notes"]
        );

        let note = build_create_fields(&CreateItemType::SecureNote);
        assert_eq!(note.len(), 2);

        let card = build_create_fields(&CreateItemType::Card);
        let card_labels: Vec<&str> = card.iter().map(|f| f.label.as_str()).collect();
        assert!(card_labels.contains(&"Cardholder"));
        assert!(card_labels.contains(&"CVV"));

        let id = build_create_fields(&CreateItemType::Identity);
        let id_labels: Vec<&str> = id.iter().map(|f| f.label.as_str()).collect();
        assert!(id_labels.contains(&"First Name"));
        assert!(id_labels.contains(&"Country"));

        let ssh = build_create_fields(&CreateItemType::SshKey);
        let ssh_labels: Vec<&str> = ssh.iter().map(|f| f.label.as_str()).collect();
        assert!(ssh_labels.contains(&"Private Key"));
        // Fingerprint is computed by bw — not editable on create.
        assert!(!ssh_labels.iter().any(|l| l.contains("Fingerprint")));
    }

    #[test]
    fn build_create_fields_password_row_is_hidden() {
        let login = build_create_fields(&CreateItemType::Login);
        let pw = login.iter().find(|f| f.label == "Password").unwrap();
        assert!(pw.hidden);
    }
}

/// Builds the empty field set for the "create new item" form.
pub fn build_create_fields(item_type: &CreateItemType) -> Vec<EditField> {
    let ef = |label: &str, hidden: bool| EditField::new(label, "", hidden);
    match item_type {
        CreateItemType::Login => vec![
            ef("Name", false),
            ef("Username", false),
            ef("Password", true),
            ef("URL", false),
            // Empty = use bw default ("Domain"). Accepts label
            // ("Domain"/"Host"/"Starts With"/"Exact"/"Regex"/"Never")
            // or digit 0-5.
            ef("URL Match", false),
            ef("Notes", false),
        ],
        CreateItemType::SecureNote => vec![ef("Name", false), ef("Notes", false)],
        CreateItemType::Card => vec![
            ef("Name", false),
            ef("Cardholder", false),
            ef("Brand", false),
            ef("Number", true),
            ef("Exp Month", false),
            ef("Exp Year", false),
            ef("CVV", true),
            ef("Notes", false),
        ],
        CreateItemType::Identity => vec![
            ef("Name", false),
            ef("First Name", false),
            ef("Last Name", false),
            ef("Email", false),
            ef("Phone", false),
            ef("Company", false),
            ef("Address", false),
            ef("City", false),
            ef("State", false),
            ef("ZIP", false),
            ef("Country", false),
            ef("Notes", false),
        ],
        // The fingerprint is computed by `bw` from the private key, so
        // the create form doesn't accept it.
        CreateItemType::SshKey => vec![
            ef("Name", false),
            ef("Private Key", true),
            ef("Public Key", false),
            ef("Notes", false),
        ],
    }
}
