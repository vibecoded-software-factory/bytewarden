//! Vault backend port.

use super::BwError;
use crate::domain::{
    Collection, Folder, Item, LoginOutcome, Organization, TwoFactorMethod, VaultInfo,
};

/// Bundle returned by [`VaultPort::parallel_session_data`]: the four
/// secondary reads the TUI fires immediately after a successful login
/// or session resume, all carrying their own `Result` so a partial
/// failure does not poison the whole load.
pub struct ParallelSessionData {
    pub folders: Result<Vec<Folder>, BwError>,
    pub organizations: Result<Vec<Organization>, BwError>,
    pub collections: Result<Vec<Collection>, BwError>,
    pub import_formats: Result<Vec<String>, BwError>,
}

/// Abstraction over the password-vault backend.
///
/// The current concrete implementation is
/// [`crate::adapters::bw_cli::BwCliAdapter`], which shells out to the
/// Bitwarden CLI. Tests can plug in a fake implementation.
///
/// All methods take `&mut self` (rather than the more granular `&self` for
/// reads) because every operation may mutate session state under the hood
/// — and a uniform mutability boundary keeps the trait object signature
/// simple for the call sites.
///
/// Errors are returned as `String` for now to keep the surface terse;
/// they are treated as opaque by the application layer and surfaced in
/// the TUI feedback strip.
pub trait VaultPort {
    // ── Authentication ────────────────────────────────────────────────────

    /// Returns a snapshot of the current authentication state.
    fn status(&mut self) -> Result<VaultInfo, BwError>;

    /// First-time login. May return [`LoginOutcome::NeedsOtp`] if the
    /// backend triggered new-device verification.
    fn login(&mut self, email: &str, password: &str) -> LoginOutcome;

    /// First-time login supplying the device-verification OTP.
    /// Returns the session key on success.
    fn login_with_otp(&mut self, email: &str, password: &str, otp: &str)
    -> Result<String, BwError>;

    /// Login when the account has a permanent second factor enrolled.
    ///
    /// `method` selects which factor to validate (Authenticator,
    /// Email or YubiKey — see [`TwoFactorMethod`] for the full set).
    /// `code` is the user-supplied secret for that method (six-digit
    /// TOTP, e-mailed code, or YubiKey OTP press).
    ///
    /// Returns the session key on success.
    fn login_with_two_factor(
        &mut self,
        email: &str,
        password: &str,
        code: &str,
        method: TwoFactorMethod,
    ) -> Result<String, BwError>;

    /// Headless login using a personal API key.
    ///
    /// `bw login --apikey` reads `BW_CLIENTID` and `BW_CLIENTSECRET`
    /// from the environment of the parent process. After this
    /// succeeds the vault is *Locked* (not Unlocked) — the caller
    /// still needs to call [`Self::unlock`] with the master password
    /// before vault data is accessible.
    fn login_with_api_key(&mut self) -> Result<(), BwError>;

    /// Login via Single-Sign-On.
    ///
    /// `bw login --sso` opens the user's default browser for the
    /// federated authentication exchange and blocks until the
    /// callback arrives. Same post-conditions as
    /// [`Self::login_with_api_key`]: the vault is Locked afterwards
    /// and needs an explicit unlock with the master password.
    fn login_with_sso(&mut self) -> Result<(), BwError>;

    /// Unlocks an existing locked session. Returns the session key on success.
    fn unlock(&mut self, password: &str) -> Result<String, BwError>;

    /// Locks the vault — purges the session key from memory.
    fn lock(&mut self);

    /// Logs out of the current account, removing it from the local
    /// `bw` CLI state. Distinct from [`Self::lock`]: lock keeps the
    /// account configured (only the session key is dropped); logout
    /// removes the account and the next launch starts at email entry.
    fn logout(&mut self) -> Result<(), BwError>;

    /// Returns the current session key if the vault is unlocked.
    fn session_key(&self) -> Option<&str>;

    // ── Configuration ─────────────────────────────────────────────────────

    /// Configures the Bitwarden server URL (`bw config server <url>`).
    ///
    /// `url` may be a fully-qualified URL (`https://vault.example.com`)
    /// or a known hostname (`bitwarden.com`, `bitwarden.eu`). Per the
    /// CLI, this is only valid when the vault is *unauthenticated* —
    /// the caller is responsible for logging out first.
    fn set_server(&mut self, url: &str) -> Result<(), BwError>;

    // ── Vault data ────────────────────────────────────────────────────────

    /// Lists all (non-trashed) items.
    fn list_items(&mut self) -> Result<Vec<Item>, BwError>;

    /// Lists trashed items only.
    fn list_trash(&mut self) -> Result<Vec<Item>, BwError>;

    /// Synchronizes the local cache with the remote server.
    fn sync(&mut self) -> Result<(), BwError>;

    // ── Single-field reads ────────────────────────────────────────────────

    /// Generates the current TOTP code for a login item.
    fn get_totp(&mut self, item_id: &str) -> Result<String, BwError>;

    /// Returns the raw JSON for a single item — used as the base for
    /// edit patching.
    ///
    /// The buffer carries plaintext credentials (passwords, TOTP
    /// seeds, private keys, …) so it is wrapped in
    /// [`zeroize::Zeroizing`]. Callers receive a value that is
    /// transparently `Deref<Target=String>` — `.as_str()`,
    /// `serde_json::from_str(&buf)` etc. all work — and the underlying
    /// allocation is overwritten with zeroes when the wrapper goes
    /// out of scope.
    fn get_item_json(&mut self, item_id: &str) -> Result<zeroize::Zeroizing<String>, BwError>;

    /// Checks whether the password of a login item appears in known
    /// breach datasets (HaveIBeenPwned). Returns the number of times
    /// the password has been seen — `0` means "safe so far".
    ///
    /// # Errors
    ///
    /// Returns an error string if the item is not a login (or has no
    /// password), or if the network call to HIBP fails.
    fn check_exposed(&mut self, item_id: &str) -> Result<u32, BwError>;

    // ── Item CRUD ─────────────────────────────────────────────────────────

    /// Creates a new item from a JSON string.
    fn create_item(&mut self, item_json: &str) -> Result<Item, BwError>;

    /// Replaces an existing item with the JSON payload.
    fn edit_item(&mut self, item_id: &str, item_json: &str) -> Result<Item, BwError>;

    /// Deletes an item — `permanent = false` moves it to trash.
    fn delete_item(&mut self, item_id: &str, permanent: bool) -> Result<(), BwError>;

    /// Restores a trashed item back to the vault.
    fn restore_item(&mut self, item_id: &str) -> Result<(), BwError>;

    // ── Folder CRUD ───────────────────────────────────────────────────────

    /// Lists every folder visible in the current session.
    fn list_folders(&mut self) -> Result<Vec<Folder>, BwError>;

    /// Creates a new folder with the given name.
    fn create_folder(&mut self, name: &str) -> Result<Folder, BwError>;

    /// Renames an existing folder.
    fn edit_folder(&mut self, folder_id: &str, name: &str) -> Result<Folder, BwError>;

    /// Deletes a folder. Items previously assigned to it have their
    /// `folder_id` cleared by `bw` (they are not deleted).
    fn delete_folder(&mut self, folder_id: &str) -> Result<(), BwError>;

    // ── Export ────────────────────────────────────────────────────────────

    /// Exports the vault to a file at `output_path`.
    ///
    /// `format` is one of the values accepted by `bw export --format`
    /// (currently `"csv"`, `"json"`, `"encrypted_json"`). The file
    /// contains plaintext credentials for the unencrypted formats —
    /// the caller is responsible for choosing a safe destination.
    fn export(&mut self, format: &str, output_path: &str) -> Result<(), BwError>;

    /// Returns the fingerprint phrase for the current user (a stable
    /// 4–5 word identifier derived from their public key). Used to
    /// verify identity out-of-band when receiving sharing invitations.
    fn get_fingerprint(&mut self) -> Result<String, BwError>;

    /// Imports vault data from a file via `bw import <format> <path>`.
    ///
    /// `format` is one of the values listed by `bw import --formats`
    /// (e.g. `"bitwardenjson"`, `"lastpasscsv"`, `"1passwordcsv"`,
    /// `"keepass2xml"`, `"chromecsv"`, etc.). The path is read by
    /// `bw` directly — we do not load the file ourselves.
    fn import(&mut self, format: &str, input_path: &str) -> Result<(), BwError>;

    /// Returns the list of import formats `bw` advertises via
    /// `bw import --formats`. Used by the import popup to render a
    /// dropdown instead of asking the user to type the identifier
    /// from memory.
    ///
    /// The list is static (depends only on the installed `bw`
    /// version), so the TUI loads it once at login and caches it.
    fn list_import_formats(&mut self) -> Result<Vec<String>, BwError>;

    /// Moves a personal item into an organisation, assigning it to
    /// the given collections. Equivalent to `bw move <id> <org_id>
    /// <base64-encoded-json-array-of-collection-ids>`.
    ///
    /// Bw rejects the call when `collection_ids` is empty (org
    /// items must live in ≥1 collection); the TUI enforces the
    /// same precondition before reaching this call so the user
    /// gets an inline error instead of a CLI failure.
    fn move_item(
        &mut self,
        item_id: &str,
        organization_id: &str,
        collection_ids: &[String],
    ) -> Result<(), BwError>;

    // ── Attachments ───────────────────────────────────────────────────────

    /// Uploads `file_path` as an attachment of `item_id`. Returns the
    /// updated item (with the new attachment in its `attachments`
    /// list).
    fn upload_attachment(&mut self, item_id: &str, file_path: &str) -> Result<Item, BwError>;

    /// Downloads the attachment named `file_name` of `item_id` to
    /// `output_path`.
    fn download_attachment(
        &mut self,
        item_id: &str,
        file_name: &str,
        output_path: &str,
    ) -> Result<(), BwError>;

    /// Deletes the attachment with the given id from `item_id`.
    fn delete_attachment(&mut self, item_id: &str, attachment_id: &str) -> Result<(), BwError>;

    // ── Send (Bitwarden Send) ─────────────────────────────────────────────

    /// Creates a text Send and returns its access URL.
    ///
    /// `name` is the user-visible label, `days_to_expire` is the
    /// number of days from now until the Send self-destructs (bw
    /// allows 1–31), and `content` is the text body.
    ///
    /// Future arguments — file Send, password protection, max access
    /// count, hidden text — are not exposed yet.
    fn send_text(
        &mut self,
        name: &str,
        days_to_expire: u8,
        content: &str,
    ) -> Result<String, BwError>;

    // ── Memberships (read-only) ───────────────────────────────────────────

    /// Lists every Bitwarden organisation the user is a member of.
    /// Personal-only accounts return an empty list.
    fn list_organizations(&mut self) -> Result<Vec<Organization>, BwError>;

    /// Lists every collection the user can see across all of their
    /// organisations.
    fn list_collections(&mut self) -> Result<Vec<Collection>, BwError>;

    // ── Bulk session data ─────────────────────────────────────────────────

    /// Loads the secondary session data the TUI needs right after a
    /// fresh login or session resume — folders, organisations,
    /// collections and the import-format list — bundled so adapters
    /// that can run them concurrently (one Node cold-start per call
    /// is the dominant cost for the bw CLI) can amortise the spawn
    /// overhead.
    ///
    /// The default implementation runs them sequentially via the
    /// individual methods, preserving correctness for every
    /// implementation that doesn't bother to override it. Callers
    /// must tolerate any subset of the four results being `Err` —
    /// e.g. a personal-only account legitimately returns empty
    /// vectors for organisations and collections, but the
    /// import-format query may still fail on a stripped-down `bw`
    /// install.
    fn parallel_session_data(&mut self) -> ParallelSessionData {
        ParallelSessionData {
            folders: self.list_folders(),
            organizations: self.list_organizations(),
            collections: self.list_collections(),
            import_formats: self.list_import_formats(),
        }
    }
}
