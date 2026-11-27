//! Self-update. Builds are not signed by any CA (no Developer ID, no Authenticode: decided
//! 7 Sep 2026), so trust comes from our own ed25519 release key: every release asset has a
//! detached signature, the public key is compiled in, and nothing that fails verification
//! is ever run. HTTPS to GitHub plus a checksum from the same release would not be enough:
//! whoever can replace an asset can replace its checksum too.
//!
//! Swap per install kind: Windows NSIS (silent installer after exit), macOS `.app` (bundle
//! from the DMG copied over this one with `ditto --noextattr`, reopen), Linux AppImage
//! (rename over, restart). Anything else (deb, dev build, translocated or unwritable
//! macOS bundle) gets the release page instead.
//!
//! CONTRACT: [`UpdateConfig`], [`Updater::start`], [`finish`]. OWNER: platform.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use cswap_core::EventSink;

#[derive(Debug, Clone)]
pub struct UpdateConfig {
    /// `owner/repo` on GitHub.
    pub repo: String,
    /// This build's version (`CARGO_PKG_VERSION`).
    pub current: String,
    /// The app's data dir: `updater.log`, download cache.
    pub data_dir: PathBuf,
    /// Check on start and every few hours (the `autoUpdate` setting).
    pub auto: bool,
}

pub struct Updater {}

impl Updater {
    /// `quit` ends the app; called once the installer/helper has been handed the swap.
    pub fn start(config: UpdateConfig, sink: EventSink, quit: Arc<dyn Fn() + Send + Sync>) -> Arc<Updater> {
        let _ = (config, sink, quit);
        unimplemented!("platform")
    }
}

/// When this process was started as the update helper, finishes the swap and returns its
/// outcome; `None` for a normal launch. The app calls this first thing in `main`.
pub fn finish(args: &[OsString]) -> Option<Result<(), String>> {
    let _ = args;
    None
}
