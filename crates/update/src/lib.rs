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
//! Flow as 0.1.23's electron-updater: check on start (after 15 s) and every 6 hours while
//! `autoUpdate` is on → a newer release downloads by itself → "Downloaded" → the user
//! installs (the app quits and comes back new), or quitting with an update downloaded
//! installs it without restarting (0.1.x's `autoInstallOnAppQuit`).
//!
//! CONTRACT: [`UpdateConfig`], [`Updater::start`], [`finish`]. OWNER: platform.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use cswap_core::{Event, EventSink, Pending, UpdaterState, UpdaterStatus, Updates};
use ed25519_dalek::VerifyingKey;
use sha2::{Digest, Sha512};

pub mod kind;
pub mod legacy;
pub mod release;
pub mod sign;
pub mod swap;
pub mod time;

use kind::InstallKind;
use swap::Download;

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

/// First check after start, then the interval (0.1.23's `schedule`).
const FIRST_CHECK: Duration = Duration::from_secs(15);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);
pub const API_BASE: &str = "https://api.github.com";
/// Test seam: point the release lookup at a local server.
pub const API_ENV: &str = "CSWAP_DESKTOP_UPDATE_API";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Verifies a downloaded asset against its `.sig` beside it.
pub fn verify_download(dl: &Download, key: &VerifyingKey) -> Result<(), String> {
    let sig_path = PathBuf::from(format!("{}.sig", dl.file.display()));
    let sig = std::fs::read_to_string(&sig_path).map_err(|e| format!("no signature for {}: {e}", dl.asset))?;
    let digest = sign::sha512_file(&dl.file).map_err(|e| format!("cannot read {}: {e}", dl.file.display()))?;
    sign::verify(key, &dl.asset, &dl.version, &digest, &sig)
}

pub struct Updater {
    inner: Arc<Inner>,
}

struct Inner {
    cfg: UpdateConfig,
    sink: EventSink,
    quit: Arc<dyn Fn() + Send + Sync>,
    kind: InstallKind,
    key: Result<VerifyingKey, String>,
    api: String,
    rt: tokio::runtime::Handle,
    state: Mutex<UpdaterState>,
    ready: Mutex<Option<Download>>,
    busy: AtomicBool,
    auto: AtomicBool,
    /// Bumped whenever the schedule changes; a timer that sees another value stops.
    schedule: AtomicU64,
    install_when_ready: AtomicBool,
}

/// The runtime the app entered, or one of our own (tests, tools).
fn runtime() -> tokio::runtime::Handle {
    static OWN: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
        OWN.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .thread_name("updater")
                .build()
                .expect("tokio runtime for the updater")
        })
        .handle()
        .clone()
    })
}

impl Updater {
    /// `quit` ends the app; called once the installer/helper has been handed the swap.
    pub fn start(config: UpdateConfig, sink: EventSink, quit: Arc<dyn Fn() + Send + Sync>) -> Arc<Updater> {
        let api = std::env::var(API_ENV).ok().filter(|s| !s.is_empty()).unwrap_or_else(|| API_BASE.into());
        let u = Self::with(config, sink, quit, kind::Probe::current(), sign::release_key(), api);
        u.set_auto(u.inner.cfg.auto);
        u
    }

    /// [`Updater::start`] with every outside input given, for tests. Schedules nothing.
    pub fn with(
        config: UpdateConfig,
        sink: EventSink,
        quit: Arc<dyn Fn() + Send + Sync>,
        kind: InstallKind,
        key: Result<VerifyingKey, String>,
        api: String,
    ) -> Arc<Updater> {
        swap::clean_leftovers(&config.data_dir);
        let mut state = UpdaterState {
            status: UpdaterStatus::Idle,
            current: config.current.clone(),
            release_url: Some(release::RELEASES_PAGE.into()),
            log_path: Some(config.data_dir.join("updater.log").display().to_string()),
            ..Default::default()
        };
        let off = match (&kind, &key) {
            (InstallKind::Dev, _) => Some("this is a development build; updates are off".to_string()),
            (_, Err(e)) => Some(e.clone()),
            _ => None,
        };
        if let Some(reason) = off {
            state.status = UpdaterStatus::Disabled;
            state.disabled_reason = Some(reason);
        } else if let Some(failure) = swap::take_failure(&config.data_dir) {
            state.status = UpdaterStatus::Error;
            state.error = Some(format!("the last update did not install: {failure}"));
        }
        if let InstallKind::Link { reason } = &kind {
            state.disabled_reason.get_or_insert_with(|| reason.clone());
        }
        let inner = Arc::new(Inner {
            cfg: config,
            sink,
            quit,
            kind,
            key,
            api,
            rt: runtime(),
            state: Mutex::new(state),
            ready: Mutex::new(None),
            busy: AtomicBool::new(false),
            auto: AtomicBool::new(false),
            schedule: AtomicU64::new(0),
            install_when_ready: AtomicBool::new(false),
        });
        Arc::new(Updater { inner })
    }

    pub fn kind(&self) -> &InstallKind {
        &self.inner.kind
    }

    /// Follows the `autoUpdate` setting: on = check in 15 s and every 6 h; off = no timer.
    pub fn set_auto(&self, on: bool) {
        let was = self.inner.auto.swap(on, Ordering::SeqCst);
        if was == on && on {
            return;
        }
        let generation = self.inner.schedule.fetch_add(1, Ordering::SeqCst) + 1;
        if !on || self.inner.disabled() {
            return;
        }
        let inner = self.inner.clone();
        self.inner.rt.spawn(async move {
            tokio::time::sleep(FIRST_CHECK).await;
            loop {
                if inner.schedule.load(Ordering::SeqCst) != generation {
                    return;
                }
                inner.check().await;
                tokio::time::sleep(CHECK_EVERY).await;
            }
        });
    }

    /// Quitting with an update downloaded installs it without restarting, as 0.1.x's
    /// `autoInstallOnAppQuit` did. Blocks for the hand-over; the app exits right after.
    pub fn install_on_quit(&self) {
        let dl = lock(&self.inner.ready).clone();
        if let (Some(dl), Ok(key)) = (dl, &self.inner.key)
            && self.inner.kind.can_swap()
            && verify_download(&dl, key).is_ok()
        {
            let r = swap::hand_over(&self.inner.kind, &dl, &self.inner.cfg.data_dir, false);
            self.inner.log(&format!("install on quit: {r:?}"));
        }
    }
}

impl Inner {
    fn disabled(&self) -> bool {
        matches!(lock(&self.state).status, UpdaterStatus::Disabled)
    }

    fn log(&self, msg: &str) {
        swap::log_line(&self.cfg.data_dir, msg);
    }

    fn update(&self, f: impl FnOnce(&mut UpdaterState)) {
        let snapshot = {
            let mut s = lock(&self.state);
            f(&mut s);
            s.clone()
        };
        self.sink.send(Event::Updater(snapshot));
    }

    fn fail(&self, msg: String) {
        self.log(&format!("[error] {msg}"));
        self.update(|s| {
            s.status = UpdaterStatus::Error;
            s.error = Some(msg);
            s.percent = None;
        });
    }

    /// One check, downloading a newer release when this copy can install it.
    async fn check(self: &Arc<Self>) {
        if self.disabled() || self.busy.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Err(e) = self.check_inner().await {
            self.fail(e);
        }
        self.busy.store(false, Ordering::SeqCst);
        if self.install_when_ready.swap(false, Ordering::SeqCst) && lock(&self.state).status == UpdaterStatus::Downloaded {
            self.install_now();
        }
    }

    async fn check_inner(self: &Arc<Self>) -> Result<(), String> {
        let downloaded = lock(&self.ready).clone();
        self.update(|s| {
            s.status = UpdaterStatus::Checking;
            s.error = None;
        });
        let client = client(&self.cfg.current)?;
        let url = format!("{}/repos/{}/releases/latest", self.api.trim_end_matches('/'), self.cfg.repo);
        let resp = client
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| format!("cannot reach GitHub: {e}"))?;
        match resp.status().as_u16() {
            200..=299 => {}
            404 => return Err("GitHub has no published release yet".into()),
            code => return Err(format!("GitHub answered HTTP {code}")),
        }
        let rel: release::Release = resp.json().await.map_err(|e| format!("unreadable release: {e}"))?;
        let latest = release::parse_version(&rel.tag_name).ok_or_else(|| format!("latest release tag {:?} is not a version", rel.tag_name))?;
        let current = release::parse_version(&self.cfg.current).ok_or("this build's version is not semver")?;
        let now = time::iso_now();
        let page = if rel.html_url.is_empty() { release::RELEASES_PAGE.to_string() } else { rel.html_url.clone() };
        if latest <= current {
            self.update(|s| {
                s.status = UpdaterStatus::NotAvailable;
                s.version = Some(latest.to_string());
                s.checked_at = Some(now);
            });
            return Ok(());
        }
        let version = latest.to_string();
        self.update(|s| {
            s.status = UpdaterStatus::Available;
            s.version = Some(version.clone());
            s.checked_at = Some(now);
            s.release_url = Some(page.clone());
            s.percent = None;
        });
        self.log(&format!("update available: {version}"));
        if let Some(dl) = downloaded.filter(|d| d.version == version) {
            *lock(&self.ready) = Some(dl);
            self.update(|s| {
                s.status = UpdaterStatus::Downloaded;
                s.percent = Some(100.0);
            });
            return Ok(());
        }
        let Some(name) = release::asset_name(&self.kind, std::env::consts::ARCH) else {
            return Ok(()); // the release page is the way (Link): Available + disabled_reason
        };
        let (asset, sig) = release::pick(&rel, &name)?;
        let key = self.key.clone()?;
        let dl = self.download(&client, asset, sig, &version, &key).await?;
        *lock(&self.ready) = Some(dl);
        self.update(|s| {
            s.status = UpdaterStatus::Downloaded;
            s.percent = Some(100.0);
        });
        self.log(&format!("update downloaded and verified: {version}"));
        Ok(())
    }

    async fn download(
        &self,
        client: &reqwest::Client,
        asset: &release::Asset,
        sig: &release::Asset,
        version: &str,
        key: &VerifyingKey,
    ) -> Result<Download, String> {
        let dir = swap::updates_dir(&self.cfg.data_dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        let file = dir.join(&asset.name);
        let sig_file = dir.join(&sig.name);
        let part = dir.join(format!("{}.part", asset.name));
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_file(&sig_file);

        let sig_text = get_ok(client, &sig.browser_download_url).await?.text().await.map_err(|e| format!("signature download failed: {e}"))?;
        let resp = get_ok(client, &asset.browser_download_url).await?;
        let total = resp.content_length().unwrap_or(asset.size);
        let digest = match self.stream_to(resp, &part, total).await {
            Ok(d) => d,
            Err(e) => {
                let _ = std::fs::remove_file(&part);
                return Err(e);
            }
        };
        if let Err(e) = sign::verify(key, &asset.name, version, &digest, &sig_text) {
            let _ = std::fs::remove_file(&part);
            return Err(format!("{e}; the download was deleted and nothing was run"));
        }
        std::fs::write(&sig_file, sig_text).map_err(|e| format!("cannot keep the signature: {e}"))?;
        std::fs::rename(&part, &file).map_err(|e| format!("cannot move the download into place: {e}"))?;
        Ok(Download { file, asset: asset.name.clone(), version: version.into() })
    }

    async fn stream_to(&self, mut resp: reqwest::Response, path: &Path, total: u64) -> Result<[u8; 64], String> {
        use tokio::io::AsyncWriteExt;
        let mut f = tokio::fs::File::create(path).await.map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        let mut h = Sha512::new();
        let (mut done, mut shown) = (0u64, -1i64);
        while let Some(chunk) = resp.chunk().await.map_err(|e| format!("download failed: {e}"))? {
            h.update(&chunk);
            f.write_all(&chunk).await.map_err(|e| format!("cannot write {}: {e}", path.display()))?;
            done += chunk.len() as u64;
            let pct = (done * 100).checked_div(total).map_or(0, |p| p.min(100) as i64);
            if pct != shown {
                shown = pct;
                self.update(|s| {
                    s.status = UpdaterStatus::Downloading;
                    s.percent = Some(pct as f32);
                });
            }
        }
        f.flush().await.map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        Ok(h.finalize().into())
    }

    /// Re-verifies and hands over; quits the app on success.
    fn install_now(&self) {
        let Some(dl) = lock(&self.ready).clone() else { return };
        let key = match &self.key {
            Ok(k) => *k,
            Err(e) => return self.fail(e.clone()),
        };
        if let Err(e) = verify_download(&dl, &key) {
            *lock(&self.ready) = None;
            let _ = std::fs::remove_file(&dl.file);
            return self.fail(format!("{e}; the download was deleted and nothing was run"));
        }
        match swap::hand_over(&self.kind, &dl, &self.cfg.data_dir, true) {
            Ok(()) => {
                self.log(&format!("handed over {} for {}", dl.asset, dl.version));
                (self.quit)();
            }
            Err(e) => self.fail(e),
        }
    }
}

impl Updates for Updater {
    fn state(&self) -> UpdaterState {
        lock(&self.inner.state).clone()
    }

    fn check(&self) -> Pending<UpdaterState> {
        let (fill, pending) = Pending::channel(self.inner.sink.waker());
        let inner = self.inner.clone();
        self.inner.rt.spawn(async move {
            inner.check().await;
            fill.fill(lock(&inner.state).clone());
        });
        pending
    }

    fn install(&self) {
        let st = self.state();
        match st.status {
            UpdaterStatus::Downloaded => {
                let inner = self.inner.clone();
                // Off the UI thread: hdiutil/ditto on macOS take seconds.
                std::thread::spawn(move || inner.install_now());
            }
            UpdaterStatus::Available if !self.inner.kind.can_swap() => {
                open_url(st.release_url.as_deref().unwrap_or(release::RELEASES_PAGE));
            }
            UpdaterStatus::Available | UpdaterStatus::Error | UpdaterStatus::NotAvailable | UpdaterStatus::Idle => {
                // "Downloads if needed": check (which downloads), then install once ready.
                self.inner.install_when_ready.store(true, Ordering::SeqCst);
                let inner = self.inner.clone();
                self.inner.rt.spawn(async move { inner.check().await });
            }
            UpdaterStatus::Checking | UpdaterStatus::Downloading => {
                self.inner.install_when_ready.store(true, Ordering::SeqCst);
            }
            UpdaterStatus::Disabled => {}
        }
    }
}

/// reqwest on rustls with the ring provider (see Cargo.toml). Follows redirects: release
/// downloads answer 302 to GitHub's object storage. No token is ever sent (public repo).
fn client(version: &str) -> Result<reqwest::Client, String> {
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().certs {
        let _ = roots.add(cert);
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut tls = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| format!("TLS setup failed: {e}"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    tls.alpn_protocols = vec![b"http/1.1".to_vec()];
    reqwest::Client::builder()
        .tls_backend_preconfigured(tls)
        .connect_timeout(Duration::from_secs(15))
        // Per read, not for the whole body: a slow line is fine, a stalled one is not.
        .read_timeout(Duration::from_secs(60))
        .user_agent(format!("claude-swap-desktop/{version}"))
        .build()
        .map_err(|e| format!("HTTP client setup failed: {e}"))
}

async fn get_ok(client: &reqwest::Client, url: &str) -> Result<reqwest::Response, String> {
    let resp = client.get(url).send().await.map_err(|e| format!("download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("{url} answered HTTP {}", resp.status().as_u16()));
    }
    Ok(resp)
}

/// The release page in the default browser (deb, translocated bundle, ...).
pub fn open_url(url: &str) {
    let opener = if cfg!(windows) {
        "explorer.exe"
    } else if cfg!(target_os = "macos") {
        "/usr/bin/open"
    } else {
        "xdg-open"
    };
    let mut cmd = std::process::Command::new(opener);
    cmd.arg(url);
    let _ = swap::spawn_detached(cmd);
}

/// When this process was started as the update helper, finishes the swap and returns its
/// outcome; `None` for a normal launch. The app calls this first thing in `main`.
pub fn finish(args: &[OsString]) -> Option<Result<(), String>> {
    // 0.1.x's electron-updater "installs" an AppImage by running the new file once with
    // this set and waiting for it to exit (AppImageUpdater.doInstall, install on quit).
    // Starting the whole app there would hang 0.1.x's quit until this copy is closed.
    if std::env::var("APPIMAGE_EXIT_AFTER_INSTALL").as_deref() == Ok("true") {
        return Some(Ok(()));
    }
    if args.get(1).is_some_and(|a| a == swap::HELPER_FLAG) {
        return Some(swap::run_helper(&args[2..]));
    }
    None
}

#[cfg(test)]
mod tests;
