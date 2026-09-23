use super::BwError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorMode {
    Password,

    Passphrase,
}

#[derive(Debug, Clone)]
pub struct GeneratorOptions {
    pub mode: GeneratorMode,

    pub length: u8,
    pub uppercase: bool,
    pub lowercase: bool,
    pub numbers: bool,
    pub special: bool,

    pub avoid_ambiguous: bool,

    pub words: u8,

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

pub trait PasswordGeneratorPort {
    fn generate(&self, opts: &GeneratorOptions) -> Result<String, BwError>;
}
