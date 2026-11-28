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

/// How the Accounts page lists the accounts other than the active one (1.1.0): rows with
/// mini bars, or tiles with small rings. It replaced 1.0's bars/rings `usageView`, whose key
/// an older file may still carry; the settings store keeps it as an unknown key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountsLayout {
    #[default]
    Rows,
    Tiles,
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
    pub show_pace: bool,
    /// The active account as a hero card on top of the Accounts page (with the Best next
    /// strip under it); off puts it back into the list.
    pub show_hero: bool,
    pub accounts_layout: AccountsLayout,
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
            show_pace: true,
            show_hero: true,
            accounts_layout: AccountsLayout::Rows,
            sidebar_collapsed: false,
        }
    }
}

/// A change to the settings, applied to the current value under the store's lock, so two
/// writers (the window and the tray) never overwrite each other's fields.
pub type SettingsEdit = Box<dyn FnOnce(&mut AppSettings) + Send>;

#[cfg(test)]
mod tests {
    use super::*;

    // TESTED RULE: the 1.1.0 fields round-trip under their camelCase names, and a file from
    // 1.0 (no such keys, the retired `usageView`) still loads, with the hero shown and rows.
    #[test]
    fn hero_and_layout_round_trip_and_old_files_load() {
        let s = AppSettings { show_hero: false, accounts_layout: AccountsLayout::Tiles, ..Default::default() };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["showHero"], false);
        assert_eq!(v["accountsLayout"], "tiles");
        assert_eq!(serde_json::from_value::<AppSettings>(v).unwrap(), s);

        let old: AppSettings =
            serde_json::from_str(r#"{"theme":"dark","usageView":"rings","showPace":false}"#).unwrap();
        assert!(old.show_hero);
        assert_eq!(old.accounts_layout, AccountsLayout::Rows);
        assert_eq!((old.theme, old.show_pace), (ThemeSetting::Dark, false));
    }
}
