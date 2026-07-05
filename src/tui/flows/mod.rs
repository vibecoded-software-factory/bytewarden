//! Multi-step user flows that mutate [`crate::tui::App`] and talk to the
//! ports via the worker thread.
//!
//! Each flow splits into two halves:
//!
//! * a `request_*` builder — validates input, stashes a
//!   [`crate::tui::worker::InFlight`] ticket on `App`, sets a `Running`
//!   toast, and sends a [`WorkerRequest`] on `app.worker_tx`;
//! * a `handle_*` response handler — invoked by [`apply_response`] when
//!   the matching [`WorkerResponse`] arrives; it mutates `App` and may
//!   chain the next step by calling another `request_*`.
//!
//! Free functions take `&mut App` so they can be dispatched without
//! method-resolution gymnastics.

pub mod assign_collections;
pub mod auth;
pub mod copy;
pub mod export;
pub mod folders;
pub mod generator;
pub mod import;
pub mod item_json;
pub mod items;
pub mod memberships;
pub mod palette;
pub mod reprompt;
pub mod send;
pub mod vault;

use crate::tui::app::App;
use crate::tui::worker::{InFlight, WorkerResponse};

/// `true` for responses that carry no [`InFlight`] ticket and are routed
/// purely by variant (fire-and-forget). Today that's only `bw lock`,
/// whose UI state was already reset on the render thread when the request
/// was sent. Kept as a pure predicate so the routing is unit-testable.
pub fn is_fire_and_forget(resp: &WorkerResponse) -> bool {
    matches!(resp, WorkerResponse::Locked)
}

/// Routes one worker response: fire-and-forget variants first, then the
/// `(in_flight, response)` pair to the owning `handle_*`.
pub fn apply_response(app: &mut App, resp: WorkerResponse) {
    if is_fire_and_forget(&resp) {
        return;
    }
    let ticket = app.in_flight.take();
    match (ticket, resp) {
        // ── Boot / auth ───────────────────────────────────────────────
        (Some(InFlight::BootStatus), WorkerResponse::Status(r)) => auth::handle_boot_status(app, r),
        (Some(InFlight::ResumeItems), WorkerResponse::Items(r)) => {
            auth::handle_resume_items(app, r)
        }
        (Some(InFlight::ResumeSessionData), WorkerResponse::SessionData(d)) => {
            auth::handle_resume_session_data(app, d)
        }
        (Some(InFlight::Login), WorkerResponse::Login(o)) => auth::handle_login(app, o),
        (Some(InFlight::Unlock), WorkerResponse::SessionKey(r)) => auth::handle_unlock(app, r),
        (Some(InFlight::LoginOtp), WorkerResponse::SessionKey(r)) => auth::handle_login_otp(app, r),
        (Some(InFlight::LoginTwoFactor), WorkerResponse::SessionKey(r)) => {
            auth::handle_login_two_factor(app, r)
        }
        (Some(InFlight::LoginApiKey), WorkerResponse::LoginLocked(r)) => {
            auth::handle_api_key(app, r)
        }
        (Some(InFlight::LoginSso), WorkerResponse::LoginLocked(r)) => auth::handle_sso(app, r),
        (Some(InFlight::PostLoginItems), WorkerResponse::Items(r)) => {
            auth::handle_post_login_items(app, r)
        }
        (Some(InFlight::PostLoginSessionData), WorkerResponse::SessionData(d)) => {
            auth::handle_post_login_session_data(app, d)
        }
        (Some(InFlight::Logout), WorkerResponse::Logout(r)) => auth::handle_logout(app, r),
        (Some(InFlight::SetServer), WorkerResponse::SetServer(r)) => {
            auth::handle_set_server(app, r)
        }
        (Some(InFlight::Fingerprint), WorkerResponse::Fingerprint(r)) => {
            auth::handle_fingerprint(app, r)
        }

        // ── Vault list ────────────────────────────────────────────────
        (Some(InFlight::LoadItems), WorkerResponse::Items(r)) => vault::handle_load_items(app, r),
        (Some(InFlight::ReloadItemsSilent), WorkerResponse::Items(r)) => {
            vault::handle_reload_items_silent(app, r)
        }
        (Some(InFlight::LoadTrash), WorkerResponse::Trash(r)) => vault::handle_load_trash(app, r),
        (Some(InFlight::Sync), WorkerResponse::Unit(r)) => vault::handle_sync(app, r),
        (Some(InFlight::SyncReload), WorkerResponse::Items(r)) => vault::handle_sync_reload(app, r),

        // ── Items CRUD ────────────────────────────────────────────────
        (Some(InFlight::CreateItem), WorkerResponse::Item(r)) => {
            items::handle_create(app, r.map(|b| *b))
        }
        (Some(InFlight::SaveEditFetch), WorkerResponse::ItemJson(r)) => {
            items::handle_save_edit_fetch(app, r)
        }
        (Some(InFlight::SaveEditCommit), WorkerResponse::Item(r)) => {
            items::handle_save_edit_commit(app, r.map(|b| *b))
        }
        (Some(InFlight::ToggleFavoriteFetch { item_id }), WorkerResponse::ItemJson(r)) => {
            items::handle_toggle_fetch(app, item_id, r)
        }
        (Some(InFlight::ToggleFavoriteCommit { new_favorite }), WorkerResponse::Item(r)) => {
            items::handle_toggle_commit(app, new_favorite, r.map(|b| *b))
        }
        (
            Some(InFlight::DeleteItem {
                permanent,
                item_id,
                name,
            }),
            WorkerResponse::Unit(r),
        ) => items::handle_delete(app, permanent, item_id, name, r),
        (Some(InFlight::DeleteReloadTrash), WorkerResponse::Trash(r)) => {
            items::handle_delete_reload_trash(app, r)
        }
        (Some(InFlight::RestoreItem { item_id, name }), WorkerResponse::Unit(r)) => {
            items::handle_restore(app, item_id, name, r)
        }
        (Some(InFlight::RestoreReloadItems), WorkerResponse::Items(r)) => {
            items::handle_restore_reload(app, r)
        }
        (Some(InFlight::CheckExposed), WorkerResponse::Exposed(r)) => {
            items::handle_check_exposed(app, r)
        }
        (Some(InFlight::DownloadAttachment), WorkerResponse::Unit(r)) => {
            items::handle_download_attachment(app, r)
        }
        (Some(InFlight::DeleteAttachment), WorkerResponse::Unit(r)) => {
            items::handle_delete_attachment(app, r)
        }
        (Some(InFlight::DeleteAttachmentRefresh { item_id }), WorkerResponse::ItemJson(r)) => {
            items::handle_delete_attachment_refresh(app, item_id, r)
        }
        (Some(InFlight::UploadAttachment), WorkerResponse::Item(r)) => {
            items::handle_upload_attachment(app, r.map(|b| *b))
        }

        // ── Copy (TOTP) ───────────────────────────────────────────────
        (Some(InFlight::CopyTotp), WorkerResponse::Totp(r)) => copy::handle_copy_totp(app, r),

        // ── Folders ───────────────────────────────────────────────────
        (Some(InFlight::CreateFolder), WorkerResponse::Folder(r)) => folders::handle_create(app, r),
        (Some(InFlight::EditFolder), WorkerResponse::Folder(r)) => folders::handle_edit(app, r),
        (Some(InFlight::DeleteFolder { name }), WorkerResponse::Unit(r)) => {
            folders::handle_delete(app, name, r)
        }
        (Some(InFlight::FolderReload), WorkerResponse::Folders(r)) => {
            folders::handle_reload(app, r)
        }
        (Some(InFlight::FolderDeleteReloadItems), WorkerResponse::Items(r)) => {
            folders::handle_delete_reload_items(app, r)
        }

        // ── Export / Import ───────────────────────────────────────────
        (Some(InFlight::Export), WorkerResponse::Unit(r)) => export::handle(app, r),
        (Some(InFlight::Import), WorkerResponse::Unit(r)) => import::handle(app, r),
        (Some(InFlight::ImportReloadItems), WorkerResponse::Items(r)) => {
            import::handle_reload_items(app, r)
        }
        (Some(InFlight::ImportReloadFolders), WorkerResponse::Folders(r)) => {
            import::handle_reload_folders(app, r)
        }

        // ── Send ──────────────────────────────────────────────────────
        (Some(InFlight::SendText), WorkerResponse::SendUrl(r)) => send::handle(app, r),

        // ── Assign collections / move ─────────────────────────────────
        (Some(InFlight::MoveItem), WorkerResponse::Unit(r)) => {
            assign_collections::handle_move(app, r)
        }
        (Some(InFlight::MoveReloadItems), WorkerResponse::Items(r)) => {
            assign_collections::handle_move_reload(app, r)
        }

        // ── Memberships ───────────────────────────────────────────────
        (Some(InFlight::MembershipsOrgs), WorkerResponse::Orgs(r)) => {
            memberships::handle_orgs(app, r)
        }
        (Some(InFlight::MembershipsCollections), WorkerResponse::Collections(r)) => {
            memberships::handle_collections(app, r)
        }

        // ── Reprompt ──────────────────────────────────────────────────
        (Some(InFlight::RepromptUnlock), WorkerResponse::SessionKey(r)) => {
            reprompt::handle_unlock(app, r)
        }

        // ── Generator ─────────────────────────────────────────────────
        (Some(InFlight::Generate), WorkerResponse::Generated(r)) => generator::handle(app, r),

        // ── Programming-error surface ─────────────────────────────────
        (other, _resp) => {
            app.push_cmd(
                "<worker>",
                false,
                &format!("unmatched worker response for ticket {other:?}"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::worker::WorkerResponse;

    #[test]
    fn lock_is_fire_and_forget() {
        assert!(is_fire_and_forget(&WorkerResponse::Locked));
    }

    #[test]
    fn ticketed_responses_are_not_fire_and_forget() {
        assert!(!is_fire_and_forget(&WorkerResponse::Items(Ok(Vec::new()))));
        assert!(!is_fire_and_forget(&WorkerResponse::Unit(Ok(()))));
        assert!(!is_fire_and_forget(&WorkerResponse::SessionKey(Ok(
            String::new()
        ))));
    }
}
