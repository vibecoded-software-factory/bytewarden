use zeroize::Zeroizing;

use crate::ports::{GeneratorMode, GeneratorOptions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorFocus {
    Mode,

    Length,
    Uppercase,
    Lowercase,
    Numbers,
    Special,
    Ambiguous,

    Words,
    Separator,
    Capitalize,
    IncludeNumber,

    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnTarget {
    EditField(usize),

    CreateField(usize),
}

#[derive(Debug, Clone)]
pub struct GeneratorState {
    pub options: GeneratorOptions,

    pub result: Zeroizing<String>,

    pub focus: GeneratorFocus,

    pub return_target: Option<ReturnTarget>,
}

impl Default for GeneratorState {
    fn default() -> Self {
        Self {
            options: GeneratorOptions::default(),
            result: Zeroizing::new(String::new()),
            focus: GeneratorFocus::Length,
            return_target: None,
        }
    }
}

pub fn focusable_for(mode: GeneratorMode) -> &'static [GeneratorFocus] {
    match mode {
        GeneratorMode::Password => &[
            GeneratorFocus::Mode,
            GeneratorFocus::Length,
            GeneratorFocus::Uppercase,
            GeneratorFocus::Lowercase,
            GeneratorFocus::Numbers,
            GeneratorFocus::Special,
            GeneratorFocus::Ambiguous,
            GeneratorFocus::Result,
        ],
        GeneratorMode::Passphrase => &[
            GeneratorFocus::Mode,
            GeneratorFocus::Words,
            GeneratorFocus::Separator,
            GeneratorFocus::Capitalize,
            GeneratorFocus::IncludeNumber,
            GeneratorFocus::Result,
        ],
    }
}

pub fn focus_index(mode: GeneratorMode, focus: GeneratorFocus) -> usize {
    focusable_for(mode)
        .iter()
        .position(|f| *f == focus)
        .unwrap_or(0)
}

pub const PASSWORD_LENGTH_MIN: u8 = 5;
pub const PASSWORD_LENGTH_MAX: u8 = 128;

pub const PASSPHRASE_WORDS_MIN: u8 = 3;
pub const PASSPHRASE_WORDS_MAX: u8 = 20;
