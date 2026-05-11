//! Clipboard / copy-to-clipboard flows.

use crate::domain::identity::{build_full_name, identity_fields};
use crate::tui::action::{ActionState, PendingAction};
use crate::tui::app::App;
use crate::tui::reprompt::ProtectedAction;

/// Queues a copy action with a "Running…" label.
fn queue(app: &mut App, action: PendingAction, msg: &str) {
    app.set_action(ActionState::Running(msg.to_string()));
    app.pending_action = action;
}

// ── Triggered from key bindings ───────────────────────────────────────────

/// Queues a copy of the selected item's username.
pub fn copy_username_to_clipboard(app: &mut App) {
    if app.selected_item().is_some() {
        queue(app, PendingAction::CopyUsername, "Copying user…");
    }
}

/// Queues a copy of the selected item's password.
///
/// If the item carries the Bitwarden `reprompt` flag the request is
/// deferred behind a master-password popup; the popup re-enters this
/// function once verification succeeds (the flag has been verified
/// for this single action, not cached).
pub fn copy_password_to_clipboard(app: &mut App) {
    if app.selected_item().is_none() {
        return;
    }
    if super::reprompt::maybe_open(app, ProtectedAction::CopyPassword) {
        return;
    }
    queue(app, PendingAction::CopyPassword, "Copying pass…");
}

/// Selects which field to copy based on the detail view's row cursor.
///
/// The function walks the same field order as the detail renderer so
/// the cursor index stays in sync with what the user sees. If the
/// item carries the `reprompt` flag *and* the focused row is a
/// hidden field (password / TOTP / hidden custom field), the request
/// is deferred behind the reverify popup. Non-secret rows (name,
/// username, URL, …) on the same item are not gated — copying a
/// username from a reprompt-protected item is no more sensitive
/// than viewing the item itself.
pub fn copy_selected_field(app: &mut App) {
    let item = match app.selected_item() {
        Some(i) => i.clone(),
        None => return,
    };

    // Decide reprompt before walking the field list. We build the
    // detail-row view once (with `show_password = true` so the
    // hidden flag is the only signal we read) and read the focused
    // row's `hidden` boolean.
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
        return queue(
            app,
            PendingAction::CopyRaw(item.name.clone(), "Name copied ✓".into()),
            "Copying…",
        );
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
                return queue(
                    app,
                    PendingAction::CopyRaw(uri.clone(), "URL copied ✓".into()),
                    "Copying…",
                );
            }
            idx += 1;
        }
        if login.totp.is_some() {
            if app.detail_field == idx {
                return queue(
                    app,
                    PendingAction::CopyTotp(item.id.clone()),
                    "Copying TOTP…",
                );
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
                    return queue(
                        app,
                        PendingAction::CopyRaw(v.into(), format!("{lbl} copied ✓")),
                        "Copying…",
                    );
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
                return queue(
                    app,
                    PendingAction::CopyRaw(v, "Expiry copied ✓".into()),
                    "Copying…",
                );
            }
            idx += 1;
        }
        if let Some(v) = card.code.as_deref()
            && !v.is_empty()
        {
            if app.detail_field == idx {
                return queue(
                    app,
                    PendingAction::CopyRaw(v.into(), "CVV copied ✓".into()),
                    "Copying…",
                );
            }
            idx += 1;
        }
    }

    if let Some(ssh) = item.ssh_key.as_ref() {
        // Walk order must match `tui::detail_fields::build_detail_fields`:
        // Public Key → Private Key → Fingerprint, skipping empties.
        if let Some(pk) = &ssh.public_key
            && !pk.is_empty()
        {
            if app.detail_field == idx {
                return queue(
                    app,
                    PendingAction::CopyRaw(pk.clone(), "Public Key copied ✓".into()),
                    "Copying…",
                );
            }
            idx += 1;
        }
        if let Some(priv_key) = &ssh.private_key {
            // Private key row is always present in the detail view as
            // long as the SSH payload exists, so account for it whether
            // it is empty or not — the renderer mirrors that behaviour.
            if app.detail_field == idx {
                return queue(
                    app,
                    PendingAction::CopyRaw(priv_key.clone(), "Private Key copied ✓".into()),
                    "Copying…",
                );
            }
            idx += 1;
        }
        if let Some(fp) = &ssh.key_fingerprint
            && !fp.is_empty()
        {
            if app.detail_field == idx {
                return queue(
                    app,
                    PendingAction::CopyRaw(fp.clone(), "Fingerprint copied ✓".into()),
                    "Copying…",
                );
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
                return queue(
                    app,
                    PendingAction::CopyRaw(full, "Name copied ✓".into()),
                    "Copying…",
                );
            }
            idx += 1;
        }
        for (lbl, val) in identity_fields(id) {
            if let Some(v) = val
                && !v.is_empty()
            {
                if app.detail_field == idx {
                    return queue(
                        app,
                        PendingAction::CopyRaw(v.to_string(), format!("{lbl} copied ✓")),
                        "Copying…",
                    );
                }
                idx += 1;
            }
        }
    }

    for field in &item.fields {
        let value = field.value.as_deref().unwrap_or("");
        let label = field.name.as_deref().unwrap_or("Field");
        if app.detail_field == idx {
            return queue(
                app,
                PendingAction::CopyRaw(value.into(), format!("{label} copied ✓")),
                "Copying…",
            );
        }
        idx += 1;
    }

    if let Some(notes) = &item.notes
        && !notes.is_empty()
        && app.detail_field == idx
    {
        queue(
            app,
            PendingAction::CopyRaw(notes.clone(), "Notes copied ✓".into()),
            "Copying…",
        );
    }
}

// ── Pending-action executors ──────────────────────────────────────────────

/// Performs the actual clipboard write and updates the action state.
///
/// Uses [`crate::ports::ClipboardPort::write_with_clear`] so the
/// secret is wiped after `app.clipboard_clear_secs` seconds (default
/// 30; `0` disables the auto-clear). The clear is contingent on the
/// clipboard still holding the value we wrote — if the user copied
/// something else in the meantime we leave their selection alone.
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

/// Renders the success toast for a clipboard write — adds the
/// auto-clear hint when a TTL is active so the user knows the
/// password isn't going to sit there forever.
fn copied_toast(ttl: u64) -> String {
    if ttl == 0 {
        "Copied ✓".to_string()
    } else {
        format!("Copied ✓ (clears in {ttl}s)")
    }
}

pub fn do_copy_username(app: &mut App) {
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

pub fn do_copy_password(app: &mut App) {
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

pub fn do_copy_totp(app: &mut App, item_id: String) {
    let cmd = format!("bw get totp {item_id}");
    match app.vault.get_totp(&item_id) {
        Ok(v) => {
            app.push_cmd(&cmd, true, "totp [hidden]");
            write_clipboard(app, v, "TOTP copied ✓");
        }
        Err(e) => app.cmd_err(&cmd, &e, "Failed"),
    }
}

pub fn do_copy_raw(app: &mut App, text: String, msg: String) {
    app.push_cmd("clipboard", true, &msg);
    write_clipboard(app, text, &msg);
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
