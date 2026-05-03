//! Composition root — instantiates concrete adapters and starts the
//! [`bytewarden::tui`] event loop.

use bytewarden::adapters::{
    BwCliAdapter, BwGeneratorAdapter, SystemClipboardAdapter, TomlSettingsAdapter,
};
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

    let vault = Box::new(BwCliAdapter::new_with(seed_session_key));
    let clipboard = Box::new(SystemClipboardAdapter::new());
    let settings = Box::new(TomlSettingsAdapter::new());
    let generator = Box::new(BwGeneratorAdapter::new());

    tui::run(vault, clipboard, settings, generator)
}
