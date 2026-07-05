//! Clipboard / copy-to-clipboard flows.
//!
//! Clipboard writes are fast and stay synchronous on the render thread —
//! only the TOTP path needs the worker (it shells out to `bw get totp`
//! first, then writes the code).

use crate::domain::identity::{build_full_name, identity_fields};
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
    let Some(item) = app.selected_item() else {
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
    if app.selected_item().is_none() {
        return;
    }
    if super::reprompt::maybe_open(app, ProtectedAction::CopyPassword) {
        return;
    }
    let Some(item) = app.selected_item() else {
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

/// Copies the field under the detail view's row cursor.
///
/// Walks the same field order as the detail renderer so the cursor index
/// stays in sync. If the item carries the `reprompt` flag *and* the
/// focused row is a hidden field (password / TOTP / hidden custom), the
/// request is deferred behind the reverify popup. Non-secret rows are not
/// gated.
pub fn copy_selected_field(app: &mut App) {
    let item = match app.selected_item() {
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

    let mut idx = 0usize;

    if app.detail_field == idx {
        return copy_raw(app, item.name.clone(), "Name copied ✓");
    }
    idx += 1;
    if app.detail_field == idx {
        return; // Type — not useful to copy.
    }
    idx += 1;

    if let Some(login) = item.login.as_ref() {
        if login.username.is_some() {
            if app.detail_field == idx {
                return copy_username_to_clipboard(app);
            }
            idx += 1;
        }
        if app.detail_field == idx {
            return copy_password_to_clipboard(app);
        }
        idx += 1;
        for uri in login
            .uris
            .iter()
            .flat_map(|u| u.iter())
            .filter_map(|u| u.uri.as_ref())
        {
            if app.detail_field == idx {
                return copy_raw(app, uri.clone(), "URL copied ✓");
            }
            idx += 1;
        }
        if login.totp.is_some() {
            if app.detail_field == idx {
                return request_copy_totp(app, item.id.clone());
            }
            idx += 1;
        }
    }

    if let Some(card) = item.card.as_ref() {
        for (val, lbl) in [
            (card.cardholder_name.as_deref(), "Cardholder"),
            (card.brand.as_deref(), "Brand"),
            (card.number.as_deref(), "Number"),
        ] {
            if let Some(v) = val
                && !v.is_empty()
            {
                if app.detail_field == idx {
                    return copy_raw(app, v.into(), &format!("{lbl} copied ✓"));
                }
                idx += 1;
            }
        }
        if card.exp_month.is_some() || card.exp_year.is_some() {
            if app.detail_field == idx {
                let v = format!(
                    "{}/{}",
                    card.exp_month.as_deref().unwrap_or("?"),
                    card.exp_year.as_deref().unwrap_or("?")
                );
                return copy_raw(app, v, "Expiry copied ✓");
            }
            idx += 1;
        }
        if let Some(v) = card.code.as_deref()
            && !v.is_empty()
        {
            if app.detail_field == idx {
                return copy_raw(app, v.into(), "CVV copied ✓");
            }
            idx += 1;
        }
    }

    if let Some(ssh) = item.ssh_key.as_ref() {
        if let Some(pk) = &ssh.public_key
            && !pk.is_empty()
        {
            if app.detail_field == idx {
                return copy_raw(app, pk.clone(), "Public Key copied ✓");
            }
            idx += 1;
        }
        if let Some(priv_key) = &ssh.private_key {
            if app.detail_field == idx {
                return copy_raw(app, priv_key.clone(), "Private Key copied ✓");
            }
            idx += 1;
        }
        if let Some(fp) = &ssh.key_fingerprint
            && !fp.is_empty()
        {
            if app.detail_field == idx {
                return copy_raw(app, fp.clone(), "Fingerprint copied ✓");
            }
            idx += 1;
        }
    }

    if let Some(id) = item.identity.as_ref() {
        let full = build_full_name(
            id.title.as_deref(),
            id.first_name.as_deref(),
            id.middle_name.as_deref(),
            id.last_name.as_deref(),
        );
        if !full.is_empty() {
            if app.detail_field == idx {
                return copy_raw(app, full, "Name copied ✓");
            }
            idx += 1;
        }
        for (lbl, val) in identity_fields(id) {
            if let Some(v) = val
                && !v.is_empty()
            {
                if app.detail_field == idx {
                    return copy_raw(app, v.to_string(), &format!("{lbl} copied ✓"));
                }
                idx += 1;
            }
        }
    }

    for field in &item.fields {
        let value = field.value.as_deref().unwrap_or("");
        let label = field.name.as_deref().unwrap_or("Field");
        if app.detail_field == idx {
            return copy_raw(app, value.into(), &format!("{label} copied ✓"));
        }
        idx += 1;
    }

    if let Some(notes) = &item.notes
        && !notes.is_empty()
        && app.detail_field == idx
    {
        copy_raw(app, notes.clone(), "Notes copied ✓");
    }
}

#[cfg(test)]
mod tests {
    use super::copied_toast;

    #[test]
    fn copied_toast_omits_hint_when_disabled() {
        assert_eq!(copied_toast(0), "Copied ✓");
    }

    #[test]
    fn copied_toast_includes_seconds_when_enabled() {
        assert_eq!(copied_toast(30), "Copied ✓ (clears in 30s)");
        assert_eq!(copied_toast(5), "Copied ✓ (clears in 5s)");
    }
}
