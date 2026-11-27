//! In-memory [`crate::Cswap`] with the fixture accounts of `src/renderer/src/lib/mock.ts`,
//! for UI tests, README screenshots and `cargo run --example`. OWNER: ui-accounts (lands
//! first, so ui-shell can build on it).
//!
//! Differences from mock.ts, on purpose: every address is `@example.com` (the repo is
//! public), and account 5's last-good usage carries a second model (`Sonnet`), so the
//! generated per-model columns see two models, some accounts with one, some with none,
//! and one that only has it through `lastGoodUsage`. Mutations really change the list
//! (move and swap renumber) and push [`Event::Accounts`], like the driver's refresh.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::json;

use crate::api::{Cswap, Event, EventSink, Pending, Updates};
use crate::format::{iso_from_ms, now_ms};
use crate::settings::{AppSettings, SettingsEdit};
use crate::types::*;

/// The version the mock's binary reports, as `test/fake-cswap` does.
pub const MOCK_CSWAP_VERSION: &str = "0.25.0";

struct State {
    payload: Option<ListPayload>,
    error: Option<CswapError>,
    refreshing: bool,
    plans: BTreeMap<String, PlanInfo>,
    settings: AppSettings,
    auto: AutoStatus,
    auto_events: Vec<AutoEvent>,
    log: Vec<CommandLogEntry>,
    next_log_id: u64,
    binary: Option<CswapBinaryInfo>,
    mappings: Vec<Mapping>,
}

pub struct MockCswap {
    st: Mutex<State>,
    sink: EventSink,
    now_ms: i64,
    /// Built with [`MockCswap::at`]: every stamp uses `now_ms`, so renders repeat exactly.
    fixed: bool,
}

fn win(now: i64, pct: f64, secs: i64, weekly: bool) -> UsageWindow {
    let mut w = UsageWindow {
        pct,
        resets_at: Some(iso_from_ms(now + secs * 1000)),
        countdown: Some(String::new()),
        clock: Some(String::new()),
        ..Default::default()
    };
    if weekly {
        w.expected_pct = Some(50.0);
        w.ahead_of_pace = Some(pct > 55.0);
        w.will_last_to_reset = Some(pct < 90.0);
    }
    w
}

fn scoped(now: i64, name: &str, pct: f64, secs: i64) -> ScopedWindow {
    ScopedWindow { name: name.into(), window: win(now, pct, secs, true) }
}

fn account(number: u32, email: &str) -> Account {
    Account { number, email: email.into(), usage_status: "ok".into(), ..Default::default() }
}

/// Which model columns the fixture produces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Fixture {
    /// Fable on accounts 1–2, Sonnet only in account 5's last-good usage: exercises the
    /// union of models and empty cells. The default, for tests.
    #[default]
    TwoModels,
    /// Fable only, what real cswap reports today: for README screenshots, where a second
    /// model column would make the 1240 px table scroll sideways.
    OneModel,
}

/// The five fixture accounts (two model columns), with resets relative to `now` (Unix ms).
pub fn fixture_payload(now: i64) -> ListPayload {
    fixture_payload_for(now, Fixture::TwoModels)
}

/// The five fixture accounts in the chosen [`Fixture`] variant.
pub fn fixture_payload_for(now: i64, fixture: Fixture) -> ListPayload {
    let iso = |s: i64| Some(iso_from_ms(now + s * 1000));
    let a1 = Account {
        organization_name: "you@example.com's Organization".into(),
        organization_uuid: "org-1".into(),
        is_organization: true,
        active: true,
        alias: Some("main".into()),
        usage: Some(Usage {
            five_hour: Some(win(now, 45.0, 12_600, false)),
            seven_day: Some(win(now, 12.0, 180_000, true)),
            scoped: Some(vec![scoped(now, "Fable", 17.0, 180_000)]),
            ..Default::default()
        }),
        usage_fetched_at: iso(-40),
        usage_age_seconds: Some(40.0),
        ..account(1, "you@example.com")
    };
    let a2 = Account {
        organization_name: "Company Inc".into(),
        organization_uuid: "org-2".into(),
        is_organization: true,
        usage: Some(Usage {
            five_hour: Some(win(now, 100.0, 1800, false)),
            seven_day: Some(win(now, 61.0, 250_000, true)),
            scoped: Some(vec![scoped(now, "Fable", 100.0, 250_000)]),
            ..Default::default()
        }),
        usage_fetched_at: iso(-300),
        usage_age_seconds: Some(300.0),
        ..account(2, "work@example.com")
    };
    let a3 = Account {
        disabled: Some(true),
        usage: Some(Usage {
            five_hour: Some(win(now, 3.0, 9000, false)),
            seven_day: Some(win(now, 40.0, 400_000, true)),
            ..Default::default()
        }),
        usage_fetched_at: iso(-900),
        usage_age_seconds: Some(900.0),
        ..account(3, "spare@example.com")
    };
    let a4 = Account { usage_status: "api_key".into(), ..account(4, "api-key-4@token.local") };
    let a5 = Account {
        usage_status: "token_expired".into(),
        last_good_usage: Some(Usage {
            five_hour: Some(win(now, 88.0, 3000, false)),
            seven_day: Some(win(now, 70.0, 100_000, true)),
            scoped: (fixture == Fixture::TwoModels).then(|| vec![scoped(now, "Sonnet", 34.0, 100_000)]),
            ..Default::default()
        }),
        last_good_age_seconds: Some(7200.0),
        ..account(5, "old@example.com")
    };
    ListPayload {
        schema_version: 1,
        active_account_number: Some(1),
        accounts: vec![a1, a2, a3, a4, a5],
        extra: Default::default(),
    }
}

/// The plan labels remembered for the fixture (`plans.json`).
pub fn fixture_plans(now: i64) -> BTreeMap<String, PlanInfo> {
    let plan = |label: &str, tier: &str, sub: &str, ago: i64| PlanInfo {
        label: label.into(),
        tier: Some(tier.into()),
        subscription: Some(sub.into()),
        seen_at: iso_from_ms(now - ago),
    };
    BTreeMap::from([
        ("you@example.com".into(), plan("Max 5x", "default_claude_max_5x", "max", 0)),
        ("work@example.com".into(), plan("Pro", "default_claude_pro", "pro", 86_400_000)),
    ])
}

fn ok_out(stdout: &str) -> Pending<CsResult<PlainOutput>> {
    Pending::ready(Ok(PlainOutput { stdout: stdout.into(), stderr: String::new(), exit_code: Some(0) }))
}

fn not_found(t: &str) -> CswapError {
    CswapError::new("AccountNotFoundError", format!("No account found with identifier: {t}"))
}

fn matches(a: &Account, t: &str) -> bool {
    a.number.to_string() == t || a.email == t || a.alias.as_deref() == Some(t)
}

impl MockCswap {
    /// Fixture data relative to the wall clock.
    pub fn new(sink: EventSink) -> Self {
        Self { fixed: false, ..Self::at(sink, now_ms()) }
    }

    /// Fixture data relative to a fixed `now` (Unix ms), so renders are identical every
    /// run. Pages that show countdowns must be given the same `now`.
    pub fn at(sink: EventSink, now: i64) -> Self {
        Self::with_fixture(sink, now, Fixture::TwoModels)
    }

    /// Like [`MockCswap::at`] with a chosen [`Fixture`] (README screenshots use
    /// [`Fixture::OneModel`]).
    pub fn with_fixture(sink: EventSink, now: i64, fixture: Fixture) -> Self {
        let binary = CswapBinaryInfo {
            path: Some("/home/you/.local/bin/cswap".into()),
            source: BinarySource::Uv,
            version: Some(MOCK_CSWAP_VERSION.into()),
            error: None,
            candidates: Vec::new(),
        };
        let st = State {
            payload: Some(fixture_payload_for(now, fixture)),
            error: None,
            refreshing: false,
            plans: fixture_plans(now),
            settings: AppSettings::default(),
            auto: AutoStatus::default(),
            auto_events: Vec::new(),
            log: Vec::new(),
            next_log_id: 1,
            binary: Some(binary),
            mappings: vec![
                Mapping {
                    path: "/home/you/work/client-app".into(),
                    email: "work@example.com".into(),
                    organization_uuid: Some("org-2".into()),
                    account: None,
                },
                Mapping {
                    path: "D:\\Projects\\side-project".into(),
                    email: "gone@example.com".into(),
                    organization_uuid: None,
                    account: None,
                },
            ],
        };
        Self { st: Mutex::new(st), sink, now_ms: now, fixed: true }
    }

    /// The `now` the fixture was built against.
    pub fn now_ms(&self) -> i64 {
        self.now_ms
    }

    /// Replaces the list state, for the loading (`None`, no error), empty, failed and
    /// stale (payload + error) renders. Pushes it like a refresh would.
    pub fn set_list(&self, payload: Option<ListPayload>, error: Option<CswapError>) {
        let s = {
            let mut st = self.lock();
            st.payload = payload;
            st.error = error;
            state_of(&st, self.now_ms)
        };
        self.sink.send(Event::Accounts(s));
    }

    pub fn set_plans(&self, plans: BTreeMap<String, PlanInfo>) {
        self.lock().plans = plans;
    }

    /// `None` = no cswap found (the onboarding screen).
    pub fn set_binary(&self, binary: Option<CswapBinaryInfo>) {
        self.lock().binary = binary;
    }

    /// Seeds the auto-switch event history without starting anything.
    pub fn push_auto_events(&self, events: Vec<AutoEvent>) {
        self.lock().auto_events.extend(events);
    }

    pub fn set_settings(&self, settings: AppSettings) {
        self.lock().settings = settings;
    }

    fn clock(&self) -> i64 {
        if self.fixed { self.now_ms } else { now_ms() }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        // A panicking test thread must not wedge every later call.
        self.st.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn record(&self, st: &mut State, args: &[&str]) {
        let e = CommandLogEntry {
            id: st.next_log_id,
            at: iso_from_ms(self.clock()),
            args: args.iter().map(|s| s.to_string()).collect(),
            exit_code: Some(0),
            duration_ms: 120,
            ok: true,
            stdout: Some("ok".into()),
            stderr: Some(String::new()),
            error: None,
        };
        st.next_log_id += 1;
        st.log.push(e.clone());
        self.sink.send(Event::CommandLog(e));
    }

    /// Runs `f` on the accounts under the lock, then pushes the new list.
    fn mutate<R>(&self, args: &[&str], f: impl FnOnce(&mut Vec<Account>) -> R) -> R {
        let (r, s) = {
            let mut st = self.lock();
            self.record(&mut st, args);
            let p = st.payload.get_or_insert_with(|| ListPayload { schema_version: 1, ..Default::default() });
            let r = f(&mut p.accounts);
            p.accounts.sort_by_key(|a| a.number);
            p.active_account_number = p.accounts.iter().find(|a| a.active).map(|a| a.number);
            (r, state_of(&st, self.now_ms))
        };
        self.sink.send(Event::Accounts(s));
        r
    }

    fn switch_payload(from: Option<&Account>, to: &Account, strategy: &str) -> SwitchPayload {
        let r = |a: &Account| AccountRef { number: Some(a.number), email: a.email.clone() };
        SwitchPayload {
            schema_version: 1,
            switched: from.is_none_or(|f| f.number != to.number),
            from: from.map(r),
            to: Some(r(to)),
            strategy: strategy.into(),
            reason: "switched".into(),
            message: format!("Switched to Account-{} ({})", to.number, to.email),
            ..Default::default()
        }
    }
}

fn state_of(st: &State, now: i64) -> AccountsState {
    AccountsState {
        payload: st.payload.clone(),
        fetched_at: st.payload.as_ref().map(|_| iso_from_ms(now)),
        refreshing: st.refreshing,
        error: st.error.clone(),
        plans: st.plans.clone(),
    }
}

impl Cswap for MockCswap {
    fn accounts(&self) -> AccountsState {
        state_of(&self.lock(), self.now_ms)
    }

    fn refresh(&self) -> Pending<AccountsState> {
        let mut st = self.lock();
        self.record(&mut st, &["list", "--json"]);
        Pending::ready(state_of(&st, self.now_ms))
    }

    fn status(&self) -> Pending<CsResult<StatusPayload>> {
        let st = self.lock();
        let accounts = st.payload.as_ref().map(|p| p.accounts.as_slice()).unwrap_or_default();
        let active = accounts.iter().find(|a| a.active).map(|a| StatusActive {
            email: a.email.clone(),
            managed: true,
            number: Some(a.number),
            extra: Default::default(),
        });
        Pending::ready(Ok(StatusPayload {
            schema_version: 1,
            active,
            total_managed_accounts: Some(accounts.len() as u32),
            extra: Default::default(),
        }))
    }

    fn switch_to(&self, target: &str, _force: bool) -> Pending<CsResult<SwitchPayload>> {
        let r = self.mutate(&["switch", target, "--json"], |list| {
            let from = list.iter().find(|a| a.active).cloned();
            let to = list.iter().find(|a| matches(a, target)).cloned().ok_or_else(|| not_found(target))?;
            for a in list.iter_mut() {
                a.active = a.number == to.number;
            }
            Ok(Self::switch_payload(from.as_ref(), &to, "direct"))
        });
        Pending::ready(r)
    }

    fn rotate(&self, strategy: Option<RotateStrategy>) -> Pending<CsResult<SwitchPayload>> {
        let name = match strategy {
            Some(RotateStrategy::Best) => Some("best"),
            Some(RotateStrategy::NextAvailable) => Some("next-available"),
            None => None,
        };
        let mut args = vec!["switch", "--json"];
        if let Some(n) = name {
            args.extend(["--strategy", n]);
        }
        let r = self.mutate(&args, |list| {
            let enabled: Vec<&Account> = list.iter().filter(|a| a.disabled != Some(true)).collect();
            if enabled.is_empty() {
                return Err(CswapError::new("NoAccountsError", "No enabled accounts to rotate to"));
            }
            let from = list.iter().find(|a| a.active).cloned();
            let i = enabled.iter().position(|a| a.active);
            let to = enabled[i.map_or(0, |i| (i + 1) % enabled.len())].clone();
            for a in list.iter_mut() {
                a.active = a.number == to.number;
            }
            Ok(Self::switch_payload(from.as_ref(), &to, name.unwrap_or("rotation")))
        });
        Pending::ready(r)
    }

    fn add_account(&self, opts: AddAccountOptions) -> Pending<CsResult<PlainOutput>> {
        let slot = opts.slot.map(|s| s.to_string());
        let mut args = vec!["add"];
        if let Some(s) = &slot {
            args.extend(["--slot", s.as_str()]);
        }
        let mut st = self.lock();
        self.record(&mut st, &args);
        ok_out("Added Account 6: new@example.com")
    }

    fn add_token(&self, opts: AddTokenOptions) -> Pending<CsResult<PlainOutput>> {
        // The token is never recorded, as in the driver: it would travel over stdin.
        let mut args = vec!["add-token", "-"];
        if let Some(e) = &opts.email {
            args.extend(["--email", e.as_str()]);
        }
        let mut st = self.lock();
        self.record(&mut st, &args);
        ok_out("Added Account 6")
    }

    fn remove_account(&self, target: &str) -> Pending<CsResult<PlainOutput>> {
        let r = self.mutate(&["remove", target], |list| {
            let before = list.len();
            list.retain(|a| !matches(a, target));
            if list.len() == before { Err(not_found(target)) } else { Ok(()) }
        });
        Pending::ready(r.map(|_| PlainOutput { stdout: "ok".into(), exit_code: Some(0), ..Default::default() }))
    }

    fn set_disabled(&self, target: &str, disabled: bool) -> Pending<CsResult<PlainOutput>> {
        let verb = if disabled { "disable" } else { "enable" };
        let r = self.mutate(&[verb, target], |list| {
            let a = list.iter_mut().find(|a| matches(a, target)).ok_or_else(|| not_found(target))?;
            a.disabled = disabled.then_some(true);
            Ok(())
        });
        Pending::ready(r.map(|_| PlainOutput { stdout: "ok".into(), exit_code: Some(0), ..Default::default() }))
    }

    fn set_alias(&self, target: &str, alias: Option<&str>) -> Pending<CsResult<PlainOutput>> {
        let r = self.mutate(&["alias", target, alias.unwrap_or("--unset")], |list| {
            let a = list.iter_mut().find(|a| matches(a, target)).ok_or_else(|| not_found(target))?;
            a.alias = alias.map(str::to_owned);
            Ok(())
        });
        Pending::ready(r.map(|_| PlainOutput { stdout: "ok".into(), exit_code: Some(0), ..Default::default() }))
    }

    fn move_account(&self, target: &str, slot: u32) -> Pending<CsResult<PlainOutput>> {
        let s = slot.to_string();
        // cswap's move onto a taken slot trades places, which the Move dialog promises.
        let r = self.mutate(&["move", target, &s], |list| {
            let from = list.iter().find(|a| matches(a, target)).map(|a| a.number).ok_or_else(|| not_found(target))?;
            for a in list.iter_mut() {
                if a.number == from {
                    a.number = slot;
                } else if a.number == slot {
                    a.number = from;
                }
            }
            Ok(())
        });
        Pending::ready(r.map(|_| PlainOutput { stdout: "ok".into(), exit_code: Some(0), ..Default::default() }))
    }

    fn swap_accounts(&self, a: &str, b: &str) -> Pending<CsResult<PlainOutput>> {
        let r = self.mutate(&["swap", a, b], |list| {
            let na = list.iter().find(|x| matches(x, a)).map(|x| x.number).ok_or_else(|| not_found(a))?;
            let nb = list.iter().find(|x| matches(x, b)).map(|x| x.number).ok_or_else(|| not_found(b))?;
            for x in list.iter_mut() {
                if x.number == na {
                    x.number = nb;
                } else if x.number == nb {
                    x.number = na;
                }
            }
            Ok(())
        });
        Pending::ready(r.map(|_| PlainOutput { stdout: "ok".into(), exit_code: Some(0), ..Default::default() }))
    }

    fn list_mappings(&self) -> Pending<CsResult<Vec<Mapping>>> {
        let st = self.lock();
        let accounts = st.payload.as_ref().map(|p| p.accounts.as_slice()).unwrap_or_default();
        // Joined with the current list by email, as the driver does.
        let rows = st
            .mappings
            .iter()
            .map(|m| Mapping { account: accounts.iter().find(|a| a.email == m.email).cloned(), ..m.clone() })
            .collect();
        Pending::ready(Ok(rows))
    }

    fn map_directory(&self, target: &str, path: &str) -> Pending<CsResult<PlainOutput>> {
        let mut st = self.lock();
        self.record(&mut st, &["map", target, path]);
        let Some(a) = st.payload.as_ref().and_then(|p| p.accounts.iter().find(|a| matches(a, target))).cloned() else {
            return Pending::ready(Err(not_found(target)));
        };
        st.mappings.retain(|m| m.path != path);
        st.mappings.push(Mapping {
            path: path.into(),
            email: a.email.clone(),
            organization_uuid: Some(a.organization_uuid.clone()).filter(|s| !s.is_empty()),
            account: None,
        });
        ok_out(&format!("Mapped {path} -> Account-{} ({})", a.number, a.email))
    }

    fn unmap_directory(&self, path: &str) -> Pending<CsResult<PlainOutput>> {
        let mut st = self.lock();
        self.record(&mut st, &["unmap", path]);
        st.mappings.retain(|m| m.path != path);
        ok_out("ok")
    }

    fn config(&self) -> Pending<CsResult<ConfigPayload>> {
        let set = |key: &str, value: serde_json::Value, is_set: bool| ConfigSetting {
            key: key.into(),
            value,
            is_set,
            extra: Default::default(),
        };
        Pending::ready(Ok(ConfigPayload {
            schema_version: 1,
            path: "/home/you/.local/share/claude-swap/settings.json".into(),
            settings: vec![
                set("autoswitch.threshold", json!(90), false),
                set("autoswitch.intervalSeconds", json!(60), false),
                set("autoswitch.cooldownSeconds", json!(300), false),
                set("autoswitch.hysteresisPct", json!(10), false),
                set("autoswitch.strategy", json!("best"), false),
                set("autoswitch.includeApiKeyAccounts", json!(false), false),
                set("autoswitch.unhealthyTicks", json!(3), false),
                set("autoswitch.model", json!("Fable"), true),
                set("ui.theme", json!("auto"), false),
            ],
            extra: Default::default(),
        }))
    }

    fn set_config(&self, key: &str, value: Option<&str>) -> Pending<CsResult<PlainOutput>> {
        let mut st = self.lock();
        match value {
            Some(v) => self.record(&mut st, &["config", "set", key, v]),
            None => self.record(&mut st, &["config", "unset", key]),
        }
        ok_out("ok")
    }

    fn export_accounts(&self, _opts: ExportOptions, path: &str) -> Pending<CsResult<Transfer>> {
        Pending::ready(Ok(Transfer {
            path: path.into(),
            output: PlainOutput { stdout: "Exported 5 accounts".into(), stderr: String::new(), exit_code: Some(0) },
        }))
    }

    fn import_accounts(&self, _opts: ImportOptions, path: &str) -> Pending<CsResult<Transfer>> {
        Pending::ready(Ok(Transfer {
            path: path.into(),
            output: PlainOutput { stdout: "Imported 0, skipped 5".into(), stderr: String::new(), exit_code: Some(0) },
        }))
    }

    fn list_unclaimed(&self) -> Pending<CsResult<Vec<UnclaimedEntry>>> {
        Pending::ready(Ok(vec![UnclaimedEntry {
            id: "a1b2c3".into(),
            slot: "2".into(),
            reason: "keychain locked".into(),
        }]))
    }

    fn purge_unclaimed(&self, _id: &str) -> Pending<CsResult<PlainOutput>> {
        ok_out("ok")
    }

    fn upgrade_cswap(&self) -> Pending<CsResult<PlainOutput>> {
        ok_out("claude-swap is already at the latest version")
    }

    fn token_status(&self) -> Pending<CsResult<PlainOutput>> {
        ok_out(
            "Account-1: you@example.com\n  token: valid (expires in 6h) source=keychain\nAccount-2: work@example.com\n  token: expired",
        )
    }

    fn launch_session(&self, target: &str, cwd: Option<&str>) -> Pending<CsResult<LaunchedSession>> {
        Pending::ready(Ok(LaunchedSession {
            command: format!("cswap run {target}"),
            cwd: cwd.unwrap_or("/home/you").into(),
        }))
    }

    fn auto_start(&self, dry_run: bool) -> Pending<AutoStatus> {
        let (status, ev) = {
            let mut st = self.lock();
            st.auto = AutoStatus {
                running: true,
                pid: Some(4242),
                started_at: Some(iso_from_ms(self.clock())),
                dry_run: Some(dry_run),
                ..Default::default()
            };
            let ev = poll_event(self.clock());
            st.auto_events.push(ev.clone());
            (st.auto.clone(), ev)
        };
        self.sink.send(Event::AutoStatus(status.clone()));
        self.sink.send(Event::AutoEvent(ev));
        Pending::ready(status)
    }

    fn auto_stop(&self) -> Pending<AutoStatus> {
        let status = AutoStatus { running: false, exit_code: Some(0), ..Default::default() };
        self.lock().auto = status.clone();
        self.sink.send(Event::AutoStatus(status.clone()));
        Pending::ready(status)
    }

    fn auto_status(&self) -> AutoStatus {
        self.lock().auto.clone()
    }

    fn auto_events(&self) -> Vec<AutoEvent> {
        self.lock().auto_events.clone()
    }

    fn auto_once(&self, _dry_run: bool) -> Pending<CsResult<AutoOnceResult>> {
        let ev = auto_event(
            self.clock(),
            "no-switch",
            json!({ "reason": "below-threshold", "detail": "active headroom 55%" }),
        );
        Pending::ready(Ok(AutoOnceResult { exit_code: Some(2), outcome: AutoOutcome::NoAction, events: vec![ev] }))
    }

    fn settings(&self) -> AppSettings {
        self.lock().settings.clone()
    }

    fn edit_settings(&self, edit: SettingsEdit) -> AppSettings {
        let s = {
            let mut st = self.lock();
            edit(&mut st.settings);
            st.settings.clone()
        };
        self.sink.send(Event::Settings(s.clone()));
        s
    }

    fn binary(&self) -> Option<CswapBinaryInfo> {
        self.lock().binary.clone()
    }

    fn detect_binary(&self) -> Pending<CswapBinaryInfo> {
        Pending::ready(self.lock().binary.clone().unwrap_or_default())
    }

    fn install_cswap(&self) -> Pending<CsResult<PlainOutput>> {
        ok_out("installed")
    }

    fn command_log(&self) -> Vec<CommandLogEntry> {
        self.lock().log.clone()
    }

    fn clear_command_log(&self) {
        self.lock().log.clear();
    }

    fn vault_path(&self) -> Option<String> {
        Some("/home/you/.local/share/claude-swap".into())
    }
}

/// An auto-switch event of `kind` with `fields` merged in (an object), stamped `at`.
pub fn auto_event(at: i64, kind: &str, fields: serde_json::Value) -> AutoEvent {
    AutoEvent {
        schema_version: 1,
        event: kind.into(),
        ts: iso_from_ms(at),
        extra: match fields {
            serde_json::Value::Object(m) => m,
            _ => Default::default(),
        },
    }
}

/// The `poll` event mock.ts emitted on start.
pub fn poll_event(at: i64) -> AutoEvent {
    auto_event(
        at,
        "poll",
        json!({
            "active": { "number": 1, "email": "you@example.com" },
            "headroomPct": { "1": 55, "2": 0, "3": 60 },
            "threshold": 90,
            "windowsPct": { "1": { "5h": 45, "7d": 12 }, "2": { "5h": 100, "7d": 61 } }
        }),
    )
}

/// Self-update stand-in: always disabled, as 0.1.23's browser mock.
pub struct MockUpdates;

impl Updates for MockUpdates {
    fn state(&self) -> UpdaterState {
        UpdaterState {
            status: UpdaterStatus::Disabled,
            current: "0.0.0-mock".into(),
            disabled_reason: Some("Updates are off in the mock.".into()),
            ..Default::default()
        }
    }
    fn check(&self) -> Pending<UpdaterState> {
        Pending::ready(self.state())
    }
    fn install(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock() -> (MockCswap, std::sync::mpsc::Receiver<Event>) {
        let (sink, rx) = EventSink::new();
        (MockCswap::at(sink, 1_795_800_000_000), rx)
    }

    fn numbers(m: &MockCswap) -> Vec<(u32, String)> {
        m.accounts().payload.unwrap().accounts.iter().map(|a| (a.number, a.email.clone())).collect()
    }

    #[test]
    fn switch_pushes_the_new_list_and_logs_the_call() {
        let (m, rx) = mock();
        let r = m.switch_to("2", false).take().unwrap().unwrap();
        assert!(r.switched);
        assert_eq!(r.to.unwrap().number, Some(2));
        let s = m.accounts();
        assert_eq!(s.payload.as_ref().unwrap().active_account_number, Some(2));
        let evs: Vec<Event> = rx.try_iter().collect();
        assert!(
            evs.iter().any(
                |e| matches!(e, Event::Accounts(a) if a.payload.as_ref().unwrap().active_account_number == Some(2))
            )
        );
        assert_eq!(m.command_log()[0].args, ["switch", "2", "--json"]);
        assert_eq!(m.switch_to("nobody", false).take().unwrap().unwrap_err().kind, "AccountNotFoundError");
    }

    #[test]
    fn rotate_skips_disabled_accounts() {
        let (m, _rx) = mock();
        // 1 → 2 → 4 (3 is disabled) → 5 → 1
        let to = |m: &MockCswap| m.rotate(None).take().unwrap().unwrap().to.unwrap().number.unwrap();
        assert_eq!([to(&m), to(&m), to(&m), to(&m)], [2, 4, 5, 1]);
    }

    #[test]
    fn move_onto_a_taken_slot_trades_places() {
        let (m, _rx) = mock();
        m.move_account("main", 3).take().unwrap().unwrap();
        let n = numbers(&m);
        assert_eq!(n[0], (1, "spare@example.com".into()));
        assert_eq!(n[2], (3, "you@example.com".into()));
        m.swap_accounts("1", "2").take().unwrap().unwrap();
        assert_eq!(numbers(&m)[0], (1, "work@example.com".into()));
    }

    #[test]
    fn alias_disable_remove() {
        let (m, _rx) = mock();
        m.set_alias("2", Some("work")).take().unwrap().unwrap();
        m.set_disabled("work", true).take().unwrap().unwrap();
        let s = m.accounts().payload.unwrap();
        assert_eq!(s.accounts[1].alias.as_deref(), Some("work"));
        assert_eq!(s.accounts[1].disabled, Some(true));
        m.set_disabled("2", false).take().unwrap().unwrap();
        assert_eq!(m.accounts().payload.unwrap().accounts[1].disabled, None);
        m.remove_account("work").take().unwrap().unwrap();
        assert_eq!(numbers(&m).len(), 4);
        assert!(m.remove_account("work").take().unwrap().is_err());
    }

    // The token must never reach the command log, mock or not: the LOG page shows it.
    #[test]
    fn add_token_never_logs_the_token() {
        let (m, _rx) = mock();
        let opts = AddTokenOptions { token: "sk-ant-oat01-SECRET".into(), ..Default::default() };
        m.add_token(opts).take().unwrap().unwrap();
        assert!(m.command_log().iter().all(|e| !e.args.iter().any(|a| a.contains("SECRET"))));
    }

    #[test]
    fn fixture_has_models_in_some_accounts_only() {
        let p = fixture_payload(0);
        let models = |a: &Account| -> Vec<String> {
            a.usage
                .as_ref()
                .or(a.last_good_usage.as_ref())
                .and_then(|u| u.scoped.clone())
                .unwrap_or_default()
                .into_iter()
                .map(|s| s.name)
                .collect()
        };
        assert_eq!(models(&p.accounts[0]), ["Fable"]);
        assert_eq!(models(&p.accounts[4]), ["Sonnet"]);
        assert!(models(&p.accounts[2]).is_empty());
        assert!(p.accounts.iter().all(|a| a.email.ends_with("@example.com") || a.email.ends_with("@token.local")));
        let one = fixture_payload_for(0, Fixture::OneModel);
        assert_eq!(crate::view::models(&one.accounts), ["Fable"]);
        let (sink, _rx) = EventSink::new();
        let m = MockCswap::with_fixture(sink, 0, Fixture::OneModel);
        assert_eq!(crate::view::models(&m.accounts().payload.unwrap().accounts), ["Fable"]);
    }

    #[test]
    fn mappings_join_accounts_and_change() {
        let (m, _rx) = mock();
        let rows = m.list_mappings().take().unwrap().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].account.as_ref().map(|a| a.number), Some(2));
        assert!(rows[1].account.is_none(), "the mapped account is gone");
        m.map_directory("main", "/tmp/picked-project").take().unwrap().unwrap();
        assert_eq!(m.list_mappings().take().unwrap().unwrap().len(), 3);
        m.unmap_directory("/tmp/picked-project").take().unwrap().unwrap();
        assert_eq!(m.list_mappings().take().unwrap().unwrap().len(), 2);
    }

    #[test]
    fn settings_edits_are_pushed() {
        let (m, rx) = mock();
        let s = m.edit_settings(Box::new(|s| s.mask_emails = true));
        assert!(s.mask_emails && m.settings().mask_emails);
        assert!(rx.try_iter().any(|e| matches!(e, Event::Settings(s) if s.mask_emails)));
    }

    #[test]
    fn auto_start_and_stop() {
        let (m, _rx) = mock();
        assert!(m.auto_start(true).take().unwrap().running);
        assert_eq!(m.auto_events()[0].event, "poll");
        assert!(!m.auto_stop().take().unwrap().running);
    }
}
