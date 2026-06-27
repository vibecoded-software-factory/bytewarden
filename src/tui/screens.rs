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
    /// Settings overlay — a sectioned preferences screen (Theme first),
    /// opened with `F9`.
    Settings,
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
    /// Master-password reverify popup, opened when the user attempts
    /// an action that exposes a secret (copy password / TOTP / hidden
    /// custom field, F2 reveal) on an item with the Bitwarden
    /// `reprompt` flag set.
    RepromptUnlock,
    /// Multi-select popup for assigning the focused item to one or
    /// more collections of its owning organisation. Opened with
    /// `Alt+L` from the edit-mode "Collections" row.
    AssignCollections,
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
