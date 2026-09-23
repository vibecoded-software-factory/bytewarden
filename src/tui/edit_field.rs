use crate::domain::LineEditor;
use crate::domain::filter::CreateItemType;
use crate::domain::item::{Item, item_type_label};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditFieldKind {
    BuiltIn,

    Custom(u8),

    Uri { index: usize, role: UriRole },

    Collections,

    Organization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UriRole {
    Url,

    Match,
}

#[derive(Debug, Clone)]
pub struct EditField {
    pub label: String,

    pub editor: LineEditor,

    pub hidden: bool,

    pub revealed: bool,

    pub read_only: bool,

    pub kind: EditFieldKind,

    pub collection_ids: Vec<String>,

    pub organization_id: Option<String>,
}

impl EditField {
    pub fn new(label: &str, value: &str, hidden: bool) -> Self {
        Self {
            label: label.to_string(),
            editor: LineEditor::with_text(value),
            hidden,
            revealed: false,
            read_only: false,
            kind: EditFieldKind::BuiltIn,
            collection_ids: Vec::new(),
            organization_id: None,
        }
    }

    pub fn value(&self) -> &str {
        self.editor.text()
    }

    pub fn organization(display: &str, id: Option<String>) -> Self {
        Self {
            read_only: true,
            kind: EditFieldKind::Organization,
            organization_id: id,
            ..Self::new("Organization", display, false)
        }
    }

    pub fn is_organization(&self) -> bool {
        matches!(self.kind, EditFieldKind::Organization)
    }

    pub fn collections(display: &str, ids: Vec<String>) -> Self {
        Self {
            read_only: true,
            kind: EditFieldKind::Collections,
            collection_ids: ids,
            ..Self::new("Collections", display, false)
        }
    }

    pub fn is_collections(&self) -> bool {
        matches!(self.kind, EditFieldKind::Collections)
    }

    pub fn read_only(label: &str, value: &str) -> Self {
        Self {
            read_only: true,
            ..Self::new(label, value, false)
        }
    }

    pub fn custom(label: &str, value: &str, field_type: u8) -> Self {
        Self {
            kind: EditFieldKind::Custom(field_type),
            ..Self::new(label, value, field_type == 1)
        }
    }

    pub fn uri_url(label: &str, value: &str, index: usize) -> Self {
        Self {
            kind: EditFieldKind::Uri {
                index,
                role: UriRole::Url,
            },
            ..Self::new(label, value, false)
        }
    }

    pub fn uri_match(label: &str, value: &str, index: usize) -> Self {
        Self {
            kind: EditFieldKind::Uri {
                index,
                role: UriRole::Match,
            },
            ..Self::new(label, value, false)
        }
    }

    pub fn is_uri(&self) -> bool {
        matches!(self.kind, EditFieldKind::Uri { .. })
    }

    pub fn is_custom(&self) -> bool {
        matches!(self.kind, EditFieldKind::Custom(_))
    }

    pub fn custom_type(&self) -> Option<u8> {
        match self.kind {
            EditFieldKind::Custom(t) => Some(t),
            EditFieldKind::BuiltIn
            | EditFieldKind::Uri { .. }
            | EditFieldKind::Collections
            | EditFieldKind::Organization => None,
        }
    }

    pub fn set_custom_type(&mut self, t: u8) {
        if let EditFieldKind::Custom(_) = self.kind {
            self.kind = EditFieldKind::Custom(t);
            self.hidden = t == 1;

            self.revealed = false;
        }
    }
}

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

pub fn build_edit_fields_with_folders(
    item: &Item,
    folders: &[crate::domain::Folder],
    collections: &[crate::domain::Collection],
) -> Vec<EditField> {
    let mut fields = build_edit_fields(item);
    let folder_name = item
        .folder_id
        .as_deref()
        .and_then(|id| folders.iter().find(|f| f.id == id).map(|f| f.name.clone()))
        .unwrap_or_default();

    fields.push(EditField::new("Folder", &folder_name, false));

    if item.organization_id.is_some() {
        let display: String = item
            .collection_ids
            .iter()
            .filter_map(|id| {
                collections
                    .iter()
                    .find(|c| &c.id == id)
                    .map(|c| c.name.as_str())
            })
            .collect::<Vec<_>>()
            .join(", ");
        fields.push(EditField::collections(
            &display,
            item.collection_ids.clone(),
        ));
    }

    fields
}

pub fn build_create_fields(item_type: &CreateItemType) -> Vec<EditField> {
    let ef = |label: &str, hidden: bool| EditField::new(label, "", hidden);
    match item_type {
        CreateItemType::Login => vec![
            ef("Name", false),
            ef("Username", false),
            ef("Password", true),
            ef("URL", false),
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

        CreateItemType::SshKey => vec![
            ef("Name", false),
            ef("Private Key", true),
            ef("Public Key", false),
            ef("Notes", false),
        ],
    }
}

pub fn build_create_fields_with_orgs(
    item_type: &CreateItemType,
    organizations: &[crate::domain::Organization],
) -> Vec<EditField> {
    let mut fields = build_create_fields(item_type);
    if !organizations.is_empty() {
        fields.push(EditField::organization("Personal", None));
    }
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
            organization_id: None,
            collection_ids: Vec::new(),
            favorite: false,
            fields: vec![],
            attachments: None,
            reprompt: 0,
        }
    }

    #[test]
    fn new_initialises_cursor_at_end() {
        let f = EditField::new("Name", "abc", false);
        assert_eq!(f.editor.cursor(), 3);
        assert_eq!(f.value(), "abc");
        assert!(!f.read_only);
        assert!(!f.hidden);
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
        f.revealed = true;
        f.set_custom_type(1);
        assert!(f.hidden);
        assert!(!f.revealed);
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
    fn organization_constructor_marks_row_correctly() {
        let f = EditField::organization("Acme", Some("o1".into()));
        assert!(f.is_organization());
        assert!(f.read_only);
        assert_eq!(f.label, "Organization");
        assert_eq!(f.value(), "Acme");
        assert_eq!(f.organization_id.as_deref(), Some("o1"));
        assert!(!f.is_collections());
        assert!(!f.is_custom());
        assert!(!f.is_uri());
    }

    #[test]
    fn organization_personal_has_no_id() {
        let f = EditField::organization("Personal", None);
        assert!(f.is_organization());
        assert_eq!(f.value(), "Personal");
        assert!(f.organization_id.is_none());
    }

    #[test]
    fn build_create_fields_with_orgs_skips_row_when_no_memberships() {
        let fields = build_create_fields_with_orgs(&CreateItemType::Login, &[]);
        assert!(!fields.iter().any(|f| f.is_organization()));
    }

    #[test]
    fn build_create_fields_with_orgs_appends_personal_default_when_orgs_present() {
        let orgs = vec![crate::domain::Organization {
            id: "o1".into(),
            name: "Acme".into(),
        }];
        let fields = build_create_fields_with_orgs(&CreateItemType::Login, &orgs);
        let org_row = fields
            .iter()
            .find(|f| f.is_organization())
            .expect("organization row");
        assert_eq!(org_row.value(), "Personal");
        assert!(org_row.organization_id.is_none());

        assert!(fields.last().is_some_and(|f| f.is_organization()));
    }

    #[test]
    fn collections_constructor_marks_row_correctly() {
        let f = EditField::collections("Eng, Ops", vec!["c1".into(), "c2".into()]);
        assert!(f.is_collections());
        assert!(f.read_only);
        assert_eq!(f.label, "Collections");
        assert_eq!(f.value(), "Eng, Ops");
        assert_eq!(f.collection_ids, vec!["c1".to_string(), "c2".to_string()]);

        assert!(f.custom_type().is_none());
        assert!(!f.is_custom());
        assert!(!f.is_uri());
    }

    #[test]
    fn build_edit_fields_with_folders_emits_collections_for_org_items() {
        let mut item = empty_item(1);
        item.organization_id = Some("o1".into());
        item.collection_ids = vec!["c1".into(), "c2".into()];
        let collections = vec![
            crate::domain::Collection {
                id: "c1".into(),
                name: "Engineering".into(),
                organization_id: Some("o1".into()),
            },
            crate::domain::Collection {
                id: "c2".into(),
                name: "Ops".into(),
                organization_id: Some("o1".into()),
            },
        ];
        let fields = build_edit_fields_with_folders(&item, &[], &collections);
        let row = fields
            .iter()
            .find(|f| f.is_collections())
            .expect("collections row present for org item");
        assert_eq!(row.value(), "Engineering, Ops");
        assert_eq!(row.collection_ids, vec!["c1".to_string(), "c2".to_string()]);
        assert!(row.read_only);
    }

    #[test]
    fn build_edit_fields_with_folders_skips_collections_for_personal_items() {
        let item = empty_item(1);

        let fields = build_edit_fields_with_folders(&item, &[], &[]);
        assert!(!fields.iter().any(|f| f.is_collections()));
    }

    #[test]
    fn build_edit_fields_with_folders_keeps_unknown_collection_id_in_payload() {
        let mut item = empty_item(1);
        item.organization_id = Some("o1".into());
        item.collection_ids = vec!["visible".into(), "hidden".into()];
        let collections = vec![crate::domain::Collection {
            id: "visible".into(),
            name: "Engineering".into(),
            organization_id: Some("o1".into()),
        }];
        let fields = build_edit_fields_with_folders(&item, &[], &collections);
        let row = fields
            .iter()
            .find(|f| f.is_collections())
            .expect("collections row");
        assert_eq!(row.value(), "Engineering");
        assert_eq!(
            row.collection_ids,
            vec!["visible".to_string(), "hidden".to_string()]
        );
    }

    #[test]
    fn build_edit_fields_with_folders_resolves_id_to_name() {
        let mut item = empty_item(2);
        item.folder_id = Some("f1".into());
        let folders = vec![crate::domain::Folder {
            id: "f1".into(),
            name: "Work".into(),
        }];
        let fields = build_edit_fields_with_folders(&item, &folders, &[]);
        let folder_row = fields
            .iter()
            .find(|f| f.label == "Folder")
            .expect("folder row");
        assert_eq!(folder_row.value(), "Work");
    }

    #[test]
    fn build_edit_fields_with_folders_unknown_id_yields_empty() {
        let mut item = empty_item(2);
        item.folder_id = Some("missing".into());
        let folders: Vec<crate::domain::Folder> = vec![];
        let fields = build_edit_fields_with_folders(&item, &folders, &[]);
        let folder_row = fields
            .iter()
            .find(|f| f.label == "Folder")
            .expect("folder row");
        assert_eq!(folder_row.value(), "");
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

        assert!(!ssh_labels.iter().any(|l| l.contains("Fingerprint")));
    }

    #[test]
    fn build_create_fields_password_row_is_hidden() {
        let login = build_create_fields(&CreateItemType::Login);
        let pw = login.iter().find(|f| f.label == "Password").unwrap();
        assert!(pw.hidden);
    }
}
