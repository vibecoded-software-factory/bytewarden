//! Auto-lock inactivity timer state.
//!
//! Whether the vault auto-locks after inactivity, the idle window, and
//! the last-activity timestamp — split out of the
//! [`crate::tui::app::App`] god-struct into one small unit that owns the
//! "is the idle window elapsed?" decision. The values seed from settings
//! at boot ([`crate::ports::SettingsConfig`]); the timer is reset on
//! every keypress and checked once per tick.

use std::time::Instant;

/// The auto-lock timer.
pub struct AutoLock {
    /// Whether auto-lock is enabled at all.
    pub enabled: bool,
    /// Idle window, in seconds, before the vault locks.
    pub after_secs: u64,
    /// When the user was last active.
    pub last_activity: Instant,
}

impl AutoLock {
    /// Builds the timer from the settings values, starting the clock now.
    pub fn new(enabled: bool, after_secs: u64) -> Self {
        Self {
            enabled,
            after_secs,
            last_activity: Instant::now(),
        }
    }

    /// Records "user is active right now" — restarts the idle window.
    pub fn reset(&mut self) {
        self.last_activity = Instant::now();
    }

    /// `true` when auto-lock is on **and** the idle window has elapsed.
    pub fn is_expired(&self) -> bool {
        self.enabled && self.last_activity.elapsed().as_secs() >= self.after_secs
    }
}
