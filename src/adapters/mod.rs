//! Adapters — concrete implementations of the [`crate::ports`] traits.
//!
//! This is the only layer allowed to import OS-level dependencies
//! (`std::process::Command`, the filesystem, environment variables).
//! The rest of the crate talks to these adapters through the trait
//! abstractions, so swapping or mocking them is straightforward.

pub mod bw_cli;
pub mod bw_generator;
pub mod clipboard_system;
pub mod settings_toml;

pub use bw_cli::BwCliAdapter;
pub use bw_generator::BwGeneratorAdapter;
pub use clipboard_system::SystemClipboardAdapter;
pub use settings_toml::TomlSettingsAdapter;
