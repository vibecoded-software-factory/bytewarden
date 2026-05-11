//! Read-only detail-screen field model.
//!
//! Builds the ordered list of labelled rows the detail view renders for
//! a given [`Item`]. The same list is needed by:
//!
//! * [`crate::tui::view::detail`] — to render the cards;
//! * [`crate::tui::flows::items::enter_edit_mode`] — to map the
//!   currently-focused detail row onto the matching field of the edit
//!   form.
//!
//! Centralising the layout here ensures both consumers walk the rows
//! in the same order; otherwise a future change in one place would
//! silently desynchronise the cursor between detail and edit views.

use crate::domain::UriMatch;
use crate::domain::identity::{build_full_name, identity_fields};
use crate::domain::item::{Attachment, Item, item_type_label};

/// One row of the detail view.
pub struct DetailField {
    /// User-visible label (e.g. `"Username"`, `"Cardholder"`).
    pub label: String,
    /// Either the raw value or a masked placeholder, depending on
    /// `hidden` and the caller-supplied reveal flags.
    pub value: String,
    /// `true` when the row is currently rendered as masked dots.
    pub hidden: bool,
}

/// Builds the list of detail rows for `item`.
///
/// * `show` — whether the user pressed F2 on a hidden row.
/// * `reveal_idx` — the index of that row inside this list.
///
/// Together they decide which (if any) hidden row should be rendered
/// in cleartext for this frame. Empty optional fields are skipped, so
/// the resulting `Vec` may be shorter than the equivalent edit form.
pub fn build_detail_fields(item: &Item, show: bool, reveal_idx: usize) -> Vec<DetailField> {
    let mut f: Vec<DetailField> = vec![
        DetailField {
            label: "Name".into(),
            value: item.name.clone(),
            hidden: false,
        },
        DetailField {
            label: "Type".into(),
            value: item_type_label(item.item_type).into(),
            hidden: false,
        },
    ];

    if let Some(login) = &item.login {
        if let Some(u) = &login.username {
            f.push(DetailField {
                label: "Username".into(),
                value: u.clone(),
                hidden: false,
            });
        }
        let pass = login.password.as_deref().unwrap_or("").to_string();
        let rev = show && reveal_idx == f.len();
        f.push(DetailField {
            label: "Password".into(),
            value: if rev || pass.is_empty() {
                pass.clone()
            } else {
                "●".repeat(pass.chars().count().max(8))
            },
            hidden: !rev && !pass.is_empty(),
        });
        for uri_d in login.uris.iter().flatten() {
            if let Some(uri) = &uri_d.uri {
                // When a non-default match type is set, append it in
                // parentheses so the user sees autofill behaviour at a
                // glance without opening the edit form.
                let suffix = uri_d
                    .match_type
                    .and_then(UriMatch::from_u8)
                    .map(|m| format!("   (match: {})", m.label()))
                    .unwrap_or_default();
                f.push(DetailField {
                    label: "URL".into(),
                    value: format!("{uri}{suffix}"),
                    hidden: false,
                });
            }
        }
        if let Some(totp) = &login.totp {
            let rev = show && reveal_idx == f.len();
            f.push(DetailField {
                label: "TOTP".into(),
                value: if rev {
                    totp.clone()
                } else {
                    "●●●●●●".into()
                },
                hidden: !rev,
            });
        }
    }

    if let Some(card) = &item.card {
        push_opt_field(
            &mut f,
            "Cardholder",
            &card.cardholder_name,
            false,
            show,
            reveal_idx,
        );
        push_opt_field(&mut f, "Brand", &card.brand, false, show, reveal_idx);
        push_opt_field(&mut f, "Number", &card.number, true, show, reveal_idx);
        if card.exp_month.is_some() || card.exp_year.is_some() {
            f.push(DetailField {
                label: "Expiry".into(),
                value: format!(
                    "{}/{}",
                    card.exp_month.as_deref().unwrap_or("?"),
                    card.exp_year.as_deref().unwrap_or("?")
                ),
                hidden: false,
            });
        }
        push_opt_field(&mut f, "CVV", &card.code, true, show, reveal_idx);
    }

    if let Some(ssh) = &item.ssh_key {
        // Public key first (cleartext, longest line) then private (hidden,
        // F2 to reveal) then fingerprint.
        if let Some(pk) = &ssh.public_key
            && !pk.is_empty()
        {
            f.push(DetailField {
                label: "Public Key".into(),
                value: pk.clone(),
                hidden: false,
            });
        }
        if let Some(priv_key) = &ssh.private_key {
            let rev = show && reveal_idx == f.len();
            f.push(DetailField {
                label: "Private Key".into(),
                value: if rev || priv_key.is_empty() {
                    priv_key.clone()
                } else {
                    "●".repeat(8)
                },
                hidden: !rev && !priv_key.is_empty(),
            });
        }
        if let Some(fp) = &ssh.key_fingerprint
            && !fp.is_empty()
        {
            f.push(DetailField {
                label: "Fingerprint".into(),
                value: fp.clone(),
                hidden: false,
            });
        }
    }

    if let Some(id) = &item.identity {
        let full = build_full_name(
            id.title.as_deref(),
            id.first_name.as_deref(),
            id.middle_name.as_deref(),
            id.last_name.as_deref(),
        );
        if !full.is_empty() {
            f.push(DetailField {
                label: "Full Name".into(),
                value: full,
                hidden: false,
            });
        }
        let hidden_set = ["SSN", "Passport", "License"];
        for (lbl, val) in identity_fields(id) {
            push_opt_field(
                &mut f,
                lbl,
                val,
                hidden_set.contains(&lbl),
                show,
                reveal_idx,
            );
        }
    }

    for field in &item.fields {
        let name = field.name.as_deref().unwrap_or("Field").to_string();
        let value = field.value.as_deref().unwrap_or("").to_string();
        let is_hidden = field.field_type == 1;
        let rev = show && reveal_idx == f.len();
        f.push(DetailField {
            label: name,
            value: if is_hidden && !rev {
                "●".repeat(value.chars().count().max(4))
            } else {
                value
            },
            hidden: is_hidden && !rev,
        });
    }

    if let Some(notes) = &item.notes
        && !notes.is_empty()
    {
        f.push(DetailField {
            label: "Notes".into(),
            value: notes.clone(),
            hidden: false,
        });
    }

    // Attachments — one row per attachment, displayed as
    // "<file_name>   (<sizeName>)". Empty list omits the section
    // entirely so non-attachment items aren't cluttered.
    if let Some(atts) = &item.attachments {
        for att in atts {
            let size = att
                .size_name
                .as_deref()
                .map(|s| format!("   ({s})"))
                .unwrap_or_default();
            f.push(DetailField {
                label: "Attachment".into(),
                value: format!("{}{size}", att.file_name),
                hidden: false,
            });
        }
    }

    f
}

/// Returns the attachment that matches the row at `detail_idx` in the
/// list produced by [`build_detail_fields`], or `None` when the row
/// isn't an attachment row (or the index is out of range).
///
/// This walks the same builder as the renderer to stay in sync — the
/// alternative (counting attachment rows from the bottom) breaks
/// silently as soon as new field types are added below them.
pub fn attachment_at(item: &Item, detail_idx: usize) -> Option<&Attachment> {
    let rows = build_detail_fields(item, false, 0);
    let row = rows.get(detail_idx)?;
    if row.label != "Attachment" {
        return None;
    }
    // The renderer emits attachment rows in the same order as
    // `item.attachments`, after every other section. We count the
    // attachment rows that precede `detail_idx` and use that as the
    // index into `item.attachments`.
    let att_offset = rows[..detail_idx]
        .iter()
        .filter(|r| r.label == "Attachment")
        .count();
    item.attachments.as_ref().and_then(|a| a.get(att_offset))
}

/// Pushes a [`DetailField`] from an `Option<String>`, skipping when
/// `None` or empty.
fn push_opt_field(
    fields: &mut Vec<DetailField>,
    label: &str,
    val: &Option<String>,
    is_hidden: bool,
    show: bool,
    reveal_idx: usize,
) {
    if let Some(v) = val
        && !v.is_empty()
    {
        let rev = show && reveal_idx == fields.len();
        let hid = is_hidden && !rev;
        fields.push(DetailField {
            label: label.to_string(),
            value: if hid {
                "●".repeat(v.chars().count().max(4))
            } else {
                v.clone()
            },
            hidden: hid,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::item::{CardData, Field, IdentityData, LoginData, SshKeyData, UriData};

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
    fn always_starts_with_name_and_type() {
        let i = empty_item(2);
        let rows = build_detail_fields(&i, false, 0);
        assert_eq!(rows[0].label, "Name");
        assert_eq!(rows[1].label, "Type");
        assert_eq!(rows[1].value, "Secure Note");
    }

    #[test]
    fn login_password_is_masked_until_revealed() {
        let mut item = empty_item(1);
        item.login = Some(LoginData {
            username: Some("u".into()),
            password: Some("hunter2".into()),
            uris: None,
            totp: None,
        });
        let rows = build_detail_fields(&item, false, 0);
        let pw_idx = rows.iter().position(|r| r.label == "Password").unwrap();
        assert!(rows[pw_idx].hidden);
        assert!(rows[pw_idx].value.starts_with("●"));
        assert!(!rows[pw_idx].value.contains("hunter2"));

        // Revealed when show=true and reveal_idx points at the password row.
        let revealed = build_detail_fields(&item, true, pw_idx);
        assert!(!revealed[pw_idx].hidden);
        assert_eq!(revealed[pw_idx].value, "hunter2");
    }

    #[test]
    fn login_empty_password_is_not_hidden() {
        let mut item = empty_item(1);
        item.login = Some(LoginData {
            username: None,
            password: Some("".into()),
            uris: None,
            totp: None,
        });
        let rows = build_detail_fields(&item, false, 0);
        let pw = rows.iter().find(|r| r.label == "Password").unwrap();
        assert!(!pw.hidden);
        assert_eq!(pw.value, "");
    }

    #[test]
    fn login_username_omitted_when_none() {
        let mut item = empty_item(1);
        item.login = Some(LoginData {
            username: None,
            password: Some("p".into()),
            uris: None,
            totp: None,
        });
        let rows = build_detail_fields(&item, false, 0);
        assert!(!rows.iter().any(|r| r.label == "Username"));
    }

    #[test]
    fn login_uri_appends_match_label_when_set() {
        let mut item = empty_item(1);
        item.login = Some(LoginData {
            username: None,
            password: Some("p".into()),
            uris: Some(vec![UriData {
                uri: Some("https://x".into()),
                match_type: Some(3),
            }]),
            totp: None,
        });
        let rows = build_detail_fields(&item, false, 0);
        let url = rows.iter().find(|r| r.label == "URL").unwrap();
        assert!(url.value.contains("https://x"));
        assert!(url.value.contains("Exact"));
    }

    #[test]
    fn login_uri_no_suffix_when_match_unset() {
        let mut item = empty_item(1);
        item.login = Some(LoginData {
            username: None,
            password: Some("p".into()),
            uris: Some(vec![UriData {
                uri: Some("https://x".into()),
                match_type: None,
            }]),
            totp: None,
        });
        let rows = build_detail_fields(&item, false, 0);
        let url = rows.iter().find(|r| r.label == "URL").unwrap();
        assert_eq!(url.value, "https://x");
    }

    #[test]
    fn card_renders_expiry_when_either_part_present() {
        let mut item = empty_item(3);
        item.card = Some(CardData {
            cardholder_name: None,
            brand: None,
            number: None,
            exp_month: Some("01".into()),
            exp_year: None,
            code: None,
        });
        let rows = build_detail_fields(&item, false, 0);
        let exp = rows.iter().find(|r| r.label == "Expiry").unwrap();
        assert_eq!(exp.value, "01/?");
    }

    #[test]
    fn card_skips_expiry_when_both_parts_absent() {
        let mut item = empty_item(3);
        item.card = Some(CardData {
            cardholder_name: Some("JD".into()),
            brand: None,
            number: None,
            exp_month: None,
            exp_year: None,
            code: None,
        });
        let rows = build_detail_fields(&item, false, 0);
        assert!(!rows.iter().any(|r| r.label == "Expiry"));
    }

    #[test]
    fn ssh_orders_public_then_private_then_fingerprint() {
        let mut item = empty_item(5);
        item.ssh_key = Some(SshKeyData {
            private_key: Some("PRIV".into()),
            public_key: Some("PUB".into()),
            key_fingerprint: Some("SHA256:abc".into()),
        });
        let rows = build_detail_fields(&item, false, 0);
        let pub_idx = rows.iter().position(|r| r.label == "Public Key").unwrap();
        let priv_idx = rows.iter().position(|r| r.label == "Private Key").unwrap();
        let fp_idx = rows.iter().position(|r| r.label == "Fingerprint").unwrap();
        assert!(pub_idx < priv_idx && priv_idx < fp_idx);
        assert!(rows[priv_idx].hidden);
    }

    #[test]
    fn identity_full_name_only_when_some_part_present() {
        let mut item = empty_item(4);
        item.identity = Some(IdentityData {
            title: None,
            first_name: Some("Jane".into()),
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
        });
        let rows = build_detail_fields(&item, false, 0);
        let full = rows.iter().find(|r| r.label == "Full Name").unwrap();
        assert_eq!(full.value, "Jane");
    }

    #[test]
    fn identity_ssn_passport_license_are_hidden() {
        let mut item = empty_item(4);
        item.identity = Some(IdentityData {
            title: None,
            first_name: None,
            middle_name: None,
            last_name: None,
            email: None,
            phone: None,
            company: None,
            ssn: Some("123-45-6789".into()),
            passport: Some("PASS123".into()),
            license: Some("LIC456".into()),
            address1: None,
            address2: None,
            city: None,
            state: None,
            postal_code: None,
            country: None,
        });
        let rows = build_detail_fields(&item, false, 0);
        for label in ["SSN", "Passport", "License"] {
            let row = rows.iter().find(|r| r.label == label).unwrap();
            assert!(row.hidden, "{label} should be hidden");
            assert!(row.value.starts_with("●"));
        }
    }

    #[test]
    fn custom_hidden_field_is_masked_until_revealed() {
        let mut item = empty_item(2);
        item.fields = vec![Field {
            name: Some("Token".into()),
            value: Some("abc".into()),
            field_type: 1,
        }];
        let rows = build_detail_fields(&item, false, 0);
        let token_idx = rows.iter().position(|r| r.label == "Token").unwrap();
        assert!(rows[token_idx].hidden);

        let revealed = build_detail_fields(&item, true, token_idx);
        assert!(!revealed[token_idx].hidden);
        assert_eq!(revealed[token_idx].value, "abc");
    }

    #[test]
    fn notes_skipped_when_empty() {
        let mut item = empty_item(2);
        item.notes = Some("".into());
        let rows = build_detail_fields(&item, false, 0);
        assert!(!rows.iter().any(|r| r.label == "Notes"));
    }

    #[test]
    fn attachments_appear_with_size_label() {
        let mut item = empty_item(1);
        item.attachments = Some(vec![Attachment {
            id: "a1".into(),
            file_name: "file.pdf".into(),
            size: None,
            size_name: Some("45 KB".into()),
        }]);
        let rows = build_detail_fields(&item, false, 0);
        let att = rows.iter().find(|r| r.label == "Attachment").unwrap();
        assert!(att.value.contains("file.pdf"));
        assert!(att.value.contains("45 KB"));
    }

    #[test]
    fn attachment_at_resolves_attachment_row_to_its_record() {
        let mut item = empty_item(1);
        item.attachments = Some(vec![
            Attachment {
                id: "a1".into(),
                file_name: "first.pdf".into(),
                size: None,
                size_name: None,
            },
            Attachment {
                id: "a2".into(),
                file_name: "second.png".into(),
                size: None,
                size_name: None,
            },
        ]);
        let rows = build_detail_fields(&item, false, 0);
        let positions: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter_map(|(i, r)| (r.label == "Attachment").then_some(i))
            .collect();
        assert_eq!(positions.len(), 2);
        let first = attachment_at(&item, positions[0]).unwrap();
        assert_eq!(first.id, "a1");
        let second = attachment_at(&item, positions[1]).unwrap();
        assert_eq!(second.id, "a2");
    }

    #[test]
    fn attachment_at_returns_none_for_non_attachment_row() {
        let item = empty_item(2);
        // Row 0 is "Name", row 1 is "Type" — neither is an attachment.
        assert!(attachment_at(&item, 0).is_none());
        assert!(attachment_at(&item, 1).is_none());
    }

    #[test]
    fn attachment_at_returns_none_for_out_of_range() {
        let item = empty_item(2);
        assert!(attachment_at(&item, 999).is_none());
    }
}
