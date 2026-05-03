//! User-feedback strip + asynchronous action queue.

/// Visible state of the feedback strip / status area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionState {
    /// Nothing is happening — the strip is blank.
    Idle,
    /// A blocking operation is in flight; the wrapped string is the
    /// label shown next to the spinner.
    Running(String),
    /// Last operation succeeded — auto-clears after ~1.5s.
    Done(String),
    /// Last operation failed — auto-clears after ~1.5s.
    Error(String),
}

/// Queue slot for an action that should be dispatched on the next
/// run-loop tick.
///
/// The queue exists so a "Running…" frame can be drawn before a
/// blocking `bw` call begins — without it, the spinner would only
/// appear after the call returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingAction {
    /// Slot is empty.
    None,
    /// Submit the login form.
    Login,
    /// Copy the selected item's username.
    CopyUsername,
    /// Copy the selected item's password.
    CopyPassword,
    /// Run a vault sync.
    SyncVault,
    /// Toggle the selected item's favorite flag.
    ToggleFavorite,
    /// Copy a literal string to the clipboard with a custom success
    /// message.
    CopyRaw(String, String),
    /// Generate and copy the TOTP code for the given item id.
    CopyTotp(String),
    /// Persist edits to the currently focused item.
    SaveEdit,
    /// Persist a new item from the create form.
    CreateItem,
    /// Move (or permanently delete) the selected item.
    DeleteItem { permanent: bool },
    /// Restore the selected trashed item.
    RestoreItem,
    /// Refresh the trash list.
    LoadTrash,
    /// Run `bw generate` with the current generator options.
    GeneratePassword,
    /// Check the password of the given item against HaveIBeenPwned
    /// breach datasets.
    CheckExposed(String),
    /// Download an attachment to a destination path.
    DownloadAttachment,
    /// Delete an attachment from its parent item.
    DeleteAttachment,
}

/// Single entry in the command-log panel.
#[derive(Debug, Clone)]
pub struct CmdEntry {
    /// Verbatim shell representation (with the session key redacted).
    pub cmd: String,
    /// Whether the command succeeded.
    pub ok: bool,
    /// Free-form trailing text — error message or summary.
    pub detail: String,
}
