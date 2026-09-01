use crate::domain::{Collection, Organization};
use crate::ports::BwError;
use crate::tui::action::ActionState;
use crate::tui::app::App;
use crate::tui::screens::Screen;
use crate::tui::worker::{InFlight, WorkerRequest};

#[derive(Debug, Clone, Default)]
pub struct MembershipState {
    pub organizations: Vec<Organization>,
    pub collections: Vec<Collection>,

    pub cursor: usize,
}

impl MembershipState {
    pub fn selectable_len(&self) -> usize {
        self.collections.len()
    }
}

pub fn open(app: &mut App) {
    app.submit(
        InFlight::MembershipsOrgs,
        "Loading memberships…",
        WorkerRequest::ListOrganizations,
    );
}

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
                cursor: 0,
            });

            if app.begin(InFlight::MembershipsCollections) {
                let _ = app.worker_tx.send(WorkerRequest::ListCollections);
            }
        }
        Err(e) => {
            app.memberships = None;
            app.cmd_err("bw list organizations", &e, "Memberships failed");
        }
    }
}

pub fn handle_collections(app: &mut App, r: Result<Vec<Collection>, BwError>) {
    match r {
        Ok(mut cs) => {
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

pub fn close(app: &mut App) {
    app.memberships = None;
    app.screen = Screen::Vault;
}

pub fn move_cursor(app: &mut App, delta: i8) {
    if let Some(state) = app.memberships.as_mut() {
        let len = state.selectable_len();
        crate::tui::input::nav::nav_clamp(&mut state.cursor, len, delta);
    }
}
