//! Export-popup state.

use crate::domain::LineEditor;

/// Output formats accepted by `bw export --format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Plain CSV — easy to inspect, contains plaintext passwords.
    Csv,
    /// Plain JSON — same security profile as CSV but preserves the
    /// full Bitwarden item schema.
    Json,
    /// JSON encrypted with the user's account encryption key. Can
    /// only be re-imported into the same Bitwarden account.
    EncryptedJson,
}

impl ExportFormat {
    /// Human-readable label shown in the picker.
    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Csv => "CSV",
            ExportFormat::Json => "JSON",
            ExportFormat::EncryptedJson => "Encrypted JSON",
        }
    }

    /// Value passed to `bw export --format`.
    pub fn cli_arg(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
            ExportFormat::EncryptedJson => "encrypted_json",
        }
    }

    /// File-extension hint for the auto-generated default path.
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Json | ExportFormat::EncryptedJson => "json",
        }
    }

    /// Cycles forward through the variants (CSV → JSON → Encrypted JSON → CSV).
    pub fn next(self) -> Self {
        match self {
            ExportFormat::Csv => ExportFormat::Json,
            ExportFormat::Json => ExportFormat::EncryptedJson,
            ExportFormat::EncryptedJson => ExportFormat::Csv,
        }
    }
}

/// Which control of the export popup currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFocus {
    Format,
    Path,
}

/// Buffer for the in-flight export popup. `None` outside the popup.
#[derive(Debug, Clone)]
pub struct ExportState {
    pub format: ExportFormat,
    pub path: LineEditor,
    pub focus: ExportFocus,
}

impl ExportState {
    /// Builds a fresh popup state with a sensible default output path:
    /// `~/Downloads/bytewarden-export-YYYYMMDD-HHMMSS.<ext>`.
    pub fn new() -> Self {
        let format = ExportFormat::Json;
        Self {
            format,
            path: LineEditor::with_text(default_output_path(format)),
            focus: ExportFocus::Format,
        }
    }

    /// Replaces the path with a fresh default for the current format
    /// — used after the user cycles the format so they get a sensible
    /// extension without having to retype.
    pub fn refresh_default_path(&mut self) {
        self.path.set(default_output_path(self.format));
    }
}

impl Default for ExportState {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds `<HOME>/Downloads/bytewarden-export-<unix-timestamp>.<ext>`.
///
/// Uses the unix timestamp as a tiebreaker so subsequent exports
/// don't overwrite each other, while staying free of the `chrono`
/// dependency.
fn default_output_path(format: ExportFormat) -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!(
        "{home}/Downloads/bytewarden-export-{ts}.{ext}",
        ext = format.extension()
    )
}
