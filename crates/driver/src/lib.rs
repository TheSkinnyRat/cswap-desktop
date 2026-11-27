//! The real [`cswap_core::Cswap`]: everything goes through the cswap CLI (`--json` schema
//! v1), never by writing cswap's vault. The only files read directly are `mappings.json`,
//! the vault root (watched, so terminal/TUI changes show up live) and two non-secret
//! strings of Claude Code's `.credentials.json` (`subscriptionType`, `rateLimitTier`).
//! cswap is NOT bundled: two cswap versions writing one vault lose accounts silently.
//!
//! CONTRACT: [`DriverConfig`], [`Native::start`], [`Native::shutdown`], [`data_dir`].
//! OWNER: driver.
//!
//! Threads: `Native` runs its work on the tokio runtime it is started in (the app's one
//! runtime, `Handle::current()`), or on a small runtime of its own when started outside
//! one (tests, examples). Every trait method returns at once.

mod auto;
mod cli;
mod plans;
mod proc;
mod resolve;
mod session;
mod store;
mod time;
mod vault;

#[cfg(test)]
mod driver_tests;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use cswap_core::api::Fill;
use cswap_core::settings::SettingsEdit;
use cswap_core::*;
use tokio::runtime::{Handle, Runtime};

use crate::auto::AutoRunner;
use crate::cli::Cli;
use crate::plans::PlanStore;
use crate::store::SettingsStore;

#[derive(Debug, Clone, Default)]
pub struct DriverConfig {
    /// The app's data dir ([`data_dir`]): `settings.json`, `plans.json`.
    pub data_dir: PathBuf,
}

/// `CSWAP_DESKTOP_USERDATA` if set (tests), else `<appData>/cswap-desktop`: the same
/// folder 0.1.x used (`%APPDATA%`, `~/Library/Application Support`, `$XDG_CONFIG_HOME`
/// or `~/.config`), pinned so renaming the product never moves anyone's settings.
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("CSWAP_DESKTOP_USERDATA").filter(|v| !v.is_empty()) {
        return PathBuf::from(d);
    }
    app_data().join("cswap-desktop")
}

fn app_data() -> PathBuf {
    let home = resolve::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let nonempty = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        nonempty("APPDATA").unwrap_or_else(|| home.join("AppData").join("Roaming"))
    } else if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support")
    } else {
        nonempty("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"))
    }
}

/// What the environment decides, read once at start (and set directly by tests, which
/// must never touch the process environment of parallel tests).
#[derive(Debug, Clone, Default)]
pub(crate) struct Options {
    /// Extra environment for every cswap child.
    pub extra_env: Vec<(String, String)>,
    /// `CSWAP_DESKTOP_BIN`.
    pub bin_override: Option<String>,
    /// Where Claude Code's `.credentials.json` lives (`CLAUDE_CONFIG_DIR` or `~/.claude`).
    pub claude_config_dir: Option<PathBuf>,
    /// `CSWAP_DESKTOP_TEST_NO_SPAWN`: never open a terminal.
    pub no_spawn: bool,
}

impl Options {
    fn from_env() -> Self {
        Self {
            extra_env: Vec::new(),
            bin_override: std::env::var("CSWAP_DESKTOP_BIN").ok().filter(|v| !v.is_empty()),
            claude_config_dir: plans::claude_config_dir(),
            no_spawn: std::env::var_os("CSWAP_DESKTOP_TEST_NO_SPAWN").is_some_and(|v| !v.is_empty()),
        }
    }
}

struct Inner {
    rt: Handle,
    sink: EventSink,
    opts: Options,
    cli: Arc<Cli>,
    auto: Arc<AutoRunner>,
    settings: Mutex<SettingsStore>,
    plans: Mutex<PlanStore>,
    accounts: Mutex<AccountsState>,
    /// `Some` while a list is in flight: callers arriving meanwhile share its result.
    refresh_waiters: Mutex<Option<Vec<Fill<AccountsState>>>>,
    binary: Mutex<Option<CswapBinaryInfo>>,
    vault_path: Mutex<Option<PathBuf>>,
    vault_watch: Mutex<Option<vault::VaultWatch>>,
    timer: Mutex<Option<tokio::task::JoinHandle<()>>>,
    stopped: AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Native {
    inner: Arc<Inner>,
    /// Only when started outside a runtime.
    own_rt: Mutex<Option<Runtime>>,
}

impl Native {
    /// Loads settings, starts binary detection and the first refresh, the refresh timer and
    /// the vault watcher, and (when the setting says so) the auto-switch process. Returns at
    /// once; results arrive through `sink`.
    pub fn start(config: DriverConfig, sink: EventSink) -> Arc<Native> {
        Self::start_with(config, sink, Options::from_env())
    }

    pub(crate) fn start_with(config: DriverConfig, sink: EventSink, opts: Options) -> Arc<Native> {
        let (rt, own_rt) = match Handle::try_current() {
            Ok(h) => (h, None),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .thread_name("cswap-driver")
                    .enable_all()
                    .build()
                    .expect("tokio runtime");
                (rt.handle().clone(), Some(rt))
            }
        };
        let inner = Arc::new_cyclic(|weak: &Weak<Inner>| {
            let w = weak.clone();
            // Events that change the list: re-read it so the page follows the switch.
            let on_event: auto::OnEvent = Arc::new(move |ev: &AutoEvent| {
                if matches!(ev.event.as_str(), "switch" | "account-quarantined" | "all-exhausted")
                    && let Some(i) = w.upgrade()
                {
                    i.refresh(None);
                }
            });
            Inner {
                cli: Cli::new(&rt, Some(sink.clone()), opts.extra_env.clone()),
                auto: AutoRunner::new(rt.clone(), Some(sink.clone()), Some(on_event)),
                settings: Mutex::new(SettingsStore::open(&config.data_dir)),
                plans: Mutex::new(PlanStore::open(&config.data_dir, opts.claude_config_dir.clone())),
                accounts: Mutex::new(AccountsState::default()),
                refresh_waiters: Mutex::new(None),
                binary: Mutex::new(None),
                vault_path: Mutex::new(None),
                vault_watch: Mutex::new(None),
                timer: Mutex::new(None),
                stopped: AtomicBool::new(false),
                rt: rt.clone(),
                sink,
                opts,
            }
        });
        let i = inner.clone();
        rt.spawn(async move { i.init().await });
        Arc::new(Native { inner, own_rt: Mutex::new(own_rt) })
    }

    /// Stops the auto-switch child and the timers. Called once before the app exits.
    pub fn shutdown(&self) {
        let i = &self.inner;
        i.stopped.store(true, Ordering::SeqCst);
        if let Some(t) = lock(&i.timer).take() {
            t.abort();
        }
        lock(&i.vault_watch).take();
        i.auto.stop_blocking(Duration::from_secs(3));
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        // Never blocks, and is allowed even from inside an async context.
        if let Some(rt) = lock(&self.own_rt).take() {
            rt.shutdown_background();
        }
    }
}

impl Inner {
    fn waker(&self) -> Option<Waker> {
        let sink = self.sink.clone();
        Some(Arc::new(move || {
            if let Some(w) = sink.waker() {
                w()
            }
        }))
    }

    fn spawn_pending<T: Send + 'static>(&self, fut: impl Future<Output = T> + Send + 'static) -> Pending<T> {
        let (fill, pending) = Pending::channel(self.waker());
        self.rt.spawn(async move { fill.fill(fut.await) });
        pending
    }

    async fn init(self: Arc<Self>) {
        let info = self.detect().await;
        self.schedule_timer();
        if info.path.is_some() {
            self.refresh(None);
            let s = lock(&self.settings).get();
            if s.auto_start_auto_switch {
                self.auto.start(self.cli.binary(), self.cli.env(), s.auto_dry_run);
            }
        }
    }

    async fn detect(self: &Arc<Self>) -> CswapBinaryInfo {
        let path = lock(&self.settings).get().cswap_path;
        let info = resolve::resolve_cswap(path.as_deref(), self.opts.bin_override.as_deref(), &self.cli.env()).await;
        self.cli.set_binary(info.version.as_ref().and(info.path.as_ref()).map(PathBuf::from));
        self.resolve_vault().await;
        *lock(&self.binary) = Some(info.clone());
        // The window may have opened before detection finished: push, don't wait to be asked.
        self.sink.send(Event::Binary(info.clone()));
        info
    }

    async fn resolve_vault(self: &Arc<Self>) {
        let mut vault = if self.cli.binary().is_some() {
            vault::vault_from_settings_path(self.cli.config_path().await.as_deref())
        } else {
            None
        };
        if !vault.as_deref().is_some_and(Path::exists) {
            let def = vault::default_vault_path().filter(|d| d.exists());
            vault = def.or(vault);
        }
        let weak = Arc::downgrade(self);
        let watch = if self.stopped.load(Ordering::SeqCst) {
            None
        } else {
            vault.as_deref().and_then(|v| {
                vault::watch(&self.rt, v, move || {
                    if let Some(i) = weak.upgrade() {
                        i.refresh(None);
                    }
                })
            })
        };
        *lock(&self.vault_watch) = watch;
        *lock(&self.vault_path) = vault;
    }

    fn emit_accounts(&self, st: &AccountsState) {
        self.sink.send(Event::Accounts(st.clone()));
    }

    /// One `list --json` at a time; a caller arriving meanwhile gets the same result.
    fn refresh(self: &Arc<Self>, fill: Option<Fill<AccountsState>>) {
        if self.cli.binary().is_none() {
            let st = {
                let mut a = lock(&self.accounts);
                a.refreshing = false;
                a.error = Some(CswapError::new("NotFound", "cswap is not installed or not found"));
                a.clone()
            };
            self.emit_accounts(&st);
            if let Some(f) = fill {
                f.fill(st);
            }
            return;
        }
        {
            let mut w = lock(&self.refresh_waiters);
            if let Some(v) = w.as_mut() {
                v.extend(fill);
                return;
            }
            *w = Some(fill.into_iter().collect());
        }
        let st = {
            let mut a = lock(&self.accounts);
            a.refreshing = true;
            a.clone()
        };
        self.emit_accounts(&st);
        let me = self.clone();
        self.rt.spawn(async move {
            let r = me.cli.list().await;
            let plans = {
                let mut p = lock(&me.plans);
                if let Ok(list) = &r {
                    p.observe(list.accounts.iter().find(|a| a.active).map(|a| a.email.as_str()));
                }
                p.all()
            };
            let st = {
                let mut a = lock(&me.accounts);
                match r {
                    Ok(payload) => {
                        *a = AccountsState {
                            payload: Some(payload),
                            fetched_at: Some(time::now_iso()),
                            refreshing: false,
                            error: None,
                            plans,
                        }
                    }
                    Err(e) => {
                        a.refreshing = false;
                        a.error = Some(e);
                        a.plans = plans;
                    }
                }
                a.clone()
            };
            let waiters = lock(&me.refresh_waiters).take().unwrap_or_default();
            me.emit_accounts(&st);
            for f in waiters {
                f.fill(st.clone());
            }
        });
    }

    fn schedule_timer(self: &Arc<Self>) {
        let mut t = lock(&self.timer);
        if let Some(old) = t.take() {
            old.abort();
        }
        let secs = lock(&self.settings).get().refresh_seconds;
        // 0 = never; manual, focus and vault changes still refresh.
        if secs == 0 || self.stopped.load(Ordering::SeqCst) {
            return;
        }
        let period = Duration::from_secs(u64::from(secs.max(15)));
        let weak = Arc::downgrade(self);
        *t = Some(self.rt.spawn(async move {
            let mut iv = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
            iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                iv.tick().await;
                match weak.upgrade() {
                    Some(i) => i.refresh(None),
                    None => return,
                }
            }
        }));
    }

    /// Queued behind earlier mutations; `refresh_after` re-reads the list once it is done
    /// so the UI never shows a stale row.
    fn mutate<T: Send + 'static>(
        self: &Arc<Self>,
        refresh_after: bool,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> Pending<T> {
        let job = self.cli.serial(fut);
        let me = self.clone();
        let (fill, pending) = Pending::channel(self.waker());
        self.rt.spawn(async move {
            let Some(r) = job.await else { return };
            if refresh_after {
                me.refresh(None);
            }
            fill.fill(r);
        });
        pending
    }

    async fn list_mappings(&self) -> CsResult<Vec<Mapping>> {
        let accounts = lock(&self.accounts).payload.as_ref().map(|p| p.accounts.clone()).unwrap_or_default();
        let by_email = |email: &str, org: Option<&str>| {
            accounts
                .iter()
                .find(|a| a.email == email && org.is_none_or(|o| o.is_empty() || a.organization_uuid == o))
                .or_else(|| accounts.iter().find(|a| a.email == email))
                .cloned()
        };
        let vault = lock(&self.vault_path).clone();
        if let Some(file) = vault.as_deref().and_then(vault::read_mappings_file) {
            // BTreeMap: already sorted by path.
            return Ok(file
                .into_iter()
                .map(|(path, v)| {
                    let email = v.email.unwrap_or_default();
                    let account = by_email(&email, v.organization_uuid.as_deref());
                    Mapping { path, email, organization_uuid: v.organization_uuid, account }
                })
                .collect());
        }
        let out = self.cli.list_mappings_text().await?;
        Ok(cli::parse_mappings(&out.stdout)
            .into_iter()
            .map(|m| Mapping {
                account: by_email(&m.email, None),
                path: m.path,
                email: m.email,
                organization_uuid: None,
            })
            .collect())
    }

    async fn install_cswap(self: &Arc<Self>) -> CsResult<PlainOutput> {
        let Some(uv) = resolve::find_uv() else {
            return Err(CswapError::new(
                "NotFound",
                "uv was not found. Install uv first (https://docs.astral.sh/uv/), then retry.",
            ));
        };
        let args: Vec<String> = ["tool", "install", "claude-swap"].map(str::to_owned).to_vec();
        let env = self.cli.env();
        let out = proc::run(proc::Spec {
            bin: &uv,
            args: &args,
            env: &env,
            stdin: None,
            timeout: Duration::from_secs(300),
            cwd: None,
        })
        .await;
        if out.spawn_error.is_some() || out.exit_code != Some(0) {
            let msg = if !out.stderr.trim().is_empty() {
                out.stderr.trim().to_owned()
            } else {
                out.spawn_error.unwrap_or_else(|| format!("uv exited with {:?}", out.exit_code))
            };
            return Err(CswapError::new("InstallError", msg));
        }
        self.detect().await;
        self.refresh(None);
        Ok(PlainOutput { stdout: out.stdout, stderr: out.stderr, exit_code: Some(0) })
    }
}

macro_rules! cli_call {
    ($self:ident, $refresh:expr, |$cli:ident| $body:expr) => {{
        let $cli = $self.inner.cli.clone();
        $self.inner.mutate($refresh, async move { $body.await })
    }};
}

impl Cswap for Native {
    fn accounts(&self) -> AccountsState {
        lock(&self.inner.accounts).clone()
    }
    fn refresh(&self) -> Pending<AccountsState> {
        let (fill, pending) = Pending::channel(self.inner.waker());
        self.inner.refresh(Some(fill));
        pending
    }
    fn status(&self) -> Pending<CsResult<StatusPayload>> {
        let cli = self.inner.cli.clone();
        self.inner.spawn_pending(async move { cli.status().await })
    }

    fn switch_to(&self, target: &str, force: bool) -> Pending<CsResult<SwitchPayload>> {
        let t = target.to_owned();
        cli_call!(self, true, |cli| cli.switch_to(&t, force))
    }
    fn rotate(&self, strategy: Option<RotateStrategy>) -> Pending<CsResult<SwitchPayload>> {
        cli_call!(self, true, |cli| cli.rotate(strategy))
    }
    fn add_account(&self, opts: AddAccountOptions) -> Pending<CsResult<PlainOutput>> {
        cli_call!(self, true, |cli| cli.add_account(&opts))
    }
    fn add_token(&self, opts: AddTokenOptions) -> Pending<CsResult<PlainOutput>> {
        cli_call!(self, true, |cli| cli.add_token(&opts))
    }
    fn remove_account(&self, target: &str) -> Pending<CsResult<PlainOutput>> {
        let t = target.to_owned();
        cli_call!(self, true, |cli| cli.remove_account(&t))
    }
    fn set_disabled(&self, target: &str, disabled: bool) -> Pending<CsResult<PlainOutput>> {
        let t = target.to_owned();
        cli_call!(self, true, |cli| cli.set_disabled(&t, disabled))
    }
    fn set_alias(&self, target: &str, alias: Option<&str>) -> Pending<CsResult<PlainOutput>> {
        let (t, a) = (target.to_owned(), alias.map(str::to_owned));
        cli_call!(self, true, |cli| cli.set_alias(&t, a.as_deref()))
    }
    fn move_account(&self, target: &str, slot: u32) -> Pending<CsResult<PlainOutput>> {
        let t = target.to_owned();
        cli_call!(self, true, |cli| cli.move_account(&t, slot))
    }
    fn swap_accounts(&self, a: &str, b: &str) -> Pending<CsResult<PlainOutput>> {
        let (a, b) = (a.to_owned(), b.to_owned());
        cli_call!(self, true, |cli| cli.swap_accounts(&a, &b))
    }

    fn list_mappings(&self) -> Pending<CsResult<Vec<Mapping>>> {
        let i = self.inner.clone();
        self.inner.spawn_pending(async move { i.list_mappings().await })
    }
    fn map_directory(&self, target: &str, path: &str) -> Pending<CsResult<PlainOutput>> {
        let (t, p) = (target.to_owned(), path.to_owned());
        cli_call!(self, false, |cli| cli.map_directory(&t, &p))
    }
    fn unmap_directory(&self, path: &str) -> Pending<CsResult<PlainOutput>> {
        let p = path.to_owned();
        cli_call!(self, false, |cli| cli.unmap_directory(&p))
    }

    fn config(&self) -> Pending<CsResult<ConfigPayload>> {
        let cli = self.inner.cli.clone();
        self.inner.spawn_pending(async move { cli.config().await })
    }
    fn set_config(&self, key: &str, value: Option<&str>) -> Pending<CsResult<PlainOutput>> {
        let (k, v) = (key.to_owned(), value.map(str::to_owned));
        cli_call!(self, false, |cli| cli.set_config(&k, v.as_deref()))
    }

    fn export_accounts(&self, opts: ExportOptions, path: &str) -> Pending<CsResult<Transfer>> {
        let p = path.to_owned();
        let cli = self.inner.cli.clone();
        self.inner.mutate(false, async move {
            cli.export_accounts(&p, &opts).await.map(|output| Transfer { path: p.clone(), output })
        })
    }
    fn import_accounts(&self, opts: ImportOptions, path: &str) -> Pending<CsResult<Transfer>> {
        let p = path.to_owned();
        let cli = self.inner.cli.clone();
        self.inner.mutate(true, async move {
            cli.import_accounts(&p, &opts).await.map(|output| Transfer { path: p.clone(), output })
        })
    }

    fn list_unclaimed(&self) -> Pending<CsResult<Vec<UnclaimedEntry>>> {
        let cli = self.inner.cli.clone();
        self.inner.spawn_pending(async move { cli.unclaimed_text().await.map(|o| cli::parse_unclaimed(&o.stdout)) })
    }
    fn purge_unclaimed(&self, id: &str) -> Pending<CsResult<PlainOutput>> {
        let id = id.to_owned();
        cli_call!(self, false, |cli| cli.purge_unclaimed(&id))
    }
    fn upgrade_cswap(&self) -> Pending<CsResult<PlainOutput>> {
        let i = self.inner.clone();
        self.inner.mutate(false, async move {
            let r = i.cli.upgrade().await;
            // The version (and possibly the vault) may have changed under us.
            i.detect().await;
            i.refresh(None);
            r
        })
    }
    fn token_status(&self) -> Pending<CsResult<PlainOutput>> {
        let cli = self.inner.cli.clone();
        self.inner.spawn_pending(async move { cli.token_status().await })
    }
    fn launch_session(&self, target: &str, cwd: Option<&str>) -> Pending<CsResult<LaunchedSession>> {
        let bin = self.inner.cli.binary();
        Pending::ready(session::launch_session(bin.as_deref(), target, cwd, self.inner.opts.no_spawn))
    }

    fn auto_start(&self, dry_run: bool) -> Pending<AutoStatus> {
        // Remembered, so autostart and the tray's Start use the mode last chosen.
        self.edit_settings(Box::new(move |s| s.auto_dry_run = dry_run));
        let i = &self.inner;
        Pending::ready(i.auto.start(i.cli.binary(), i.cli.env(), dry_run))
    }
    fn auto_stop(&self) -> Pending<AutoStatus> {
        let auto = self.inner.auto.clone();
        self.inner.spawn_pending(async move { auto.stop().await })
    }
    fn auto_status(&self) -> AutoStatus {
        self.inner.auto.status()
    }
    fn auto_events(&self) -> Vec<AutoEvent> {
        self.inner.auto.events()
    }
    fn auto_once(&self, dry_run: bool) -> Pending<CsResult<AutoOnceResult>> {
        cli_call!(self, true, |cli| cli.auto_once(dry_run))
    }

    fn settings(&self) -> AppSettings {
        lock(&self.inner.settings).get()
    }
    fn edit_settings(&self, edit: SettingsEdit) -> AppSettings {
        let i = &self.inner;
        let (before, after) = {
            let mut s = lock(&i.settings);
            let before = s.get();
            (before, s.edit(edit))
        };
        i.sink.send(Event::Settings(after.clone()));
        if before.refresh_seconds != after.refresh_seconds {
            i.schedule_timer();
        }
        if before.cswap_path != after.cswap_path {
            let i = i.clone();
            i.rt.clone().spawn(async move {
                i.detect().await;
                i.refresh(None);
            });
        }
        after
    }
    fn binary(&self) -> Option<CswapBinaryInfo> {
        lock(&self.inner.binary).clone()
    }
    fn detect_binary(&self) -> Pending<CswapBinaryInfo> {
        let i = self.inner.clone();
        self.inner.spawn_pending(async move {
            let info = i.detect().await;
            i.refresh(None);
            info
        })
    }
    fn install_cswap(&self) -> Pending<CsResult<PlainOutput>> {
        let i = self.inner.clone();
        self.inner.spawn_pending(async move { i.install_cswap().await })
    }
    fn command_log(&self) -> Vec<CommandLogEntry> {
        self.inner.cli.log()
    }
    fn clear_command_log(&self) {
        self.inner.cli.clear_log()
    }
    fn vault_path(&self) -> Option<String> {
        lock(&self.inner.vault_path).as_ref().map(|p| p.display().to_string())
    }
}
