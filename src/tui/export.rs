use crate::domain::LineEditor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,

    Json,

    EncryptedJson,
}

impl ExportFormat {
    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Csv => "CSV",
            ExportFormat::Json => "JSON",
            ExportFormat::EncryptedJson => "Encrypted JSON",
        }
    }

    pub fn cli_arg(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
            ExportFormat::EncryptedJson => "encrypted_json",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Json | ExportFormat::EncryptedJson => "json",
        }
    }

    pub fn next(self) -> Self {
        match self {
            ExportFormat::Csv => ExportFormat::Json,
            ExportFormat::Json => ExportFormat::EncryptedJson,
            ExportFormat::EncryptedJson => ExportFormat::Csv,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFocus {
    Format,
    Path,
}

#[derive(Debug, Clone)]
pub struct ExportState {
    pub format: ExportFormat,
    pub path: LineEditor,
    pub focus: ExportFocus,
}

impl ExportState {
    pub fn new() -> Self {
        let format = ExportFormat::Json;
        Self {
            format,
            path: LineEditor::with_text(default_output_path(format)),
            focus: ExportFocus::Format,
        }
    }

    pub fn refresh_default_path(&mut self) {
        self.path.set(default_output_path(self.format));
    }
}

impl Default for ExportState {
    fn default() -> Self {
        Self::new()
    }
}

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
