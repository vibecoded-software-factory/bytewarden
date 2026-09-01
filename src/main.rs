use bytewarden::adapters::{
    BwCliAdapter, BwGeneratorAdapter, SystemClipboardAdapter, TomlSettingsAdapter,
};
use bytewarden::ports::SettingsPort;
use bytewarden::tui;
use bytewarden::tui::session_file;

use color_eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;

    session_file::cleanup_orphans();
    let seed_session_key = session_file::load();

    let settings_adapter = TomlSettingsAdapter::new();
    let cfg = settings_adapter.read();

    let adapter = BwCliAdapter::new_with(seed_session_key)
        .with_list_items_timeout(cfg.list_items_timeout_secs);

    let list_items_timeout = adapter.list_items_timeout_handle();
    let vault = Box::new(adapter);
    let clipboard = Box::new(SystemClipboardAdapter::new());
    let settings = Box::new(settings_adapter);
    let generator = Box::new(BwGeneratorAdapter::new());

    tui::run(vault, clipboard, settings, generator, list_items_timeout)
}
