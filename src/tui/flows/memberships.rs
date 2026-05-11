//! Memberships popup flow — fetches organisations + collections and
//! parks them on the App for the read-only popup view to render.

use crate::domain::{Collection, Organization};
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::screens::Screen;

/// Cached snapshot of the user's organisation memberships, fetched
/// when the popup is opened.
#[derive(Debug, Clone, Default)]
pub struct MembershipState {
    pub organizations: Vec<Organization>,
    pub collections: Vec<Collection>,
}

/// Refreshes [`crate::tui::App::organizations`] and
/// [`crate::tui::App::collections`] from `bw` without showing a toast.
///
/// Called once after each successful login (and on the resume-from-
/// status fast path) so the Folders sidebar can render collection
/// rows immediately. Personal-only accounts return empty lists, in
/// which case the sidebar collapses back to its previous shape with
/// no extra rows.
///
/// Failures are logged but otherwise swallowed: missing memberships
/// just means the sidebar will not show collection rows. The user
/// can still operate on items and reload via the Memberships popup
/// if needed.
pub fn refresh_memberships_silent(app: &mut App) {
    let cmd = format!(
        "bw list organizations --session {}",
        app.session_key_display()
    );
    match app.vault.list_organizations() {
        Ok(orgs) => {
            let count = orgs.len();
            app.organizations = orgs;
            app.push_cmd(&cmd, true, &format!("{count} organisations loaded"));
        }
        Err(e) => {
            app.push_cmd(&cmd, false, &e);
            app.organizations.clear();
        }
    }

    let cmd = format!(
        "bw list collections --session {}",
        app.session_key_display()
    );
    match app.vault.list_collections() {
        Ok(mut cs) => {
            // Sort by `Org / Name` so the sidebar order is stable.
            // We use the org id as the primary key (org names are
            // resolved at render time, not here) so the order is
            // deterministic even if the org list refresh races with
            // the collection refresh.
            cs.sort_by(|a, b| {
                a.organization_id
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.organization_id.as_deref().unwrap_or(""))
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
            let count = cs.len();
            app.collections = cs;
            app.push_cmd(&cmd, true, &format!("{count} collections loaded"));
        }
        Err(e) => {
            app.push_cmd(&cmd, false, &e);
            app.collections.clear();
        }
    }
}

/// Opens the memberships popup, fetching the org + collection lists
/// fresh each time so the user sees current data without an extra
/// refresh action. On error the popup is not opened.
pub fn open(app: &mut App) {
    app.set_action(ActionState::Running("Loading memberships…".into()));
    let orgs = match app.vault.list_organizations() {
        Ok(v) => v,
        Err(e) => {
            app.cmd_err("bw list organizations", &e, "Memberships failed");
            return;
        }
    };
    let mut collections = match app.vault.list_collections() {
        Ok(v) => v,
        Err(e) => {
            app.cmd_err("bw list collections", &e, "Memberships failed");
            return;
        }
    };
    // Sort once on open so the per-org filter slice in the renderer
    // stays in display order without paying a re-sort + lowercased
    // allocations on every frame.
    collections.sort_by(|a, b| {
        a.organization_id
            .as_deref()
            .unwrap_or("")
            .cmp(b.organization_id.as_deref().unwrap_or(""))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    let total_orgs = orgs.len();
    let total_collections = collections.len();
    app.memberships = Some(MembershipState {
        organizations: orgs,
        collections,
    });
    app.set_action(ActionState::Done(format!(
        "Memberships ✓ ({total_orgs} org{}, {total_collections} collection{})",
        if total_orgs == 1 { "" } else { "s" },
        if total_collections == 1 { "" } else { "s" }
    )));
    app.screen = Screen::Memberships;
}

/// Closes the popup and returns to the vault list.
pub fn close(app: &mut App) {
    app.memberships = None;
    app.screen = Screen::Vault;
}
