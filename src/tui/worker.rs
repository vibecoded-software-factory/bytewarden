use std::panic::AssertUnwindSafe;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use zeroize::Zeroizing;

use crate::domain::vault_info::LoginOutcome;
use crate::domain::{Collection, Folder, Item, Organization, TwoFactorMethod, VaultInfo};
use crate::ports::{
    BwError, GeneratorOptions, ParallelSessionData, PasswordGeneratorPort, VaultPort,
};

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

pub enum WorkerRequest {
    Status,

    Login {
        email: String,
        password: Zeroizing<String>,
    },

    LoginOtp {
        email: String,
        password: Zeroizing<String>,
        otp: Zeroizing<String>,
    },

    LoginTwoFactor {
        email: String,
        password: Zeroizing<String>,
        code: Zeroizing<String>,
        method: TwoFactorMethod,
    },

    LoginApiKey,

    LoginSso,

    Unlock {
        password: Zeroizing<String>,
    },

    Lock,

    Logout,

    SetServer {
        url: String,
    },

    ListItems,

    ListTrash,

    Sync,

    GetTotp {
        item_id: String,
    },

    GetItemJson {
        item_id: String,
    },

    CheckExposed {
        item_id: String,
    },

    CreateItem {
        json: Zeroizing<String>,
    },

    EditItem {
        item_id: String,
        json: Zeroizing<String>,
    },

    DeleteItem {
        item_id: String,
        permanent: bool,
    },

    RestoreItem {
        item_id: String,
    },

    ListFolders,

    CreateFolder {
        name: String,
    },

    EditFolder {
        folder_id: String,
        name: String,
    },

    DeleteFolder {
        folder_id: String,
    },

    Export {
        format: String,
        path: String,
    },

    Import {
        format: String,
        path: String,
    },

    GetFingerprint,

    MoveItem {
        item_id: String,
        organization_id: String,
        collection_ids: Vec<String>,
    },

    UploadAttachment {
        item_id: String,
        file_path: String,
    },

    DownloadAttachment {
        item_id: String,
        file_name: String,
        output_path: String,
    },

    DeleteAttachment {
        item_id: String,
        attachment_id: String,
    },

    SendText {
        name: String,
        days: u8,
        content: String,
    },

    ListOrganizations,

    ListCollections,

    ParallelSessionData,

    Generate {
        opts: GeneratorOptions,
    },

    Shutdown,
}

pub enum WorkerResponse {
    Status(Result<VaultInfo, BwError>),
    Login(LoginOutcome),

    SessionKey(Result<String, BwError>),

    LoginLocked(Result<(), BwError>),
    Logout(Result<(), BwError>),
    SetServer(Result<(), BwError>),
    Items(Result<Vec<Item>, BwError>),
    Trash(Result<Vec<Item>, BwError>),

    Unit(Result<(), BwError>),
    Totp(Result<String, BwError>),
    ItemJson(Result<Zeroizing<String>, BwError>),
    Exposed(Result<u32, BwError>),

    Item(Result<Box<Item>, BwError>),

    Folder(Result<Folder, BwError>),
    Folders(Result<Vec<Folder>, BwError>),
    Orgs(Result<Vec<Organization>, BwError>),
    Collections(Result<Vec<Collection>, BwError>),
    Fingerprint(Result<String, BwError>),
    SendUrl(Result<String, BwError>),
    SessionData(ParallelSessionData),
    Generated(Result<String, BwError>),

    Locked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InFlight {
    BootStatus,

    ResumeItems,

    ResumeSessionData,

    Login,

    Unlock,

    LoginOtp,

    LoginTwoFactor,

    LoginApiKey,

    LoginSso,

    PostLoginItems,

    PostLoginSessionData,

    Logout,

    SetServer,

    Fingerprint,

    LoadItems,

    ReloadItemsSilent,

    LoadTrash,

    Sync,

    SyncReload,

    CreateItem,

    SaveEditFetch,

    SaveEditCommit,

    ToggleFavoriteFetch {
        item_id: String,
    },

    ToggleFavoriteCommit {
        new_favorite: bool,
    },

    DeleteItem {
        permanent: bool,
        item_id: String,
        name: String,
    },

    DeleteReloadTrash,

    RestoreItem {
        item_id: String,
        name: String,
    },

    RestoreReloadItems,

    CheckExposed,

    DownloadAttachment,

    DeleteAttachment,

    DeleteAttachmentRefresh {
        item_id: String,
    },

    UploadAttachment,

    CopyTotp,

    CreateFolder,

    EditFolder,

    DeleteFolder {
        name: String,
    },

    FolderReload,

    FolderDeleteReloadItems,

    Export,

    Import,

    ImportReloadItems,

    ImportReloadFolders,

    SendText,

    MoveItem,

    MoveReloadItems,

    MembershipsOrgs,

    MembershipsCollections,

    RepromptUnlock,

    Generate,
}

pub struct WorkerHandle {
    tx: Option<Sender<WorkerRequest>>,
    rx: Option<Receiver<WorkerResponse>>,
    join: Option<JoinHandle<()>>,
}

impl WorkerHandle {
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
        let _guard = crate::tui::PANIC_HOOK_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
