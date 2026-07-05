//! Memberships popup flow — fetches organisations + collections and
//! parks them on the App for the read-only popup view to render.

use crate::domain::{Collection, Organization};
use crate::ports::BwError;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::screens::Screen;
use crate::tui::worker::{InFlight, WorkerRequest};

/// Cached snapshot of the user's organisation memberships, fetched
/// when the popup is opened.
#[derive(Debug, Clone, Default)]
pub struct MembershipState {
    pub organizations: Vec<Organization>,
    pub collections: Vec<Collection>,
}

/// Opens the memberships popup — fetches organisations, then collections
/// (two worker round-trips). The popup is shown once both arrive.
pub fn open(app: &mut App) {
    app.set_action(ActionState::Running("Loading memberships…".into()));
    app.in_flight = Some(InFlight::MembershipsOrgs);
    let _ = app.worker_tx.send(WorkerRequest::ListOrganizations);
}

/// `bw list organizations` response — stashes the orgs and fetches the
/// collections.
pub fn handle_orgs(app: &mut App, r: Result<Vec<Organization>, BwError>) {
    match r {
        Ok(orgs) => {
            app.push_cmd(
                "bw list organizations",
                true,
                &format!("{} organisations loaded", orgs.len()),
            );
            app.memberships = Some(MembershipState {
                organizations: orgs,
                collections: Vec::new(),
            });
            app.in_flight = Some(InFlight::MembershipsCollections);
            let _ = app.worker_tx.send(WorkerRequest::ListCollections);
        }
        Err(e) => {
            app.memberships = None;
            app.cmd_err("bw list organizations", &e, "Memberships failed");
        }
    }
}

/// `bw list collections` response — completes the popup and shows it.
pub fn handle_collections(app: &mut App, r: Result<Vec<Collection>, BwError>) {
    match r {
        Ok(mut cs) => {
            // Sort by `Org / Name` so the per-org slices render in a
            // stable order without a per-frame re-sort.
            cs.sort_by(|a, b| {
                a.organization_id
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.organization_id.as_deref().unwrap_or(""))
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
            app.push_cmd(
                "bw list collections",
                true,
                &format!("{} collections loaded", cs.len()),
            );
            let total_orgs = app
                .memberships
                .as_ref()
                .map(|m| m.organizations.len())
                .unwrap_or(0);
            let total_collections = cs.len();
            if let Some(m) = app.memberships.as_mut() {
                m.collections = cs;
            }
            app.set_action(ActionState::Done(format!(
                "Memberships ✓ ({total_orgs} org{}, {total_collections} collection{})",
                if total_orgs == 1 { "" } else { "s" },
                if total_collections == 1 { "" } else { "s" }
            )));
            app.screen = Screen::Memberships;
        }
        Err(e) => {
            app.memberships = None;
            app.cmd_err("bw list collections", &e, "Memberships failed");
        }
    }
}

/// Closes the popup and returns to the vault list.
pub fn close(app: &mut App) {
    app.memberships = None;
    app.screen = Screen::Vault;
}
