use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BwError {
    Spawn(String),

    Timeout { label: String, secs: u64 },

    Exit { stderr: String, status: Option<i32> },

    InvalidJson(String),

    Shape(String),

    Internal(String),
}

impl BwError {
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
