//! Clipboard / copy-to-clipboard flows.

use crate::domain::identity::{build_full_name, identity_fields};
use crate::tui::action::{ActionState, PendingAction};
use crate::tui::app::App;

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
pub fn copy_password_to_clipboard(app: &mut App) {
    if app.selected_item().is_some() {
        queue(app, PendingAction::CopyPassword, "Copying pass…");
    }
}

/// Selects which field to copy based on the detail view's row cursor.
///
/// The function walks the same field order as the detail renderer so
/// the cursor index stays in sync with what the user sees.
pub fn copy_selected_field(app: &mut App) {
    let item = match app.selected_item() {
        Some(i) => i.clone(),
        None => return,
    };
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
fn write_clipboard(app: &mut App, text: String, success_msg: &str) {
    match app.clipboard.write(&text) {
        Ok(()) => {
            app.push_cmd("clipboard", true, success_msg);
            app.set_action(ActionState::Done("Copied ✓".into()));
        }
        Err(e) => {
            app.push_cmd("clipboard", false, &e);
            app.set_action(ActionState::Error(format!("Clipboard error: {e}")));
        }
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
    let cmd = format!(
        "bw get totp {} --session {}",
        item_id,
        app.session_key_display()
    );
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
