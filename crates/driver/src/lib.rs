//! The real [`cswap_core::Cswap`]: everything goes through the cswap CLI (`--json` schema
//! v1), never by writing cswap's vault. The only files read directly are `mappings.json`,
//! the vault root (watched, so terminal/TUI changes show up live) and two non-secret
//! strings of Claude Code's `.credentials.json` (`subscriptionType`, `rateLimitTier`).
//! cswap is NOT bundled: two cswap versions writing one vault lose accounts silently.
//!
//! CONTRACT: [`DriverConfig`], [`Native::start`], [`Native::shutdown`], [`data_dir`].
//! OWNER: driver.

use std::path::PathBuf;
use std::sync::Arc;

use cswap_core::EventSink;

#[derive(Debug, Clone, Default)]
pub struct DriverConfig {
    /// The app's data dir ([`data_dir`]): `settings.json`, `plans.json`.
    pub data_dir: PathBuf,
}

/// `CSWAP_DESKTOP_USERDATA` if set (tests), else `<appData>/cswap-desktop`: the same
/// folder 0.1.x used (`%APPDATA%`, `~/Library/Application Support`, `$XDG_CONFIG_HOME`
/// or `~/.config`), pinned so renaming the product never moves anyone's settings.
pub fn data_dir() -> PathBuf {
    unimplemented!("driver")
}

pub struct Native {}

impl Native {
    /// Loads settings, starts binary detection and the first refresh, the refresh timer and
    /// the vault watcher, and (when the setting says so) the auto-switch process. Returns at
    /// once; results arrive through `sink`.
    pub fn start(config: DriverConfig, sink: EventSink) -> Arc<Native> {
        let _ = (config, sink);
        unimplemented!("driver")
    }

    /// Stops the auto-switch child and the timers. Called once before the app exits.
    pub fn shutdown(&self) {}
}
