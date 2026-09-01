use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct UserSettings {
    pub save_email: bool,

    pub email: Option<String>,

    pub auto_lock: bool,

    pub lock_after_secs: u64,

    pub keep_session: bool,

    pub clipboard_clear_secs: u64,

    pub list_items_timeout_secs: u64,

    pub icon_style: String,
}

pub trait SettingsPort {
    fn read(&self) -> UserSettings;

    fn write(&self, save_email: bool, email: Option<&str>);

    fn write_auto_lock(&self, auto_lock: bool);

    fn write_keep_session(&self, keep_session: bool);

    fn write_lock_after_secs(&self, secs: u64);

    fn write_clipboard_clear_secs(&self, secs: u64);

    fn write_list_items_timeout_secs(&self, secs: u64);

    fn write_icon_style(&self, style: &str);

    fn write_theme_name(&self, name: &str);

    fn config_dir(&self) -> PathBuf;
}
