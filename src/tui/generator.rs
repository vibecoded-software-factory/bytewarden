//! Generator-screen state.
//!
//! Owned by [`crate::tui::App`] and consumed by
//! [`crate::tui::flows::generator`], [`crate::tui::input::generator`]
//! and [`crate::tui::view::generator`].

use crate::ports::{GeneratorMode, GeneratorOptions};

/// Currently focused control on the generator screen.
///
/// The exact set of valid focuses depends on `mode` — the helper
/// [`focusable_for`] returns the ordered list for the active mode and
/// the navigation logic looks up by index inside that list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorFocus {
    Mode,
    // Password
    Length,
    Uppercase,
    Lowercase,
    Numbers,
    Special,
    Ambiguous,
    // Passphrase
    Words,
    Separator,
    Capitalize,
    IncludeNumber,
    // Common
    Result,
}

/// Where the "Use" action should write the generated value when
/// returning from the generator screen.
///
/// Set by `flows::generator::open_for_*` and consumed by
/// `flows::generator::use_generated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnTarget {
    /// Write back into `app.edit_fields[idx]` and return to the detail
    /// screen with `edit_mode = true`.
    EditField(usize),
    /// Write back into `app.create_fields[idx]` and return to the
    /// create screen.
    CreateField(usize),
}

/// All state the generator screen needs.
#[derive(Debug, Clone)]
pub struct GeneratorState {
    pub options: GeneratorOptions,
    /// Last successfully generated value (empty before the first
    /// generation succeeds).
    pub result: String,
    /// Currently focused control.
    pub focus: GeneratorFocus,
    /// Where to write the generated value when the user picks "Use".
    /// `None` means the generator was opened standalone — copy is the
    /// only output.
    pub return_target: Option<ReturnTarget>,
}

impl Default for GeneratorState {
    fn default() -> Self {
        Self {
            options: GeneratorOptions::default(),
            result: String::new(),
            focus: GeneratorFocus::Length,
            return_target: None,
        }
    }
}

/// Returns the ordered list of focusable controls for the given mode.
///
/// Used by both the navigation handler and the renderer so they stay
/// in sync without explicitly numbering the rows.
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

/// Returns the index of `focus` inside the focusable list for `mode`,
/// or `0` if not present (which can happen right after a mode switch).
pub fn focus_index(mode: GeneratorMode, focus: GeneratorFocus) -> usize {
    focusable_for(mode)
        .iter()
        .position(|f| *f == focus)
        .unwrap_or(0)
}

/// Bounds for password length (matches `bw generate` constraints
/// loosely — Bitwarden enforces `>= 5` only, but very long passwords
/// have no practical upside and a UI cap keeps the input single-line).
pub const PASSWORD_LENGTH_MIN: u8 = 5;
pub const PASSWORD_LENGTH_MAX: u8 = 128;

/// Bounds for passphrase word count.
pub const PASSPHRASE_WORDS_MIN: u8 = 3;
pub const PASSPHRASE_WORDS_MAX: u8 = 20;
