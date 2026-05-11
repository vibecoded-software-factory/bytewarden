//! Composition root — instantiates concrete adapters and starts the
//! [`bytewarden::tui`] event loop.

use bytewarden::adapters::{
    BwCliAdapter, BwGeneratorAdapter, SystemClipboardAdapter, TomlSettingsAdapter,
};
use bytewarden::ports::SettingsPort;
use bytewarden::tui;
use bytewarden::tui::session_file;

use color_eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;

    // If the user enabled "Keep session while terminal is open" on a
    // previous launch, the per-PPID file holds the session key. We
    // pass it directly to the adapter constructor — no env var
    // mutation, no `unsafe`. Stale files (parents that already died,
    // or files older than the age cap) are cleaned up while we're
    // here. The adapter still falls back to `$BW_SESSION` from the
    // inherited environment when the seed is `None`, so users who
    // export the variable manually keep working.
    session_file::cleanup_orphans();
    let seed_session_key = session_file::load();

    // Read settings before constructing the vault adapter so the
    // configurable `bw list items` timeout (used by users with very
    // large vaults) is in effect from the very first call. The TUI
    // also reads the same `UserSettings` via its own port, so the
    // double-read is intentional — keeps the composition root the
    // single source of truth for adapter wiring without coupling the
    // TUI to the adapter constructor.
    let settings_adapter = TomlSettingsAdapter::new();
    let cfg = settings_adapter.read();

    let vault = Box::new(
        BwCliAdapter::new_with(seed_session_key)
            .with_list_items_timeout(cfg.list_items_timeout_secs),
    );
    let clipboard = Box::new(SystemClipboardAdapter::new());
    let settings = Box::new(settings_adapter);
    let generator = Box::new(BwGeneratorAdapter::new());

    tui::run(vault, clipboard, settings, generator)
}
