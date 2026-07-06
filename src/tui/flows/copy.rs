//! Clipboard / copy-to-clipboard flows.
//!
//! Clipboard writes are fast and stay synchronous on the render thread —
//! only the TOTP path needs the worker (it shells out to `bw get totp`
//! first, then writes the code).

use crate::domain::identity::{build_full_name, identity_fields};
use crate::domain::item::Item;
use crate::ports::BwError;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::reprompt::ProtectedAction;
use crate::tui::worker::{InFlight, WorkerRequest};

/// Performs the clipboard write and updates the action state.
///
/// Uses [`crate::ports::ClipboardPort::write_with_clear`] so the secret
/// is wiped after `app.clipboard_clear_secs` seconds (default 30; `0`
/// disables it). The clear is contingent on the clipboard still holding
/// the value we wrote.
fn write_clipboard(app: &mut App, text: String, success_msg: &str) {
    let ttl = app.clipboard_clear_secs;
    match app.clipboard.write_with_clear(&text, ttl) {
        Ok(()) => {
            app.push_cmd("clipboard", true, success_msg);
            app.set_action(ActionState::Done(copied_toast(ttl)));
        }
        Err(e) => {
            app.push_cmd("clipboard", false, &e);
            app.set_action(ActionState::Error(format!("Clipboard error: {e}")));
        }
    }
}

/// Renders the success toast for a clipboard write — adds the auto-clear
/// hint when a TTL is active.
fn copied_toast(ttl: u64) -> String {
    if ttl == 0 {
        "Copied ✓".to_string()
    } else {
        format!("Copied ✓ (clears in {ttl}s)")
    }
}

/// Copies a literal string with a custom toast.
pub fn copy_raw(app: &mut App, text: String, msg: &str) {
    write_clipboard(app, text, msg);
}

/// Copies the selected item's username (no secret → no reprompt).
pub fn copy_username_to_clipboard(app: &mut App) {
    let Some(item) = app.vault.selected_item() else {
        return;
    };
    let name = item.name.clone();
    let username = item
        .login
        .as_ref()
        .and_then(|l| l.username.clone())
        .unwrap_or_default();
    app.push_cmd("clipboard", true, &format!("username for {name}"));
    write_clipboard(app, username, "Username copied ✓");
}

/// Copies the selected item's password.
///
/// If the item carries the Bitwarden `reprompt` flag the request is
/// deferred behind a master-password popup; the popup re-enters this
/// function once verification succeeds (verified for this single action,
/// not cached).
pub fn copy_password_to_clipboard(app: &mut App) {
    if app.vault.selected_item().is_none() {
        return;
    }
    if super::reprompt::maybe_open(app, ProtectedAction::CopyPassword) {
        return;
    }
    let Some(item) = app.vault.selected_item() else {
        return;
    };
    let name = item.name.clone();
    let password = item
        .login
        .as_ref()
        .and_then(|l| l.password.clone())
        .unwrap_or_default();
    app.push_cmd("clipboard", true, &format!("password for {name} [hidden]"));
    write_clipboard(app, password, "Password copied ✓");
}

/// Copies the selected item's TOTP code, gated behind the reprompt popup
/// for `reprompt`-flagged items (the code is a secret). The popup
/// re-enters via [`ProtectedAction::CopyTotp`] once verification succeeds.
/// This is the entry point the vault-level callers (right-click menu) use;
/// the detail-row path reaches [`request_copy_totp`] through its own gate.
pub fn copy_totp_to_clipboard(app: &mut App) {
    let Some(item) = app.vault.selected_item() else {
        return;
    };
    let item_id = item.id.clone();
    if super::reprompt::maybe_open(app, ProtectedAction::CopyTotp(item_id.clone())) {
        return;
    }
    request_copy_totp(app, item_id);
}

/// Fetches and copies the selected item's TOTP code (worker round-trip).
pub fn request_copy_totp(app: &mut App, item_id: String) {
    app.submit(
        InFlight::CopyTotp,
        "Copying TOTP…",
        WorkerRequest::GetTotp { item_id },
    );
}

/// `bw get totp` response — writes the code to the clipboard.
pub fn handle_copy_totp(app: &mut App, r: Result<String, BwError>) {
    match r {
        Ok(v) => {
            app.push_cmd("bw get totp", true, "totp [hidden]");
            write_clipboard(app, v, "TOTP copied ✓");
        }
        Err(e) => app.cmd_err("bw get totp", &e, "Failed"),
    }
}

/// What copying a given detail row does. One entry per row, in the exact
/// order of [`crate::tui::detail_fields::build_detail_fields`], so the row
/// cursor and the copied value can never drift apart — a test pins the
/// two to the same length.
enum CopyTarget {
    /// The Type row and attachment rows — not copyable.
    Skip,
    /// Routes to [`copy_username_to_clipboard`].
    Username,
    /// Routes to [`copy_password_to_clipboard`] (reprompt-gated).
    Password,
    /// Routes to [`request_copy_totp`] (worker round-trip).
    Totp,
    /// A plain value copied verbatim, with its own toast label.
    Value { label: String, text: String },
}

/// Builds the ordered list of copy targets for `item`, mirroring
/// [`crate::tui::detail_fields::build_detail_fields`] row-for-row. This is
/// the single source of truth for "which value does the Nth detail row
/// copy" — extracted from a hand-walked `idx += 1` chain that silently
/// diverged from the renderer (it missed attachment rows entirely).
fn detail_copy_targets(item: &Item) -> Vec<CopyTarget> {
    let val = |label: &str, text: String| CopyTarget::Value {
        label: label.to_string(),
        text,
    };
    let mut t = vec![val("Name", item.name.clone()), CopyTarget::Skip]; // Name, Type

    if let Some(login) = &item.login {
        if login.username.is_some() {
            t.push(CopyTarget::Username);
        }
        t.push(CopyTarget::Password); // always present on a login
        for uri in login.uris.iter().flatten().filter_map(|u| u.uri.as_ref()) {
            // Copy the raw URI (the renderer appends a "(match: …)" hint).
            t.push(val("URL", uri.clone()));
        }
        if login.totp.is_some() {
            t.push(CopyTarget::Totp);
        }
    }

    if let Some(card) = &item.card {
        for (v, lbl) in [
            (card.cardholder_name.as_deref(), "Cardholder"),
            (card.brand.as_deref(), "Brand"),
            (card.number.as_deref(), "Number"),
        ] {
            if let Some(v) = v.filter(|v| !v.is_empty()) {
                t.push(val(lbl, v.to_string()));
            }
        }
        if card.exp_month.is_some() || card.exp_year.is_some() {
            t.push(val(
                "Expiry",
                format!(
                    "{}/{}",
                    card.exp_month.as_deref().unwrap_or("?"),
                    card.exp_year.as_deref().unwrap_or("?")
                ),
            ));
        }
        if let Some(v) = card.code.as_deref().filter(|v| !v.is_empty()) {
            t.push(val("CVV", v.to_string()));
        }
    }

    if let Some(ssh) = &item.ssh_key {
        if let Some(pk) = ssh.public_key.as_deref().filter(|v| !v.is_empty()) {
            t.push(val("Public Key", pk.to_string()));
        }
        if let Some(pk) = &ssh.private_key {
            t.push(val("Private Key", pk.clone()));
        }
        if let Some(fp) = ssh.key_fingerprint.as_deref().filter(|v| !v.is_empty()) {
            t.push(val("Fingerprint", fp.to_string()));
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
            t.push(val("Name", full));
        }
        for (lbl, value) in identity_fields(id) {
            if let Some(v) = value.as_deref().filter(|v| !v.is_empty()) {
                t.push(val(lbl, v.to_string()));
            }
        }
    }

    for field in &item.fields {
        t.push(val(
            field.name.as_deref().unwrap_or("Field"),
            field.value.as_deref().unwrap_or("").to_string(),
        ));
    }

    if let Some(notes) = item.notes.as_deref().filter(|n| !n.is_empty()) {
        t.push(val("Notes", notes.to_string()));
    }

    // One non-copyable row per attachment (the renderer shows these; the
    // old walk forgot them, so their index copied nothing).
    if let Some(atts) = &item.attachments {
        for _ in atts {
            t.push(CopyTarget::Skip);
        }
    }

    t
}

/// Copies the field under the detail view's row cursor.
///
/// The row order comes from [`detail_copy_targets`] (which mirrors the
/// renderer), so the cursor index can't select the wrong value. If the
/// item carries the `reprompt` flag *and* the focused row is a hidden
/// field (password / TOTP / hidden custom), the request is deferred
/// behind the reverify popup. Non-secret rows are not gated.
pub fn copy_selected_field(app: &mut App) {
    let item = match app.vault.selected_item() {
        Some(i) => i.clone(),
        None => return,
    };

    if item.needs_reprompt() {
        let rows = crate::tui::detail_fields::build_detail_fields(&item, true, 0);
        let focused_is_secret = rows.get(app.detail_field).is_some_and(|f| f.hidden);
        if focused_is_secret
            && super::reprompt::maybe_open(app, ProtectedAction::CopySelectedDetailField)
        {
            return;
        }
    }

    match detail_copy_targets(&item).into_iter().nth(app.detail_field) {
        Some(CopyTarget::Username) => copy_username_to_clipboard(app),
        Some(CopyTarget::Password) => copy_password_to_clipboard(app),
        Some(CopyTarget::Totp) => request_copy_totp(app, item.id.clone()),
        Some(CopyTarget::Value { label, text }) => {
            copy_raw(app, text, &format!("{label} copied ✓"))
        }
        Some(CopyTarget::Skip) | None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{copied_toast, detail_copy_targets};
    use crate::domain::item::{Attachment, CardData, Field, Item, LoginData, SshKeyData, UriData};
    use crate::tui::detail_fields::build_detail_fields;

    #[test]
    fn copied_toast_omits_hint_when_disabled() {
        assert_eq!(copied_toast(0), "Copied ✓");
    }

    #[test]
    fn copied_toast_includes_seconds_when_enabled() {
        assert_eq!(copied_toast(30), "Copied ✓ (clears in 30s)");
        assert_eq!(copied_toast(5), "Copied ✓ (clears in 5s)");
    }

    fn base(name: &str, item_type: u8) -> Item {
        Item {
            id: "x".into(),
            name: name.into(),
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
            fields: Vec::new(),
            attachments: None,
            reprompt: 0,
        }
    }

    /// The copy targets must line up 1:1 with the rendered detail rows —
    /// otherwise the row cursor copies the wrong value (the bug this
    /// extraction fixed: the old walk forgot attachment rows).
    #[test]
    fn copy_targets_align_with_detail_rows() {
        let mut login = base("Login", 1);
        login.login = Some(LoginData {
            username: Some("u".into()),
            password: Some("p".into()),
            uris: Some(vec![
                UriData {
                    uri: Some("https://a".into()),
                    match_type: Some(0),
                },
                UriData {
                    uri: Some("https://b".into()),
                    match_type: None,
                },
            ]),
            totp: Some("otpauth://x".into()),
        });
        login.fields = vec![
            Field {
                name: Some("api".into()),
                value: Some("k".into()),
                field_type: 0,
            },
            Field {
                name: Some("secret".into()),
                value: Some("s".into()),
                field_type: 1,
            },
        ];
        login.notes = Some("hello".into());
        login.attachments = Some(vec![Attachment {
            id: "a1".into(),
            file_name: "f.pdf".into(),
            size: None,
            size_name: Some("1 KB".into()),
        }]);

        let mut card = base("Card", 3);
        card.card = Some(CardData {
            cardholder_name: Some("CH".into()),
            brand: Some("Visa".into()),
            number: Some("4111".into()),
            exp_month: Some("12".into()),
            exp_year: Some("30".into()),
            code: Some("123".into()),
        });

        let mut ssh = base("Ssh", 5);
        ssh.ssh_key = Some(SshKeyData {
            private_key: Some("priv".into()),
            public_key: Some("pub".into()),
            key_fingerprint: Some("fp".into()),
        });

        for item in [login, card, ssh] {
            assert_eq!(
                detail_copy_targets(&item).len(),
                build_detail_fields(&item, false, 0).len(),
                "copy targets diverged from detail rows for {}",
                item.name
            );
        }
    }
}
