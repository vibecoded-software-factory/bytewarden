//! Key handler for the generator screen.
//!
//! Generation is fully manual: changing any option only updates the
//! configuration. The actual `bw generate` call is fired *only* when
//! the user explicitly presses Enter.

use crossterm::event::{KeyCode, KeyEvent};

use crate::ports::GeneratorMode;
use crate::tui::app::App;
use crate::tui::flows::generator::{
    cancel, copy_result, focus_step, request_generate, toggle_mode, use_result,
};
use crate::tui::generator::{
    GeneratorFocus, PASSPHRASE_WORDS_MAX, PASSPHRASE_WORDS_MIN, PASSWORD_LENGTH_MAX,
    PASSWORD_LENGTH_MIN,
};
use crate::tui::input::is_alt;

/// Dispatches a single key event on the generator screen.
pub fn handle(app: &mut App, key: KeyEvent) {
    // Modifier-driven shortcuts first — Alt+C and Alt+U work regardless
    // of the focused control.
    if is_alt(&key) {
        match key.code {
            KeyCode::Char('c') => return copy_result(app),
            KeyCode::Char('u') => return use_result(app),
            _ => {}
        }
    }

    match key.code {
        KeyCode::Esc => return cancel(app),
        KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => {
            return focus_step(app, 1);
        }
        KeyCode::BackTab | KeyCode::Up | KeyCode::Char('k') => {
            return focus_step(app, -1);
        }
        // The single, explicit "generate now" trigger.
        KeyCode::Enter => return request_generate(app),
        _ => {}
    }

    // Per-control behaviour for arrows / space / typing — these only
    // mutate the configuration; no implicit `queue_regenerate`.
    match app.generator.focus {
        GeneratorFocus::Mode => match key.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') => toggle_mode(app),
            _ => {}
        },
        GeneratorFocus::Length => match key.code {
            KeyCode::Right | KeyCode::Char('+') => {
                app.generator.options.length = app
                    .generator
                    .options
                    .length
                    .saturating_add(1)
                    .min(PASSWORD_LENGTH_MAX);
            }
            KeyCode::Left | KeyCode::Char('-') => {
                app.generator.options.length = app
                    .generator
                    .options
                    .length
                    .saturating_sub(1)
                    .max(PASSWORD_LENGTH_MIN);
            }
            _ => {}
        },
        GeneratorFocus::Words => match key.code {
            KeyCode::Right | KeyCode::Char('+') => {
                app.generator.options.words = app
                    .generator
                    .options
                    .words
                    .saturating_add(1)
                    .min(PASSPHRASE_WORDS_MAX);
            }
            KeyCode::Left | KeyCode::Char('-') => {
                app.generator.options.words = app
                    .generator
                    .options
                    .words
                    .saturating_sub(1)
                    .max(PASSPHRASE_WORDS_MIN);
            }
            _ => {}
        },
        GeneratorFocus::Separator => match key.code {
            KeyCode::Char(c) if !is_alt(&key) => {
                app.generator.options.separator = c.to_string();
            }
            KeyCode::Backspace => app.generator.options.separator.clear(),
            _ => {}
        },
        GeneratorFocus::Uppercase => toggle_if(app, key, |o| &mut o.uppercase),
        GeneratorFocus::Lowercase => toggle_if(app, key, |o| &mut o.lowercase),
        GeneratorFocus::Numbers => toggle_if(app, key, |o| &mut o.numbers),
        GeneratorFocus::Special => toggle_if(app, key, |o| &mut o.special),
        GeneratorFocus::Ambiguous => toggle_if(app, key, |o| &mut o.avoid_ambiguous),
        GeneratorFocus::Capitalize => toggle_if(app, key, |o| &mut o.capitalize),
        GeneratorFocus::IncludeNumber => toggle_if(app, key, |o| &mut o.include_number),
        GeneratorFocus::Result => {
            // Result row is read-only — Enter (handled above) is the
            // only meaningful action. Suppress an `unused_imports`
            // warning when the enum gains future variants.
            let _ = GeneratorMode::Password;
        }
    }
}

/// Toggles a `bool` field of the generator options on Space / arrows.
/// Pure config mutation — does not trigger a regenerate.
fn toggle_if(
    app: &mut App,
    key: KeyEvent,
    pick: fn(&mut crate::ports::GeneratorOptions) -> &mut bool,
) {
    let trigger = matches!(
        key.code,
        KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right
    );
    if !trigger {
        return;
    }
    let f = pick(&mut app.generator.options);
    *f = !*f;
}
