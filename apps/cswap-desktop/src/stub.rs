//! `--features stub-driver`: a small in-memory cswap, so the tray, the window's lifetime
//! and the updater can be tried before the real driver is merged. Not shipped.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use cswap_core::settings::SettingsEdit;
use cswap_core::*;

pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("CSWAP_DESKTOP_USERDATA").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    std::env::temp_dir().join("cswap-desktop-stub")
}

pub struct StubCswap {
    sink: EventSink,
    state: Mutex<(AccountsState, AutoStatus, AppSettings)>,
}

fn unsupported<T>() -> Pending<CsResult<T>> {
    Pending::ready(Err(CswapError::new("Stub", "not available in the stub driver")))
}

fn account(n: u32, email: &str, active: bool, five: f64, seven: f64) -> Account {
    Account {
        number: n,
        email: email.into(),
        active,
        usage_status: "ok".into(),
        usage: Some(Usage {
            five_hour: Some(UsageWindow { pct: five, ..Default::default() }),
            seven_day: Some(UsageWindow { pct: seven, ..Default::default() }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

impl StubCswap {
    pub fn start(data_dir: PathBuf, sink: EventSink) -> Arc<dyn Cswap> {
        // The settings a test put in the data dir (closeToTray, autoUpdate, ...), read the
        // tolerant way the real store does.
        let settings: AppSettings = std::fs::read_to_string(data_dir.join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let payload = ListPayload {
            schema_version: 1,
            active_account_number: Some(1),
            accounts: vec![
                account(1, "alice@example.com", true, 12.0, 40.0),
                account(2, "bob@example.com", false, 80.0, 55.0),
            ],
            ..Default::default()
        };
        let accounts = AccountsState { payload: Some(payload), fetched_at: Some("now".into()), ..Default::default() };
        let s =
            Arc::new(StubCswap { sink, state: Mutex::new((accounts, AutoStatus::default(), settings)) });
        let st = s.state.lock().unwrap().0.clone();
        s.sink.send(Event::Accounts(st));
        s
    }

    fn push_accounts(&self) -> AccountsState {
        let st = self.state.lock().unwrap().0.clone();
        self.sink.send(Event::Accounts(st.clone()));
        st
    }

    fn set_auto(&self, running: bool, dry: bool) -> AutoStatus {
        let st =
            AutoStatus { running, dry_run: running.then_some(dry), pid: running.then_some(1), ..Default::default() };
        self.state.lock().unwrap().1 = st.clone();
        self.sink.send(Event::AutoStatus(st.clone()));
        st
    }
}

impl Cswap for StubCswap {
    fn accounts(&self) -> AccountsState {
        self.state.lock().unwrap().0.clone()
    }
    fn refresh(&self) -> Pending<AccountsState> {
        Pending::ready(self.push_accounts())
    }
    fn status(&self) -> Pending<CsResult<StatusPayload>> {
        unsupported()
    }
    fn switch_to(&self, target: &str, _force: bool) -> Pending<CsResult<SwitchPayload>> {
        let n: u32 = target.parse().unwrap_or(0);
        {
            let mut g = self.state.lock().unwrap();
            if let Some(p) = g.0.payload.as_mut() {
                for a in &mut p.accounts {
                    a.active = a.number == n;
                }
                p.active_account_number = Some(n);
            }
        }
        self.push_accounts();
        Pending::ready(Ok(SwitchPayload { schema_version: 1, switched: true, ..Default::default() }))
    }
    fn rotate(&self, _strategy: Option<RotateStrategy>) -> Pending<CsResult<SwitchPayload>> {
        let next = {
            let g = self.state.lock().unwrap();
            let accts = g.0.payload.as_ref().map(|p| p.accounts.clone()).unwrap_or_default();
            let i = accts.iter().position(|a| a.active).unwrap_or(0);
            accts.get((i + 1) % accts.len().max(1)).map(|a| a.number).unwrap_or(1)
        };
        self.switch_to(&next.to_string(), false)
    }
    fn add_account(&self, _opts: AddAccountOptions) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn add_token(&self, _opts: AddTokenOptions) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn remove_account(&self, _target: &str) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn set_disabled(&self, _target: &str, _disabled: bool) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn set_alias(&self, _target: &str, _alias: Option<&str>) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn move_account(&self, _target: &str, _slot: u32) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn swap_accounts(&self, _a: &str, _b: &str) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn list_mappings(&self) -> Pending<CsResult<Vec<Mapping>>> {
        Pending::ready(Ok(vec![]))
    }
    fn map_directory(&self, _target: &str, _path: &str) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn unmap_directory(&self, _path: &str) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn config(&self) -> Pending<CsResult<ConfigPayload>> {
        unsupported()
    }
    fn set_config(&self, _key: &str, _value: Option<&str>) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn export_accounts(&self, _opts: ExportOptions, _path: &str) -> Pending<CsResult<Transfer>> {
        unsupported()
    }
    fn import_accounts(&self, _opts: ImportOptions, _path: &str) -> Pending<CsResult<Transfer>> {
        unsupported()
    }
    fn list_unclaimed(&self) -> Pending<CsResult<Vec<UnclaimedEntry>>> {
        Pending::ready(Ok(vec![]))
    }
    fn purge_unclaimed(&self, _id: &str) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn upgrade_cswap(&self) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn token_status(&self) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn launch_session(&self, _target: &str, _cwd: Option<&str>) -> Pending<CsResult<LaunchedSession>> {
        unsupported()
    }
    fn auto_start(&self, dry_run: bool) -> Pending<AutoStatus> {
        Pending::ready(self.set_auto(true, dry_run))
    }
    fn auto_stop(&self) -> Pending<AutoStatus> {
        Pending::ready(self.set_auto(false, false))
    }
    fn auto_status(&self) -> AutoStatus {
        self.state.lock().unwrap().1.clone()
    }
    fn auto_events(&self) -> Vec<AutoEvent> {
        vec![]
    }
    fn auto_once(&self, _dry_run: bool) -> Pending<CsResult<AutoOnceResult>> {
        unsupported()
    }
    fn settings(&self) -> AppSettings {
        self.state.lock().unwrap().2.clone()
    }
    fn edit_settings(&self, edit: SettingsEdit) -> AppSettings {
        let s = {
            let mut g = self.state.lock().unwrap();
            edit(&mut g.2);
            g.2.clone()
        };
        self.sink.send(Event::Settings(s.clone()));
        s
    }
    fn binary(&self) -> Option<CswapBinaryInfo> {
        Some(CswapBinaryInfo {
            path: Some("stub".into()),
            source: BinarySource::Env,
            version: Some("stub".into()),
            ..Default::default()
        })
    }
    fn detect_binary(&self) -> Pending<CswapBinaryInfo> {
        Pending::ready(self.binary().unwrap_or_default())
    }
    fn install_cswap(&self) -> Pending<CsResult<PlainOutput>> {
        unsupported()
    }
    fn command_log(&self) -> Vec<CommandLogEntry> {
        vec![]
    }
    fn clear_command_log(&self) {}
    fn vault_path(&self) -> Option<String> {
        None
    }
}
