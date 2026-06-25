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
