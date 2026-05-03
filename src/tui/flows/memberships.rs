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
    let collections = match app.vault.list_collections() {
        Ok(v) => v,
        Err(e) => {
            app.cmd_err("bw list collections", &e, "Memberships failed");
            return;
        }
    };
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
