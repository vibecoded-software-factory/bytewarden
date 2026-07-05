//! Password generator port.

use super::BwError;
/// Selectable output flavour of the generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorMode {
    /// Random character password (length + class flags).
    Password,
    /// Diceware-style passphrase (word count + separator).
    Passphrase,
}

/// Configuration passed to a [`PasswordGeneratorPort`] call.
///
/// Fields not relevant to the active [`mode`](Self::mode) are ignored
/// by the adapter. Defaults match the Bitwarden CLI's own defaults
/// (`-uln --length 14`).
#[derive(Debug, Clone)]
pub struct GeneratorOptions {
    pub mode: GeneratorMode,

    // ── Password options ──────────────────────────────────────────────
    /// Output length in characters. Bitwarden enforces a minimum of 5;
    /// the adapter clamps anything lower.
    pub length: u8,
    pub uppercase: bool,
    pub lowercase: bool,
    pub numbers: bool,
    pub special: bool,
    /// Avoid characters that look alike (`O`/`0`, `l`/`1`, …).
    pub avoid_ambiguous: bool,

    // ── Passphrase options ────────────────────────────────────────────
    /// Number of words. Bitwarden enforces a minimum of 3; the adapter
    /// clamps anything lower.
    pub words: u8,
    /// String inserted between words (`"-"`, `"_"`, `"."`, `"space"`,
    /// `"empty"` are all accepted by `bw generate`).
    pub separator: String,
    pub capitalize: bool,
    pub include_number: bool,
}

impl Default for GeneratorOptions {
    fn default() -> Self {
        Self {
            mode: GeneratorMode::Password,
            length: 16,
            uppercase: true,
            lowercase: true,
            numbers: true,
            special: false,
            avoid_ambiguous: false,
            words: 4,
            separator: "-".to_string(),
            capitalize: false,
            include_number: false,
        }
    }
}

/// Abstraction over a password / passphrase generator.
///
/// Concrete implementations may delegate to the Bitwarden CLI
/// (`bw generate`, see [`crate::adapters::BwGeneratorAdapter`]) or
/// build something native on top of the `rand` crate.
///
/// The port is intentionally separate from
/// [`crate::ports::VaultPort`] because generation does not require an
/// authenticated session and the two responsibilities can be swapped
/// independently.
pub trait PasswordGeneratorPort {
    /// Generates a single password / passphrase using `opts`.
    ///
    /// # Errors
    ///
    /// Returns an error string when the underlying generator fails
    /// (e.g. the `bw` binary is missing, or all character classes are
    /// disabled at once).
    fn generate(&self, opts: &GeneratorOptions) -> Result<String, BwError>;
}
