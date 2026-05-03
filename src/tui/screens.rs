//! Screen / focus / form-field enums used by the TUI state machine.

/// Top-level screens of the TUI.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Screen {
    /// Boot splash shown while `bw status` runs.
    Splash,
    /// Login / unlock form.
    Login,
    /// Main vault list with sidebar.
    Vault,
    /// Single-item detail view.
    Detail,
    /// Help popup overlay.
    Help,
    /// "Create new item" form (with type-picker first step).
    Create,
    /// Confirm-delete popup overlay.
    ConfirmDelete,
    /// Confirm-logout popup overlay.
    ConfirmLogout,
    /// Standalone password / passphrase generator.
    Generator,
    /// Rename-custom-field popup overlay (drawn on top of the detail
    /// edit-mode screen).
    RenameField,
    /// Folder name input popup (used for both Create and Rename).
    FolderName,
    /// Confirm-delete-folder popup overlay.
    ConfirmDeleteFolder,
    /// Vault-export popup overlay (format picker + output path).
    Export,
    /// Vault-import popup overlay (format string + input path).
    Import,
    /// Attachment-upload popup overlay (drawn on top of the detail
    /// screen).
    AttachmentUpload,
    /// Attachment-download popup overlay (path picker).
    AttachmentDownload,
    /// Confirm-delete-attachment popup overlay.
    ConfirmDeleteAttachment,
    /// Send (text) creation popup overlay.
    SendCreate,
    /// Memberships popup — read-only view of organisations + their
    /// collections.
    Memberships,
}

/// Panels inside the [`Screen::Vault`] layout that can hold focus.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Focus {
    /// Top-left status pane (panel `[0]`).
    Status,
    /// Top search bar.
    Search,
    /// Sidebar "Folders" list (panel `[1]`).
    Folders,
    /// Sidebar "Items" filter list (panel `[2]`).
    Items,
    /// Main vault list (panel `[3]`).
    List,
    /// Bottom command-log panel (panel `[4]`).
    CmdLog,
}

/// Active text input on the [`Screen::Login`] form.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LoginField {
    /// Bitwarden server URL (`https://vault.bitwarden.com` by default,
    /// editable for self-hosted instances).
    Server,
    Email,
    Password,
    /// One-time code prompted after a "new device" detection.
    Otp,
    /// "Save email" checkbox.
    SaveEmail,
    /// "Auto-lock" checkbox.
    AutoLock,
    /// "Keep session while terminal is open" checkbox.
    KeepSession,
}
