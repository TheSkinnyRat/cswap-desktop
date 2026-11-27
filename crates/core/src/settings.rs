//! The app's own settings (`settings.json` in the data dir, the same file 0.1.x wrote:
//! `<appData>/cswap-desktop/settings.json`). Field names and defaults are 0.1.23's, so an
//! upgraded copy reads its old settings unchanged; missing or unknown keys fall back to
//! defaults instead of failing, because a file written by any older or newer version
//! must still open.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeSetting {
    #[default]
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UsageView {
    #[default]
    Bars,
    Rings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub cswap_path: Option<String>,
    pub theme: ThemeSetting,
    pub refresh_seconds: u32,
    pub close_to_tray: bool,
    pub start_minimized: bool,
    pub launch_at_login: bool,
    pub notify_on_auto_switch: bool,
    pub auto_start_auto_switch: bool,
    pub auto_dry_run: bool,
    pub auto_update: bool,
    pub mask_emails: bool,
    pub refresh_on_focus: bool,
    pub usage_view: UsageView,
    pub show_pace: bool,
    pub sidebar_collapsed: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            cswap_path: None,
            theme: ThemeSetting::Light,
            refresh_seconds: 60,
            close_to_tray: true,
            start_minimized: false,
            launch_at_login: false,
            notify_on_auto_switch: true,
            auto_start_auto_switch: false,
            auto_dry_run: false,
            auto_update: true,
            mask_emails: false,
            refresh_on_focus: true,
            usage_view: UsageView::Bars,
            show_pace: true,
            sidebar_collapsed: false,
        }
    }
}

/// A change to the settings, applied to the current value under the store's lock, so two
/// writers (the window and the tray) never overwrite each other's fields.
pub type SettingsEdit = Box<dyn FnOnce(&mut AppSettings) + Send>;
