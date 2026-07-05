//! Worker thread that owns the vault + generator ports and serves
//! requests serially over `mpsc`.
//!
//! ## Why a worker
//!
//! Every call into the `bw` CLI is a synchronous subprocess that can
//! take from ~200 ms (Node cold-start on a local read) to tens of
//! seconds (`sync` / `login` over the network). Running it on the
//! render thread froze the UI for the whole duration — spinner static,
//! keys queued, `Ctrl+C` delayed. The worker thread keeps the render
//! loop responsive:
//!
//! * The render thread builds a [`WorkerRequest`], stashes a
//!   [`InFlight`] ticket on [`crate::tui::App`], sends the request, and
//!   immediately continues redrawing. The spinner ticks while it runs.
//! * The worker pulls one request at a time, calls into the port(s),
//!   ships a [`WorkerResponse`] back.
//! * The render thread drains the response channel between frames and
//!   routes each response via [`crate::tui::flows::apply_response`].
//!
//! Serial-by-construction: only one user request is in flight at a time
//! (`App::in_flight` is an `Option`, not a `Vec`), and input is gated
//! while it is `Some` (see `input::is_busy_blocked`). Multi-step flows
//! (login → load → session-data, save = fetch → edit, …) chain by having
//! a response handler queue the next request.
//!
//! The clipboard and settings ports stay synchronous on the render
//! thread — they're fast and don't warrant the round-trip.

use std::panic::AssertUnwindSafe;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use zeroize::Zeroizing;

use crate::domain::vault_info::LoginOutcome;
use crate::domain::{Collection, Folder, Item, Organization, TwoFactorMethod, VaultInfo};
use crate::ports::{
    BwError, GeneratorOptions, ParallelSessionData, PasswordGeneratorPort, VaultPort,
};

/// Catches a panic inside a port call and turns it into an `Err` string
/// so one bad call can't take down the worker thread.
fn run_caught<T>(f: impl FnOnce() -> Result<T, BwError>) -> Result<T, BwError> {
    match std::panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(payload) => Err(BwError::Internal(panic_payload_to_string(payload))),
    }
}

fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        return (*s).to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "<unknown panic payload>".to_string()
}

/// One unit of work for the worker. Each variant owns its arguments so
/// nothing borrows from `App` across the thread boundary.
pub enum WorkerRequest {
    /// `bw status`.
    Status,
    /// `bw login` (fresh master-password login).
    Login {
        email: String,
        password: Zeroizing<String>,
    },
    /// `bw login` resuming a new-device verification with the e-mailed code.
    LoginOtp {
        email: String,
        password: Zeroizing<String>,
        otp: Zeroizing<String>,
    },
    /// `bw login --method N` resuming a permanent-2FA challenge.
    LoginTwoFactor {
        email: String,
        password: Zeroizing<String>,
        code: Zeroizing<String>,
        method: TwoFactorMethod,
    },
    /// `bw login --apikey` (leaves the vault Locked).
    LoginApiKey,
    /// `bw login --sso` (leaves the vault Locked).
    LoginSso,
    /// `bw unlock`.
    Unlock { password: Zeroizing<String> },
    /// `bw lock` — fire-and-forget (the render thread resets UI state
    /// immediately; this just drops the worker's session key).
    Lock,
    /// `bw logout`.
    Logout,
    /// `bw config server <url>`.
    SetServer { url: String },
    /// `bw list items`.
    ListItems,
    /// `bw list items --trash`.
    ListTrash,
    /// `bw sync`.
    Sync,
    /// `bw get totp <id>`.
    GetTotp { item_id: String },
    /// `bw get item <id>` (raw JSON, base for patching).
    GetItemJson { item_id: String },
    /// HaveIBeenPwned breach check for an item's password.
    CheckExposed { item_id: String },
    /// `bw create item`.
    CreateItem { json: Zeroizing<String> },
    /// `bw edit item`.
    EditItem {
        item_id: String,
        json: Zeroizing<String>,
    },
    /// `bw delete item` (trash unless `permanent`).
    DeleteItem { item_id: String, permanent: bool },
    /// `bw restore item`.
    RestoreItem { item_id: String },
    /// `bw list folders`.
    ListFolders,
    /// `bw create folder`.
    CreateFolder { name: String },
    /// `bw edit folder`.
    EditFolder { folder_id: String, name: String },
    /// `bw delete folder`.
    DeleteFolder { folder_id: String },
    /// `bw export`.
    Export { format: String, path: String },
    /// `bw import`.
    Import { format: String, path: String },
    /// `bw get fingerprint me`.
    GetFingerprint,
    /// `bw move <id> <org> <collections>`.
    MoveItem {
        item_id: String,
        organization_id: String,
        collection_ids: Vec<String>,
    },
    /// `bw create attachment`.
    UploadAttachment { item_id: String, file_path: String },
    /// `bw get attachment`.
    DownloadAttachment {
        item_id: String,
        file_name: String,
        output_path: String,
    },
    /// `bw delete attachment`.
    DeleteAttachment {
        item_id: String,
        attachment_id: String,
    },
    /// `bw send create` (text).
    SendText {
        name: String,
        days: u8,
        content: String,
    },
    /// `bw list organizations`.
    ListOrganizations,
    /// `bw list collections`.
    ListCollections,
    /// The four post-auth reads (folders/orgs/collections/import-formats)
    /// in one trip — the adapter parallelises them internally.
    ParallelSessionData,
    /// `bw generate`.
    Generate { opts: GeneratorOptions },
    /// Terminates the worker. Sent on drop of [`WorkerHandle`].
    Shutdown,
}

/// Result envelope for one [`WorkerRequest`]. Several mutating calls that
/// only return success/failure share the [`WorkerResponse::Unit`]
/// envelope; the [`InFlight`] ticket disambiguates which flow they
/// belong to.
pub enum WorkerResponse {
    Status(Result<VaultInfo, BwError>),
    Login(LoginOutcome),
    /// unlock / login-otp / login-2fa → the new session key.
    SessionKey(Result<String, BwError>),
    /// api-key / sso login (vault left Locked, no key yet).
    LoginLocked(Result<(), BwError>),
    Logout(Result<(), BwError>),
    SetServer(Result<(), BwError>),
    Items(Result<Vec<Item>, BwError>),
    Trash(Result<Vec<Item>, BwError>),
    /// Shared envelope for mutating calls returning `()` — sync, delete
    /// item, restore item, delete folder, export, import, move item,
    /// delete/download attachment. Routed by [`InFlight`].
    Unit(Result<(), BwError>),
    Totp(Result<String, BwError>),
    ItemJson(Result<Zeroizing<String>, BwError>),
    Exposed(Result<u32, BwError>),
    /// create_item / edit_item / upload_attachment → the updated item.
    /// Boxed: `Item` is ~900 bytes and would bloat every response moved
    /// across the channel (`clippy::large_enum_variant`). The dispatcher
    /// unboxes before calling the handler.
    Item(Result<Box<Item>, BwError>),
    /// create_folder / edit_folder → the folder.
    Folder(Result<Folder, BwError>),
    Folders(Result<Vec<Folder>, BwError>),
    Orgs(Result<Vec<Organization>, BwError>),
    Collections(Result<Vec<Collection>, BwError>),
    Fingerprint(Result<String, BwError>),
    SendUrl(Result<String, BwError>),
    SessionData(ParallelSessionData),
    Generated(Result<String, BwError>),
    /// `bw lock` finished — fire-and-forget, routed by variant and ignored.
    Locked,
}

/// Caller-side context for an in-flight request — stored on
/// [`crate::tui::App::in_flight`] and consumed when the matching
/// response arrives. Multi-step flows chain by queueing the next
/// request (with a fresh ticket) from the response handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InFlight {
    /// Boot: `bw status`.
    BootStatus,
    /// Boot resume: list items with the seeded session key.
    ResumeItems,
    /// Boot resume: post-list parallel session data.
    ResumeSessionData,
    /// Fresh master-password login.
    Login,
    /// Unlock an already-authenticated (Locked) vault.
    Unlock,
    /// Resume a new-device verification with the e-mailed code.
    LoginOtp,
    /// Resume a permanent-2FA challenge.
    LoginTwoFactor,
    /// `bw login --apikey`.
    LoginApiKey,
    /// `bw login --sso`.
    LoginSso,
    /// Post-login: load items.
    PostLoginItems,
    /// Post-login: parallel session data.
    PostLoginSessionData,
    /// Log out of the account.
    Logout,
    /// Persist a server-URL change.
    SetServer,
    /// Fetch the fingerprint phrase.
    Fingerprint,
    /// User-initiated vault-list load (spinner → Idle).
    LoadItems,
    /// Silent vault-list refresh (preserves the prior toast).
    ReloadItemsSilent,
    /// Trash-list load.
    LoadTrash,
    /// Vault sync.
    Sync,
    /// Post-sync silent item reload.
    SyncReload,
    /// Create a new item.
    CreateItem,
    /// Save edit — step 1: fetch the item JSON to patch.
    SaveEditFetch,
    /// Save edit — step 2: commit the patched JSON.
    SaveEditCommit,
    /// Toggle favorite — step 1: fetch the item JSON.
    ToggleFavoriteFetch { item_id: String },
    /// Toggle favorite — step 2: commit the flipped JSON.
    ToggleFavoriteCommit { new_favorite: bool },
    /// Delete (trash or permanent) an item.
    DeleteItem {
        permanent: bool,
        item_id: String,
        name: String,
    },
    /// Post-delete silent trash reload.
    DeleteReloadTrash,
    /// Restore a trashed item.
    RestoreItem { item_id: String, name: String },
    /// Post-restore silent item reload.
    RestoreReloadItems,
    /// HIBP exposed-password check.
    CheckExposed,
    /// Download an attachment.
    DownloadAttachment,
    /// Delete an attachment — step 1: the delete call.
    DeleteAttachment,
    /// Delete an attachment — step 2: refetch the item JSON to refresh it.
    DeleteAttachmentRefresh { item_id: String },
    /// Upload an attachment.
    UploadAttachment,
    /// Copy a TOTP code (fetch → clipboard in the handler).
    CopyTotp,
    /// Create a folder.
    CreateFolder,
    /// Rename a folder.
    EditFolder,
    /// Delete a folder (`name` carried for the success toast).
    DeleteFolder { name: String },
    /// Silent folder reload after a folder mutation.
    FolderReload,
    /// Silent item reload after a folder delete (items lost their folder).
    FolderDeleteReloadItems,
    /// Export the vault.
    Export,
    /// Import into the vault.
    Import,
    /// Post-import silent item reload.
    ImportReloadItems,
    /// Post-import silent folder reload.
    ImportReloadFolders,
    /// Create a text Send (URL copied in the handler).
    SendText,
    /// Move an item into an organisation's collections.
    MoveItem,
    /// Post-move silent item reload.
    MoveReloadItems,
    /// Memberships popup — step 1: list organisations.
    MembershipsOrgs,
    /// Memberships popup — step 2: list collections.
    MembershipsCollections,
    /// Reprompt master-password reverify.
    RepromptUnlock,
    /// Generate a password / passphrase.
    Generate,
}

/// Owning handle to the worker thread.
pub struct WorkerHandle {
    tx: Option<Sender<WorkerRequest>>,
    rx: Option<Receiver<WorkerResponse>>,
    join: Option<JoinHandle<()>>,
}

impl WorkerHandle {
    /// Spawns the worker, moving the vault + generator ports onto it.
    pub fn spawn(
        mut vault: Box<dyn VaultPort + Send>,
        generator: Box<dyn PasswordGeneratorPort + Send>,
    ) -> Self {
        let (req_tx, req_rx) = channel::<WorkerRequest>();
        let (resp_tx, resp_rx) = channel::<WorkerResponse>();
        let join = std::thread::spawn(move || {
            run_worker(&mut *vault, &*generator, req_rx, resp_tx);
        });
        Self {
            tx: Some(req_tx),
            rx: Some(resp_rx),
            join: Some(join),
        }
    }

    pub fn tx(&self) -> Sender<WorkerRequest> {
        self.tx.as_ref().expect("worker tx already taken").clone()
    }

    pub fn take_rx(&mut self) -> Receiver<WorkerResponse> {
        self.rx.take().expect("worker rx already taken")
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(WorkerRequest::Shutdown);
            drop(tx);
        }
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn run_worker(
    vault: &mut dyn VaultPort,
    generator: &dyn PasswordGeneratorPort,
    req_rx: Receiver<WorkerRequest>,
    resp_tx: Sender<WorkerResponse>,
) {
    while let Ok(req) = req_rx.recv() {
        let resp = match req {
            WorkerRequest::Shutdown => break,
            WorkerRequest::Status => WorkerResponse::Status(run_caught(|| vault.status())),
            WorkerRequest::Login { email, password } => {
                // `login` returns a non-`Result` outcome; guard it
                // separately so a panic becomes a Failed outcome.
                let outcome = match std::panic::catch_unwind(AssertUnwindSafe(|| {
                    vault.login(&email, &password)
                })) {
                    Ok(o) => o,
                    Err(p) => LoginOutcome::Failed(panic_payload_to_string(p)),
                };
                WorkerResponse::Login(outcome)
            }
            WorkerRequest::LoginOtp {
                email,
                password,
                otp,
            } => WorkerResponse::SessionKey(run_caught(|| {
                vault.login_with_otp(&email, &password, &otp)
            })),
            WorkerRequest::LoginTwoFactor {
                email,
                password,
                code,
                method,
            } => WorkerResponse::SessionKey(run_caught(|| {
                vault.login_with_two_factor(&email, &password, &code, method)
            })),
            WorkerRequest::LoginApiKey => {
                WorkerResponse::LoginLocked(run_caught(|| vault.login_with_api_key()))
            }
            WorkerRequest::LoginSso => {
                WorkerResponse::LoginLocked(run_caught(|| vault.login_with_sso()))
            }
            WorkerRequest::Unlock { password } => {
                WorkerResponse::SessionKey(run_caught(|| vault.unlock(&password)))
            }
            WorkerRequest::Lock => {
                let _ = std::panic::catch_unwind(AssertUnwindSafe(|| vault.lock()));
                WorkerResponse::Locked
            }
            WorkerRequest::Logout => WorkerResponse::Logout(run_caught(|| vault.logout())),
            WorkerRequest::SetServer { url } => {
                WorkerResponse::SetServer(run_caught(|| vault.set_server(&url)))
            }
            WorkerRequest::ListItems => WorkerResponse::Items(run_caught(|| vault.list_items())),
            WorkerRequest::ListTrash => WorkerResponse::Trash(run_caught(|| vault.list_trash())),
            WorkerRequest::Sync => WorkerResponse::Unit(run_caught(|| vault.sync())),
            WorkerRequest::GetTotp { item_id } => {
                WorkerResponse::Totp(run_caught(|| vault.get_totp(&item_id)))
            }
            WorkerRequest::GetItemJson { item_id } => {
                WorkerResponse::ItemJson(run_caught(|| vault.get_item_json(&item_id)))
            }
            WorkerRequest::CheckExposed { item_id } => {
                WorkerResponse::Exposed(run_caught(|| vault.check_exposed(&item_id)))
            }
            WorkerRequest::CreateItem { json } => {
                WorkerResponse::Item(run_caught(|| vault.create_item(&json)).map(Box::new))
            }
            WorkerRequest::EditItem { item_id, json } => {
                WorkerResponse::Item(run_caught(|| vault.edit_item(&item_id, &json)).map(Box::new))
            }
            WorkerRequest::DeleteItem { item_id, permanent } => {
                WorkerResponse::Unit(run_caught(|| vault.delete_item(&item_id, permanent)))
            }
            WorkerRequest::RestoreItem { item_id } => {
                WorkerResponse::Unit(run_caught(|| vault.restore_item(&item_id)))
            }
            WorkerRequest::ListFolders => {
                WorkerResponse::Folders(run_caught(|| vault.list_folders()))
            }
            WorkerRequest::CreateFolder { name } => {
                WorkerResponse::Folder(run_caught(|| vault.create_folder(&name)))
            }
            WorkerRequest::EditFolder { folder_id, name } => {
                WorkerResponse::Folder(run_caught(|| vault.edit_folder(&folder_id, &name)))
            }
            WorkerRequest::DeleteFolder { folder_id } => {
                WorkerResponse::Unit(run_caught(|| vault.delete_folder(&folder_id)))
            }
            WorkerRequest::Export { format, path } => {
                WorkerResponse::Unit(run_caught(|| vault.export(&format, &path)))
            }
            WorkerRequest::Import { format, path } => {
                WorkerResponse::Unit(run_caught(|| vault.import(&format, &path)))
            }
            WorkerRequest::GetFingerprint => {
                WorkerResponse::Fingerprint(run_caught(|| vault.get_fingerprint()))
            }
            WorkerRequest::MoveItem {
                item_id,
                organization_id,
                collection_ids,
            } => WorkerResponse::Unit(run_caught(|| {
                vault.move_item(&item_id, &organization_id, &collection_ids)
            })),
            WorkerRequest::UploadAttachment { item_id, file_path } => WorkerResponse::Item(
                run_caught(|| vault.upload_attachment(&item_id, &file_path)).map(Box::new),
            ),
            WorkerRequest::DownloadAttachment {
                item_id,
                file_name,
                output_path,
            } => WorkerResponse::Unit(run_caught(|| {
                vault.download_attachment(&item_id, &file_name, &output_path)
            })),
            WorkerRequest::DeleteAttachment {
                item_id,
                attachment_id,
            } => WorkerResponse::Unit(run_caught(|| {
                vault.delete_attachment(&item_id, &attachment_id)
            })),
            WorkerRequest::SendText {
                name,
                days,
                content,
            } => WorkerResponse::SendUrl(run_caught(|| vault.send_text(&name, days, &content))),
            WorkerRequest::ListOrganizations => {
                WorkerResponse::Orgs(run_caught(|| vault.list_organizations()))
            }
            WorkerRequest::ListCollections => {
                WorkerResponse::Collections(run_caught(|| vault.list_collections()))
            }
            WorkerRequest::ParallelSessionData => {
                // `parallel_session_data` returns its own bundle of
                // `Result`s; on a panic synthesise an all-failed bundle.
                let data = match std::panic::catch_unwind(AssertUnwindSafe(|| {
                    vault.parallel_session_data()
                })) {
                    Ok(d) => d,
                    Err(p) => {
                        let msg = panic_payload_to_string(p);
                        ParallelSessionData {
                            folders: Err(BwError::Internal(msg.clone())),
                            organizations: Err(BwError::Internal(msg.clone())),
                            collections: Err(BwError::Internal(msg.clone())),
                            import_formats: Err(BwError::Internal(msg)),
                        }
                    }
                };
                WorkerResponse::SessionData(data)
            }
            WorkerRequest::Generate { opts } => {
                WorkerResponse::Generated(run_caught(|| generator.generate(&opts)))
            }
        };
        if resp_tx.send(resp).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::vault_info::VaultStatus;
    use crate::domain::{Collection, Folder, Item, Organization, VaultInfo};
    use crate::ports::ParallelSessionData;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Port that panics on the first `status` call and returns Ok
    /// thereafter. Every other method is a no-op.
    #[derive(Default)]
    struct PanicOnce {
        n: Arc<AtomicUsize>,
    }

    impl VaultPort for PanicOnce {
        fn status(&mut self) -> Result<VaultInfo, BwError> {
            let prev = self.n.fetch_add(1, Ordering::SeqCst);
            if prev == 0 {
                panic!("boom");
            }
            Ok(VaultInfo {
                status: VaultStatus::Unauthenticated,
                user_email: None,
                last_sync: None,
                server_url: None,
            })
        }
        fn login(&mut self, _: &str, _: &str) -> LoginOutcome {
            LoginOutcome::Failed("x".into())
        }
        fn login_with_otp(&mut self, _: &str, _: &str, _: &str) -> Result<String, BwError> {
            Ok(String::new())
        }
        fn login_with_two_factor(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: TwoFactorMethod,
        ) -> Result<String, BwError> {
            Ok(String::new())
        }
        fn login_with_api_key(&mut self) -> Result<(), BwError> {
            Ok(())
        }
        fn login_with_sso(&mut self) -> Result<(), BwError> {
            Ok(())
        }
        fn unlock(&mut self, _: &str) -> Result<String, BwError> {
            Ok(String::new())
        }
        fn lock(&mut self) {}
        fn logout(&mut self) -> Result<(), BwError> {
            Ok(())
        }
        fn session_key(&self) -> Option<&str> {
            None
        }
        fn set_server(&mut self, _: &str) -> Result<(), BwError> {
            Ok(())
        }
        fn list_items(&mut self) -> Result<Vec<Item>, BwError> {
            Ok(Vec::new())
        }
        fn list_trash(&mut self) -> Result<Vec<Item>, BwError> {
            Ok(Vec::new())
        }
        fn sync(&mut self) -> Result<(), BwError> {
            Ok(())
        }
        fn get_totp(&mut self, _: &str) -> Result<String, BwError> {
            Ok(String::new())
        }
        fn get_item_json(&mut self, _: &str) -> Result<Zeroizing<String>, BwError> {
            Ok(Zeroizing::new(String::new()))
        }
        fn check_exposed(&mut self, _: &str) -> Result<u32, BwError> {
            Ok(0)
        }
        fn create_item(&mut self, _: &str) -> Result<Item, BwError> {
            Err(BwError::Internal("no".into()))
        }
        fn edit_item(&mut self, _: &str, _: &str) -> Result<Item, BwError> {
            Err(BwError::Internal("no".into()))
        }
        fn delete_item(&mut self, _: &str, _: bool) -> Result<(), BwError> {
            Ok(())
        }
        fn restore_item(&mut self, _: &str) -> Result<(), BwError> {
            Ok(())
        }
        fn list_folders(&mut self) -> Result<Vec<Folder>, BwError> {
            Ok(Vec::new())
        }
        fn create_folder(&mut self, _: &str) -> Result<Folder, BwError> {
            Err(BwError::Internal("no".into()))
        }
        fn edit_folder(&mut self, _: &str, _: &str) -> Result<Folder, BwError> {
            Err(BwError::Internal("no".into()))
        }
        fn delete_folder(&mut self, _: &str) -> Result<(), BwError> {
            Ok(())
        }
        fn export(&mut self, _: &str, _: &str) -> Result<(), BwError> {
            Ok(())
        }
        fn get_fingerprint(&mut self) -> Result<String, BwError> {
            Ok(String::new())
        }
        fn import(&mut self, _: &str, _: &str) -> Result<(), BwError> {
            Ok(())
        }
        fn list_import_formats(&mut self) -> Result<Vec<String>, BwError> {
            Ok(Vec::new())
        }
        fn move_item(&mut self, _: &str, _: &str, _: &[String]) -> Result<(), BwError> {
            Ok(())
        }
        fn upload_attachment(&mut self, _: &str, _: &str) -> Result<Item, BwError> {
            Err(BwError::Internal("no".into()))
        }
        fn download_attachment(&mut self, _: &str, _: &str, _: &str) -> Result<(), BwError> {
            Ok(())
        }
        fn delete_attachment(&mut self, _: &str, _: &str) -> Result<(), BwError> {
            Ok(())
        }
        fn send_text(&mut self, _: &str, _: u8, _: &str) -> Result<String, BwError> {
            Ok(String::new())
        }
        fn list_organizations(&mut self) -> Result<Vec<Organization>, BwError> {
            Ok(Vec::new())
        }
        fn list_collections(&mut self) -> Result<Vec<Collection>, BwError> {
            Ok(Vec::new())
        }
        fn parallel_session_data(&mut self) -> ParallelSessionData {
            ParallelSessionData {
                folders: Ok(Vec::new()),
                organizations: Ok(Vec::new()),
                collections: Ok(Vec::new()),
                import_formats: Ok(Vec::new()),
            }
        }
    }

    struct NoopGen;
    impl PasswordGeneratorPort for NoopGen {
        fn generate(&self, _: &GeneratorOptions) -> Result<String, BwError> {
            Ok(String::new())
        }
    }

    #[test]
    fn worker_survives_panic_and_serves_next_request() {
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        let port = PanicOnce::default();
        let counter = port.n.clone();
        let mut h = WorkerHandle::spawn(Box::new(port), Box::new(NoopGen));
        let tx = h.tx();
        let rx = h.take_rx();

        tx.send(WorkerRequest::Status).unwrap();
        match rx.recv().unwrap() {
            WorkerResponse::Status(Err(BwError::Internal(msg))) => assert!(msg.contains("boom")),
            _ => panic!("expected Err on first call"),
        }

        tx.send(WorkerRequest::Status).unwrap();
        match rx.recv().unwrap() {
            WorkerResponse::Status(Ok(_)) => {}
            _ => panic!("expected Ok on second call"),
        }
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        drop(h);
        std::panic::set_hook(prev_hook);
    }
}
