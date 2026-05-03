//! [`crate::ports::PasswordGeneratorPort`] implementation that shells
//! out to `bw generate`.
//!
//! Independent from [`crate::adapters::BwCliAdapter`] because password
//! generation needs no session: it is a pure stateless operation, and
//! keeping the responsibilities split lets either backend be replaced
//! without touching the other.

use std::process::Command;

use crate::ports::{GeneratorMode, GeneratorOptions, PasswordGeneratorPort};

/// Bitwarden's hard minimum lengths, mirrored here so the adapter can
/// clamp UI input before invoking the CLI.
const MIN_PASSWORD_LENGTH: u8 = 5;
const MIN_PASSPHRASE_WORDS: u8 = 3;

/// Generator adapter — calls `bw generate <flags>` per request.
#[derive(Debug, Default)]
pub struct BwGeneratorAdapter;

impl BwGeneratorAdapter {
    /// Constructs a new adapter. Cheap; does not touch the CLI.
    pub fn new() -> Self {
        Self
    }

    /// Builds the `bw generate` argument vector for the given options.
    fn build_args(opts: &GeneratorOptions) -> Vec<String> {
        let mut args: Vec<String> = vec!["generate".to_string()];
        match opts.mode {
            GeneratorMode::Password => {
                if opts.uppercase {
                    args.push("-u".into());
                }
                if opts.lowercase {
                    args.push("-l".into());
                }
                if opts.numbers {
                    args.push("-n".into());
                }
                if opts.special {
                    args.push("-s".into());
                }
                if opts.avoid_ambiguous {
                    args.push("--ambiguous".into());
                }
                args.push("--length".into());
                args.push(opts.length.max(MIN_PASSWORD_LENGTH).to_string());
            }
            GeneratorMode::Passphrase => {
                args.push("--passphrase".into());
                args.push("--words".into());
                args.push(opts.words.max(MIN_PASSPHRASE_WORDS).to_string());
                if !opts.separator.is_empty() {
                    args.push("--separator".into());
                    args.push(opts.separator.clone());
                }
                if opts.capitalize {
                    args.push("-c".into());
                }
                if opts.include_number {
                    args.push("--includeNumber".into());
                }
            }
        }
        args
    }
}

impl PasswordGeneratorPort for BwGeneratorAdapter {
    fn generate(&self, opts: &GeneratorOptions) -> Result<String, String> {
        // Validate up front: in password mode at least one character
        // class must be enabled, otherwise `bw` returns an unhelpful
        // error and we'd surface garbage.
        if opts.mode == GeneratorMode::Password
            && !(opts.uppercase || opts.lowercase || opts.numbers || opts.special)
        {
            return Err("At least one character class must be enabled.".into());
        }

        let args = Self::build_args(opts);
        let out = Command::new("bw")
            .args(&args)
            .output()
            .map_err(|e| format!("Could not run bw: {e}"))?;

        if out.status.success() {
            let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if value.is_empty() {
                Err("bw generate returned an empty value".into())
            } else {
                Ok(value)
            }
        } else {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if err.is_empty() {
                "bw generate failed".into()
            } else {
                err
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_args_include_enabled_classes() {
        let opts = GeneratorOptions {
            mode: GeneratorMode::Password,
            length: 20,
            uppercase: true,
            lowercase: false,
            numbers: true,
            special: false,
            avoid_ambiguous: true,
            ..GeneratorOptions::default()
        };
        let args = BwGeneratorAdapter::build_args(&opts);
        assert!(args.contains(&"-u".to_string()));
        assert!(!args.contains(&"-l".to_string()));
        assert!(args.contains(&"-n".to_string()));
        assert!(!args.contains(&"-s".to_string()));
        assert!(args.contains(&"--ambiguous".to_string()));
        assert!(args.contains(&"--length".to_string()));
        assert!(args.contains(&"20".to_string()));
    }

    #[test]
    fn password_length_is_clamped_to_minimum() {
        let opts = GeneratorOptions {
            mode: GeneratorMode::Password,
            length: 2,
            ..GeneratorOptions::default()
        };
        let args = BwGeneratorAdapter::build_args(&opts);
        let len_pos = args
            .iter()
            .position(|a| a == "--length")
            .expect("--length present");
        assert_eq!(args[len_pos + 1], MIN_PASSWORD_LENGTH.to_string());
    }

    #[test]
    fn passphrase_args_include_word_count_and_separator() {
        let opts = GeneratorOptions {
            mode: GeneratorMode::Passphrase,
            words: 5,
            separator: "_".to_string(),
            capitalize: true,
            include_number: true,
            ..GeneratorOptions::default()
        };
        let args = BwGeneratorAdapter::build_args(&opts);
        assert!(args.contains(&"--passphrase".to_string()));
        assert!(args.contains(&"5".to_string()));
        assert!(args.contains(&"_".to_string()));
        assert!(args.contains(&"-c".to_string()));
        assert!(args.contains(&"--includeNumber".to_string()));
    }

    #[test]
    fn passphrase_words_clamped_to_minimum() {
        let opts = GeneratorOptions {
            mode: GeneratorMode::Passphrase,
            words: 1,
            ..GeneratorOptions::default()
        };
        let args = BwGeneratorAdapter::build_args(&opts);
        let words_pos = args
            .iter()
            .position(|a| a == "--words")
            .expect("--words present");
        assert_eq!(args[words_pos + 1], MIN_PASSPHRASE_WORDS.to_string());
    }
}
