//! The typed error returned by every fallible port operation.
//!
//! Stringly-typed errors are opaque: the UI can't tell a timeout from
//! "not found" from a missing binary, and ends up string-matching
//! human-readable output. Classifying at the adapter boundary lets the
//! command log store the *kind* and the feedback strip render a good
//! message (via [`Display`]) without any consumer parsing the text.
//!
//! Variants are ordered by pipeline stage: spawn → run → exit → parse.

use std::fmt;

/// A failed port operation, classified by where it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BwError {
    /// Couldn't exec the subprocess (`bw` / a clipboard tool not on
    /// `PATH`, or a permissions problem). Wraps the OS error text.
    Spawn(String),
    /// A per-operation wall-clock budget was exceeded; the child was
    /// killed. `label` names the operation, `secs` the budget that
    /// elapsed.
    Timeout { label: String, secs: u64 },
    /// The process exited non-zero. `stderr` is passed through verbatim
    /// (it's what the user needs to read); `status` is the exit code
    /// when one is available.
    Exit { stderr: String, status: Option<i32> },
    /// The process exited successfully but stdout wasn't the JSON we
    /// expected (serde parse failure). Carries the full diagnostic.
    InvalidJson(String),
    /// Parsed / exited 0, but the output didn't have the shape the
    /// caller needed (e.g. a non-numeric HIBP count, an empty result
    /// where a value was required).
    Shape(String),
    /// An adapter/worker panic captured via `catch_unwind`, or an
    /// internal precondition/usage failure (e.g. a session-required
    /// call issued while the vault is locked).
    Internal(String),
}

impl BwError {
    /// Builds an [`BwError::Exit`] from a process's stderr text + status
    /// code. The single constructor the adapter uses for the "`bw`
    /// exited non-zero" path.
    pub fn exit(stderr: impl Into<String>, status: Option<i32>) -> Self {
        BwError::Exit {
            stderr: stderr.into(),
            status,
        }
    }
}

impl fmt::Display for BwError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BwError::Spawn(s) => write!(f, "could not run process: {s}"),
            BwError::Timeout { label, secs } => write!(f, "{label} timed out after {secs}s"),
            // Preserve the previous behaviour where the raw stderr *was*
            // the whole error message shown to the user.
            BwError::Exit { stderr, .. } => {
                if stderr.is_empty() {
                    f.write_str("command failed")
                } else {
                    f.write_str(stderr)
                }
            }
            BwError::InvalidJson(d) => f.write_str(d),
            BwError::Shape(d) => f.write_str(d),
            BwError::Internal(d) => write!(f, "internal error: {d}"),
        }
    }
}

impl std::error::Error for BwError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_displays_stderr_verbatim() {
        let e = BwError::exit("bad session", Some(1));
        assert_eq!(e.to_string(), "bad session");
    }

    #[test]
    fn exit_with_empty_stderr_has_a_fallback() {
        assert_eq!(BwError::exit("", None).to_string(), "command failed");
    }

    #[test]
    fn timeout_reads_naturally() {
        let e = BwError::Timeout {
            label: "bw".into(),
            secs: 30,
        };
        assert_eq!(e.to_string(), "bw timed out after 30s");
    }

    #[test]
    fn internal_is_prefixed() {
        assert_eq!(
            BwError::Internal("boom".into()).to_string(),
            "internal error: boom"
        );
    }
}
