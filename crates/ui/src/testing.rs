//! Fakes for tests, the demo and the README screenshots: a [`RecordingHost`] that answers
//! dialogs from a queue and records what the window asked of the platform, and
//! [`DisabledUpdates`], an updater that is off (dev builds) but still records calls.
//! Public behind the `testing` feature; always compiled into this crate's tests.
//!
//! [`StubCswap`] is a minimal in-memory `Cswap` used until ui-accounts' `MockCswap` is merged
//! into `v1`; then the tests and the demo move to the mock and the stub goes away.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use cswap_core::settings::SettingsEdit;
use cswap_core::*;

/// Everything the window asked of the platform, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostCall {
    PickDirectory(String),
    PickFile(String),
    SaveFile(String, String),
    OpenPath(String),
    OpenExternal(String),
    CloseWindow,
    Quit,
}

#[derive(Default)]
struct HostState {
    calls: Vec<HostCall>,
    answers: VecDeque<Option<String>>,
}

/// A [`Host`] that records calls and answers dialogs from [`RecordingHost::answer`] (a
/// dialog with no queued answer is "cancelled").
pub struct RecordingHost {
    s: Mutex<HostState>,
    pub version: String,
    pub platform: &'static str,
}

impl Default for RecordingHost {
    fn default() -> Self {
        RecordingHost { s: Mutex::default(), version: "1.0.0".into(), platform: "linux" }
    }
}

impl RecordingHost {
    pub fn new(platform: &'static str) -> Arc<Self> {
        Arc::new(RecordingHost { platform, ..Default::default() })
    }
    fn s(&self) -> MutexGuard<'_, HostState> {
        self.s.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Queues the answer of the next dialog (`None` = cancelled).
    pub fn answer(&self, a: Option<&str>) {
        self.s().answers.push_back(a.map(Into::into));
    }
    pub fn calls(&self) -> Vec<HostCall> {
        self.s().calls.clone()
    }
    fn dialog(&self, call: HostCall) -> Option<String> {
        let mut s = self.s();
        s.calls.push(call);
        s.answers.pop_front().flatten()
    }
}

impl Host for RecordingHost {
    fn pick_directory(&self, title: &str) -> Option<String> {
        self.dialog(HostCall::PickDirectory(title.into()))
    }
    fn pick_file(&self, title: &str, _filters: &[(&str, &[&str])]) -> Option<String> {
        self.dialog(HostCall::PickFile(title.into()))
    }
    fn save_file(&self, title: &str, default_name: &str) -> Option<String> {
        self.dialog(HostCall::SaveFile(title.into(), default_name.into()))
    }
    fn open_path(&self, path: &str) {
        self.s().calls.push(HostCall::OpenPath(path.into()));
    }
    fn open_external(&self, url: &str) {
        self.s().calls.push(HostCall::OpenExternal(url.into()));
    }
    fn app_version(&self) -> String {
        self.version.clone()
    }
    fn platform(&self) -> &'static str {
        self.platform
    }
    fn close_window(&self) {
        self.s().calls.push(HostCall::CloseWindow);
    }
    fn quit(&self) {
        self.s().calls.push(HostCall::Quit);
    }
}

/// An [`Updates`] whose state is whatever the test sets; `check` answers with it at once.
pub struct DisabledUpdates {
    state: Mutex<UpdaterState>,
    pub checks: Mutex<u32>,
    pub installs: Mutex<u32>,
}

impl DisabledUpdates {
    pub fn new(current: &str) -> Arc<Self> {
        Arc::new(DisabledUpdates {
            state: Mutex::new(UpdaterState {
                status: UpdaterStatus::Disabled,
                current: current.into(),
                release_url: Some("https://github.com/TheSkinnyRat/cswap-desktop/releases/latest".into()),
                disabled_reason: Some("Running from source or a dev build — use the release page.".into()),
                ..Default::default()
            }),
            checks: Mutex::new(0),
            installs: Mutex::new(0),
        })
    }
    pub fn set(&self, s: UpdaterState) {
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = s;
    }
}

impl Updates for DisabledUpdates {
    fn state(&self) -> UpdaterState {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    fn check(&self) -> Pending<UpdaterState> {
        *self.checks.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        Pending::ready(self.state())
    }
    fn install(&self) {
        *self.installs.lock().unwrap_or_else(|e| e.into_inner()) += 1;
    }
}

// ---- stub cswap (until MockCswap lands) ----------------------------------------------------

pub struct StubState {
    pub accounts: AccountsState,
    pub settings: AppSettings,
    pub binary: Option<CswapBinaryInfo>,
    /// What `detect_binary` finds next (defaults to `binary`).
    pub detect: Option<CswapBinaryInfo>,
    pub log: Vec<CommandLogEntry>,
    pub unclaimed: Vec<UnclaimedEntry>,
    pub auto: AutoStatus,
    /// Every call, as `verb args…`.
    pub calls: Vec<String>,
}

pub struct StubCswap {
    pub s: Mutex<StubState>,
    sink: Option<EventSink>,
}

fn ok_out(s: &str) -> CsResult<PlainOutput> {
    Ok(PlainOutput { stdout: s.into(), stderr: String::new(), exit_code: Some(0) })
}

fn win(pct: f64) -> UsageWindow {
    UsageWindow { pct, ..Default::default() }
}

impl StubCswap {
    /// The fixture of 0.1.23's README screenshots (`docs.spec.ts`).
    pub fn new(sink: Option<EventSink>) -> Arc<Self> {
        let acc = |n: u32, email: &str, alias: Option<&str>, org: &str, p5: Option<f64>, p7: Option<f64>| Account {
            number: n,
            email: email.into(),
            alias: alias.map(Into::into),
            organization_name: org.into(),
            is_organization: !org.is_empty(),
            active: n == 1,
            usage_status: "ok".into(),
            usage: p5.map(|p| Usage { five_hour: Some(win(p)), seven_day: p7.map(win), ..Default::default() }),
            ..Default::default()
        };
        let mut a4 = acc(4, "api-key-4@token.local", None, "", None, None);
        a4.usage_status = "api_key".into();
        let accounts = vec![
            acc(1, "you@example.com", Some("main"), "", Some(45.0), Some(12.0)),
            acc(2, "work@example.com", None, "Acme Inc", Some(100.0), Some(61.0)),
            acc(3, "spare@example.com", None, "", Some(3.0), Some(40.0)),
            a4,
        ];
        let binary = CswapBinaryInfo {
            path: Some("/home/you/.local/bin/cswap".into()),
            source: BinarySource::Uv,
            version: Some("0.25.0".into()),
            error: None,
            candidates: vec!["/home/you/.local/bin".into()],
        };
        let log = vec![
            log_entry(1, &["list", "--json"], true),
            log_entry(2, &["config", "get", "--json"], true),
            log_entry(3, &["switch", "9", "--json"], false),
        ];
        Arc::new(StubCswap {
            s: Mutex::new(StubState {
                accounts: AccountsState {
                    payload: Some(ListPayload {
                        schema_version: 1,
                        active_account_number: Some(1),
                        accounts,
                        ..Default::default()
                    }),
                    // now, so the title bar reads "just now" in every render
                    fetched_at: Some(now_iso()),
                    ..Default::default()
                },
                settings: AppSettings::default(),
                binary: Some(binary.clone()),
                detect: None,
                log,
                unclaimed: vec![],
                auto: AutoStatus::default(),
                calls: vec![],
            }),
            sink,
        })
    }

    pub fn st(&self) -> MutexGuard<'_, StubState> {
        self.s.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn call(&self, c: String) {
        self.st().calls.push(c);
    }

    fn send(&self, e: Event) {
        if let Some(s) = &self.sink {
            s.send(e);
        }
    }

    pub fn calls(&self) -> Vec<String> {
        self.st().calls.clone()
    }
}

/// The current time as RFC 3339 UTC.
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn log_entry(id: u64, args: &[&str], ok: bool) -> CommandLogEntry {
    CommandLogEntry {
        id,
        at: "2026-09-10T08:00:00Z".into(),
        args: args.iter().map(|s| (*s).to_owned()).collect(),
        exit_code: Some(if ok { 0 } else { 1 }),
        duration_ms: 120 + id * 37,
        ok,
        stdout: Some(if ok { "{\"schemaVersion\": 1}".into() } else { String::new() }),
        stderr: (!ok).then(|| "No account found with identifier: 9".into()),
        error: None,
    }
}

impl Cswap for StubCswap {
    fn accounts(&self) -> AccountsState {
        self.st().accounts.clone()
    }
    fn refresh(&self) -> Pending<AccountsState> {
        self.call("refresh".into());
        let a = self.accounts();
        self.send(Event::Accounts(a.clone()));
        Pending::ready(a)
    }
    fn status(&self) -> Pending<CsResult<StatusPayload>> {
        Pending::ready(Err(CswapError::new("Stub", "not in the stub")))
    }
    fn switch_to(&self, target: &str, _force: bool) -> Pending<CsResult<SwitchPayload>> {
        self.call(format!("switch {target}"));
        let mut s = self.st();
        let Some(p) = s.accounts.payload.as_mut() else {
            return Pending::ready(Err(CswapError::new("NotFound", "no list")));
        };
        let Some(n) = p.accounts.iter().find(|a| a.number.to_string() == target).map(|a| a.number) else {
            return Pending::ready(Err(CswapError::new(
                "AccountNotFoundError",
                format!("No account found with identifier: {target}"),
            )));
        };
        for a in &mut p.accounts {
            a.active = a.number == n;
        }
        p.active_account_number = Some(n);
        let email = p.accounts.iter().find(|a| a.number == n).map(|a| a.email.clone()).unwrap_or_default();
        let st = s.accounts.clone();
        drop(s);
        self.send(Event::Accounts(st));
        Pending::ready(Ok(SwitchPayload {
            schema_version: 1,
            switched: true,
            message: format!("Switched to Account-{n} ({email})"),
            ..Default::default()
        }))
    }
    fn rotate(&self, _strategy: Option<RotateStrategy>) -> Pending<CsResult<SwitchPayload>> {
        Pending::ready(Err(CswapError::new("Stub", "not in the stub")))
    }
    fn add_account(&self, _o: AddAccountOptions) -> Pending<CsResult<PlainOutput>> {
        Pending::ready(ok_out("ok"))
    }
    fn add_token(&self, _o: AddTokenOptions) -> Pending<CsResult<PlainOutput>> {
        Pending::ready(ok_out("ok"))
    }
    fn remove_account(&self, _t: &str) -> Pending<CsResult<PlainOutput>> {
        Pending::ready(ok_out("ok"))
    }
    fn set_disabled(&self, _t: &str, _d: bool) -> Pending<CsResult<PlainOutput>> {
        Pending::ready(ok_out("ok"))
    }
    fn set_alias(&self, _t: &str, _a: Option<&str>) -> Pending<CsResult<PlainOutput>> {
        Pending::ready(ok_out("ok"))
    }
    fn move_account(&self, _t: &str, _s: u32) -> Pending<CsResult<PlainOutput>> {
        Pending::ready(ok_out("ok"))
    }
    fn swap_accounts(&self, _a: &str, _b: &str) -> Pending<CsResult<PlainOutput>> {
        Pending::ready(ok_out("ok"))
    }
    fn list_mappings(&self) -> Pending<CsResult<Vec<Mapping>>> {
        Pending::ready(Ok(vec![]))
    }
    fn map_directory(&self, _t: &str, _p: &str) -> Pending<CsResult<PlainOutput>> {
        Pending::ready(ok_out("ok"))
    }
    fn unmap_directory(&self, _p: &str) -> Pending<CsResult<PlainOutput>> {
        Pending::ready(ok_out("ok"))
    }
    fn config(&self) -> Pending<CsResult<ConfigPayload>> {
        Pending::ready(Ok(ConfigPayload { schema_version: 1, ..Default::default() }))
    }
    fn set_config(&self, _k: &str, _v: Option<&str>) -> Pending<CsResult<PlainOutput>> {
        Pending::ready(ok_out("ok"))
    }
    fn export_accounts(&self, _o: ExportOptions, path: &str) -> Pending<CsResult<Transfer>> {
        Pending::ready(Ok(Transfer { path: path.into(), output: PlainOutput::default() }))
    }
    fn import_accounts(&self, _o: ImportOptions, path: &str) -> Pending<CsResult<Transfer>> {
        Pending::ready(Ok(Transfer { path: path.into(), output: PlainOutput::default() }))
    }
    fn list_unclaimed(&self) -> Pending<CsResult<Vec<UnclaimedEntry>>> {
        Pending::ready(Ok(self.st().unclaimed.clone()))
    }
    fn purge_unclaimed(&self, id: &str) -> Pending<CsResult<PlainOutput>> {
        self.call(format!("purge {id}"));
        self.st().unclaimed.retain(|u| u.id != id);
        Pending::ready(ok_out("ok"))
    }
    fn upgrade_cswap(&self) -> Pending<CsResult<PlainOutput>> {
        self.call("upgrade".into());
        Pending::ready(ok_out("Updated claude-swap\nclaude-swap is already at the latest version"))
    }
    fn token_status(&self) -> Pending<CsResult<PlainOutput>> {
        Pending::ready(ok_out(""))
    }
    fn launch_session(&self, _t: &str, _cwd: Option<&str>) -> Pending<CsResult<LaunchedSession>> {
        Pending::ready(Err(CswapError::new("Stub", "not in the stub")))
    }
    fn auto_start(&self, _dry_run: bool) -> Pending<AutoStatus> {
        Pending::ready(self.st().auto.clone())
    }
    fn auto_stop(&self) -> Pending<AutoStatus> {
        Pending::ready(self.st().auto.clone())
    }
    fn auto_status(&self) -> AutoStatus {
        self.st().auto.clone()
    }
    fn auto_events(&self) -> Vec<AutoEvent> {
        vec![]
    }
    fn auto_once(&self, _dry_run: bool) -> Pending<CsResult<AutoOnceResult>> {
        Pending::ready(Err(CswapError::new("Stub", "not in the stub")))
    }
    fn settings(&self) -> AppSettings {
        self.st().settings.clone()
    }
    fn edit_settings(&self, edit: SettingsEdit) -> AppSettings {
        let s = {
            let mut st = self.st();
            edit(&mut st.settings);
            st.settings.clone()
        };
        self.call("edit_settings".into());
        self.send(Event::Settings(s.clone()));
        s
    }
    fn binary(&self) -> Option<CswapBinaryInfo> {
        self.st().binary.clone()
    }
    fn detect_binary(&self) -> Pending<CswapBinaryInfo> {
        self.call("detect".into());
        let b = {
            let mut s = self.st();
            let b = s.detect.clone().or_else(|| s.binary.clone()).unwrap_or_default();
            s.binary = Some(b.clone());
            b
        };
        self.send(Event::Binary(b.clone()));
        Pending::ready(b)
    }
    fn install_cswap(&self) -> Pending<CsResult<PlainOutput>> {
        self.call("install".into());
        Pending::ready(Err(CswapError::new("InstallError", "uv was not found on PATH")))
    }
    fn command_log(&self) -> Vec<CommandLogEntry> {
        self.st().log.clone()
    }
    fn clear_command_log(&self) {
        self.call("clear_log".into());
        self.st().log.clear();
    }
    fn vault_path(&self) -> Option<String> {
        Some("/home/you/.claude-swap-backup".into())
    }
}
