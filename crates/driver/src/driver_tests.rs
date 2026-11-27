//! Parity gate: `test/unit/driver.test.ts` of 0.1.23, against the fake cswap
//! (`test/fake-cswap`, run with Node) in a temp state dir. Nothing here touches the
//! process environment: every child gets its env from the driver.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cswap_core::*;
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::auto::AutoRunner;
use crate::cli::{Cli, RunOptions};

fn fake() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/fake-cswap");
    let p = if cfg!(windows) { dir.join("fake-cswap.cmd") } else { dir.join("fake-cswap.mjs") };
    // Canonical without the `\\?\` prefix cmd.exe cannot start from.
    let p = p.canonicalize().expect("fake cswap exists");
    let s = p.display().to_string();
    PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s))
}

struct T {
    dir: tempfile::TempDir,
    rt: Runtime,
    cli: Arc<Cli>,
}

impl T {
    fn new() -> T {
        let dir = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let env = vec![("FAKE_CSWAP_STATE".to_owned(), dir.path().join("state.json").display().to_string())];
        let cli = Cli::new(rt.handle(), None, env);
        cli.set_binary(Some(fake()));
        T { dir, rt, cli }
    }
    fn state(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.dir.path().join("state.json")).unwrap()).unwrap()
    }
    fn on<F: std::future::Future>(&self, f: F) -> F::Output {
        self.rt.block_on(f)
    }
    fn last_args(&self) -> Vec<String> {
        self.cli.log().last().unwrap().args.clone()
    }
}

#[test]
fn lists_accounts_from_json_and_records_the_call() {
    let t = T::new();
    let r = t.on(t.cli.list()).unwrap();
    assert_eq!(r.schema_version, 1);
    assert_eq!(r.accounts.iter().map(|a| a.number).collect::<Vec<_>>(), [1, 2, 3, 4]);
    assert!(r.accounts[0].active);
    assert_eq!(r.accounts[2].disabled, Some(true));
    assert_eq!(r.accounts[3].usage_status, "api_key");
    assert_eq!(r.accounts[0].usage.as_ref().unwrap().scoped.as_ref().unwrap()[0].name, "Fable");
    let log = t.cli.log();
    assert_eq!(log.len(), 1);
    assert_eq!(
        (log[0].args.clone(), log[0].exit_code, log[0].ok),
        (vec!["list".into(), "--json".into()], Some(0), true)
    );
}

#[test]
fn switches_by_slot_and_reports_from_to() {
    let t = T::new();
    let r = t.on(t.cli.switch_to("2", false)).unwrap();
    assert!(r.switched);
    assert_eq!(r.to.unwrap().email, "work@company.com");
    assert_eq!(t.state()["activeAccountNumber"], 2);
}

#[test]
fn switching_to_the_active_account_is_a_no_op() {
    let t = T::new();
    let r = t.on(t.cli.switch_to("you@example.com", false)).unwrap();
    assert!(!r.switched);
    assert_eq!(r.reason, "already-active");
}

#[test]
fn surfaces_the_cswap_error_envelope_as_a_typed_error() {
    let t = T::new();
    let e = t.on(t.cli.switch_to("nobody@nowhere", false)).unwrap_err();
    assert_eq!(e.kind, "AccountNotFoundError");
    assert!(e.message.contains("nobody@nowhere"));
    assert!(!t.cli.log()[0].ok);
}

#[test]
fn rotate_honours_strategy() {
    let t = T::new();
    let r = t.on(t.cli.rotate(Some(RotateStrategy::Best))).unwrap();
    assert_eq!(r.strategy, "best");
    assert_eq!(t.last_args(), ["switch", "--json", "--strategy", "best"]);
}

#[test]
fn remove_answers_the_prompt_on_stdin_and_never_on_argv() {
    let t = T::new();
    t.on(t.cli.remove_account("3")).unwrap();
    assert!(t.state()["accounts"].get("3").is_none());
    assert_eq!(t.last_args(), ["remove", "3"]);
    assert!(t.cli.log()[0].stdout.as_deref().unwrap().contains("Removed Account-3"));
}

#[test]
fn add_token_passes_the_token_over_stdin_not_argv() {
    let t = T::new();
    let o = AddTokenOptions {
        token: "sk-ant-oat01-SECRET-VALUE".into(),
        email: Some("me@example.com".into()),
        ..Default::default()
    };
    t.on(t.cli.add_token(&o)).unwrap();
    let entry = serde_json::to_string(&t.cli.log()).unwrap();
    assert!(!entry.contains("SECRET"), "the token reached the command log");
    assert_eq!(t.last_args(), ["add-token", "-", "--email", "me@example.com"]);
    let accounts = t.state()["accounts"].as_object().unwrap().clone();
    assert!(accounts.values().any(|a| a["email"] == "me@example.com"));
    assert!(!format!("{o:?}").contains("SECRET"));
}

#[test]
fn add_token_onto_a_taken_slot_without_overwrite_is_cancelled_by_the_stdin_answer() {
    let t = T::new();
    let mut o = AddTokenOptions { token: "sk-ant-api03-KEY".into(), slot: Some(2), ..Default::default() };
    let r = t.on(t.cli.add_token(&o)).unwrap();
    assert!(r.stdout.contains("Cancelled"));
    assert_eq!(t.state()["accounts"]["2"]["email"], "work@company.com");
    o.overwrite = true;
    t.on(t.cli.add_token(&o)).unwrap();
    assert_eq!(t.state()["accounts"]["2"]["email"], "api-key-2@token.local");
}

#[test]
fn add_onto_a_taken_slot_answers_the_overwrite_prompt() {
    let t = T::new();
    let mut o = AddAccountOptions { slot: Some(2), alias: Some("live".into()), overwrite: false };
    let r = t.on(t.cli.add_account(&o)).unwrap();
    assert!(r.stdout.contains("Cancelled"));
    assert_eq!(t.state()["accounts"]["2"]["email"], "work@company.com");
    o.overwrite = true;
    t.on(t.cli.add_account(&o)).unwrap();
    assert_eq!(t.state()["accounts"]["2"]["email"], "you@example.com");
    assert_eq!(t.last_args(), ["add", "--slot", "2", "--alias", "live"]);
}

#[test]
fn alias_disable_move_map_config_unclaimed_round_trip_through_the_cli() {
    let t = T::new();
    t.on(t.cli.set_alias("2", Some("work"))).unwrap();
    assert_eq!(t.state()["accounts"]["2"]["alias"], "work");
    t.on(t.cli.set_alias("2", None)).unwrap();
    assert!(t.state()["accounts"]["2"].get("alias").is_none());
    let bad = t.on(t.cli.set_alias("2", Some("123"))).unwrap_err();
    assert!(bad.message.contains("Invalid alias"), "{bad}");

    t.on(t.cli.set_disabled("1", true)).unwrap();
    assert_eq!(t.state()["accounts"]["1"]["disabled"], true);

    t.on(t.cli.move_account("4", 9)).unwrap();
    assert_eq!(t.state()["accounts"]["9"]["email"], "api-key-4@token.local");
    t.on(t.cli.swap_accounts("1", "2")).unwrap();
    assert_eq!(t.state()["accounts"]["1"]["email"], "work@company.com");
    assert_eq!(t.state()["activeAccountNumber"], 2);

    t.on(t.cli.map_directory("2", "/tmp/proj")).unwrap();
    assert!(t.state()["mappings"].get("/tmp/proj").is_some());
    let m = t.on(t.cli.list_mappings_text()).unwrap();
    assert!(crate::cli::parse_mappings(&m.stdout).iter().any(|x| x.path == "/tmp/proj"));
    t.on(t.cli.unmap_directory("/tmp/proj")).unwrap();

    let threshold = |c: &ConfigPayload| c.settings.iter().find(|s| s.key == "autoswitch.threshold").cloned().unwrap();
    let c = t.on(t.cli.config()).unwrap();
    assert_eq!(threshold(&c).value, 90.0);
    t.on(t.cli.set_config("autoswitch.threshold", Some("80"))).unwrap();
    let c2 = threshold(&t.on(t.cli.config()).unwrap());
    assert_eq!((c2.value.as_f64(), c2.is_set), (Some(80.0), true));
    assert!(t.on(t.cli.set_config("autoswitch.threshold", Some("10"))).is_err());
    t.on(t.cli.set_config("autoswitch.threshold", None)).unwrap();
    assert_eq!(t.on(t.cli.config_path()).map(PathBuf::from), Some(t.dir.path().join("settings.json")));

    let u = t.on(t.cli.unclaimed_text()).unwrap();
    assert_eq!(crate::cli::parse_unclaimed(&u.stdout), vec![]);
}

#[test]
fn export_writes_a_file_and_import_force_overwrites() {
    let t = T::new();
    let file = t.dir.path().join("backup.cswap").display().to_string();
    t.on(t.cli.export_accounts(&file, &ExportOptions { account: Some("2".into()), full: false })).unwrap();
    let env: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(env["accounts"].as_array().unwrap().len(), 1);
    assert_eq!(env["accounts"][0]["email"], "work@company.com");
    let skip = t.on(t.cli.import_accounts(&file, &ImportOptions { force: false })).unwrap();
    assert!(skip.stdout.contains("Skipped"));
    let force = t.on(t.cli.import_accounts(&file, &ImportOptions { force: true })).unwrap();
    assert!(force.stdout.contains("Imported 1"));
    assert_eq!(t.last_args(), ["import", file.as_str(), "--force"]);
}

/// 0.1.23's `quoteWin` fix (PR #6): through the `.cmd` shim on Windows an empty argument
/// was dropped and the next one shifted left. Here std's batch quoting does the work;
/// characters cmd treats specially must arrive unchanged too.
#[test]
fn empty_and_special_arguments_survive() {
    let t = T::new();
    t.on(t.cli.set_config("autoswitch.model", Some(""))).unwrap();
    let last = |t: &T| t.state()["log"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last(&t), serde_json::json!(["config", "set", "autoswitch.model", ""]));
    let tricky = r#"a b&c|d<e>f^g%PATH%!h!"q"(i)"#;
    t.on(t.cli.set_config("autoswitch.model", Some(tricky))).unwrap();
    assert_eq!(last(&t), serde_json::json!(["config", "set", "autoswitch.model", tricky]));
    t.on(t.cli.map_directory("2", r"C:\some dir\with space")).unwrap();
    assert_eq!(last(&t), serde_json::json!(["map", "2", r"C:\some dir\with space"]));
}

#[test]
fn serializes_mutations_so_two_concurrent_switches_never_interleave() {
    let t = T::new();
    t.cli.set_env("FAKE_CSWAP_DELAY", Some("150"));
    let (c1, c2) = (t.cli.clone(), t.cli.clone());
    let a = t.cli.serial(async move { c1.switch_to("2", false).await });
    let b = t.cli.serial(async move { c2.switch_to("3", false).await });
    let (a, b) = t.on(async { tokio::join!(a, b) });
    let (a, b) = (a.unwrap().unwrap(), b.unwrap().unwrap());
    assert_eq!(a.from.unwrap().number, Some(1));
    assert_eq!(b.from.unwrap().number, Some(2), "b ran after a, not concurrently");
    assert_eq!(t.state()["activeAccountNumber"], 3);
}

#[test]
fn a_missing_binary_is_a_spawn_error_not_a_crash() {
    let t = T::new();
    t.cli.set_binary(Some(t.dir.path().join("does-not-exist")));
    assert_eq!(t.on(t.cli.list()).unwrap_err().kind, "SpawnError");
    t.cli.set_binary(None);
    assert!(t.on(t.cli.list()).unwrap_err().message.contains("not configured"));
}

#[test]
fn a_hung_command_times_out_instead_of_blocking_forever() {
    let t = T::new();
    t.cli.set_env("FAKE_CSWAP_DELAY", Some("5000"));
    let started = Instant::now();
    let out = t.on(t.cli.run(&["list", "--json"], RunOptions { stdin: None, timeout: Duration::from_millis(400) }));
    assert!(started.elapsed() < Duration::from_millis(3000), "took {:?}", started.elapsed());
    assert_ne!(out.exit_code, Some(0));
    assert!(out.stderr.contains("timed out"), "{}", out.stderr);
}

#[test]
fn a_forced_cli_failure_lands_in_the_log_as_not_ok() {
    let t = T::new();
    t.cli.set_env("FAKE_CSWAP_FAIL", Some("disable"));
    let e = t.on(t.cli.set_disabled("1", true)).unwrap_err();
    assert!(e.message.contains("forced failure"), "{e}");
    assert!(t.state()["accounts"]["1"].get("disabled").is_none());
    assert!(!t.cli.log().last().unwrap().ok);
}

#[test]
fn the_command_log_is_capped() {
    let t = T::new();
    t.cli.set_binary(None);
    for _ in 0..(crate::cli::LOG_CAP + 5) {
        t.on(t.cli.run(&["list"], RunOptions::default()));
    }
    let log = t.cli.log();
    assert_eq!(log.len(), crate::cli::LOG_CAP);
    assert_eq!(log[0].id, 6, "the oldest entries go first");
}

#[test]
fn run_version_reads_the_version_line() {
    let t = T::new();
    let env = t.cli.env();
    let (v, _) = t.on(crate::resolve::run_version(&fake(), &env));
    assert_eq!(v.as_deref(), Some("0.25.0"));
    let (v, e) = t.on(crate::resolve::run_version(&t.dir.path().join("nope"), &env));
    assert!(v.is_none());
    assert!(e.is_some());
}

#[test]
fn resolve_prefers_the_settings_path_then_the_env_override() {
    let t = T::new();
    let env = t.cli.env();
    let f = fake().display().to_string();
    let r = t.on(crate::resolve::resolve_cswap(Some(&f), Some("/nope"), &env));
    assert_eq!((r.source, r.version.as_deref()), (BinarySource::Settings, Some("0.25.0")));
    let r = t.on(crate::resolve::resolve_cswap(None, Some(&f), &env));
    assert_eq!((r.source, r.version.as_deref()), (BinarySource::Env, Some("0.25.0")));
    let missing = t.dir.path().join("cswap-missing").display().to_string();
    let r = t.on(crate::resolve::resolve_cswap(Some(&missing), Some(&f), &env));
    assert_eq!((r.source, r.error.as_deref()), (BinarySource::Settings, Some("File not found")));
}

#[test]
fn auto_streams_json_events_refuses_a_second_start_and_stops_cleanly() {
    let t = T::new();
    t.on(t.cli.list()).unwrap(); // seeds the fake state
    let auto = AutoRunner::new(t.rt.handle().clone(), None, None);
    let mut env = t.cli.env();
    env.push(("FAKE_CSWAP_AUTO_INTERVAL".into(), "0.2".into()));
    let st = auto.start(Some(fake()), env.clone(), true);
    assert!(st.running);
    assert_eq!(auto.start(Some(fake()), env, false).pid, st.pid);
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && !auto.events().iter().any(|e| e.event == "sleep") {
        std::thread::sleep(Duration::from_millis(50));
    }
    let kinds: Vec<String> = auto.events().iter().map(|e| e.event.clone()).collect();
    assert!(kinds.contains(&"poll".to_owned()), "{kinds:?}");
    assert!(kinds.contains(&"sleep".to_owned()), "{kinds:?}");
    let pid = st.pid.unwrap();
    let stopped = t.on(auto.stop());
    assert!(!stopped.running);
    assert_eq!(stopped.dry_run, Some(true));
    assert!(!process_alive(pid), "the child (tree) is gone after stop");
}

#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    let out = std::process::Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
}
#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // SAFETY: signal 0 only checks existence.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[test]
fn auto_without_a_binary_reports_last_error_instead_of_spawning() {
    let t = T::new();
    let auto = AutoRunner::new(t.rt.handle().clone(), None, None);
    let st = auto.start(None, vec![], false);
    assert!(!st.running);
    assert!(st.last_error.unwrap().contains("not configured"));
}

#[test]
fn auto_once_reports_outcome_and_events() {
    let t = T::new();
    let r = t.on(t.cli.auto_once(true)).unwrap();
    assert_eq!(r.outcome, AutoOutcome::NoAction);
    assert!(r.events.iter().any(|e| e.event == "poll"));
    assert_eq!(t.last_args(), ["auto", "--once", "--json", "--dry-run"]);
}

// ---- Native end to end ----------------------------------------------------

struct N {
    dir: tempfile::TempDir,
    native: Arc<crate::Native>,
    rx: std::sync::mpsc::Receiver<Event>,
    /// The app's runtime, when the test starts `Native` the way the app does. Declared
    /// after `native`, so it outlives it.
    _rt: Option<Runtime>,
}

impl N {
    fn new(settings: Option<&str>) -> N {
        Self::start(settings, None)
    }

    fn start(settings: Option<&str>, rt: Option<Runtime>) -> N {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let claude = dir.path().join("claude");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(
            claude.join(".credentials.json"),
            r#"{"claudeAiOauth":{"accessToken":"sk-secret","subscriptionType":"max","rateLimitTier":"default_claude_max_5x"}}"#,
        )
        .unwrap();
        if let Some(s) = settings {
            std::fs::write(data.join("settings.json"), s).unwrap();
        }
        let (sink, rx) = EventSink::new();
        let opts = crate::Options {
            extra_env: vec![("FAKE_CSWAP_STATE".into(), dir.path().join("state.json").display().to_string())],
            bin_override: Some(fake().display().to_string()),
            claude_config_dir: Some(claude),
            no_spawn: true,
        };
        let config = crate::DriverConfig { data_dir: data };
        let native = match &rt {
            Some(rt) => {
                let _enter = rt.enter();
                crate::Native::start_with(config, sink, opts)
            }
            None => crate::Native::start_with(config, sink, opts),
        };
        N { dir, native, rx, _rt: rt }
    }

    /// Waits for the first event `f` accepts.
    fn wait_for<R>(&self, mut f: impl FnMut(&Event) -> Option<R>) -> R {
        let deadline = Instant::now() + Duration::from_secs(20);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            if let Ok(ev) = self.rx.recv_timeout(left)
                && let Some(r) = f(&ev)
            {
                return r;
            }
        }
        panic!("event did not arrive");
    }
}

fn wait<T>(mut p: Pending<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Some(v) = p.take() {
            return v;
        }
        assert!(!p.is_lost(), "reply lost");
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("no reply");
}

fn loaded(ev: &Event) -> Option<AccountsState> {
    match ev {
        Event::Accounts(a) if !a.refreshing && a.payload.is_some() => Some(a.clone()),
        _ => None,
    }
}

#[test]
fn native_detects_refreshes_and_remembers_the_live_plan() {
    let n = N::new(None);
    let bin = n.wait_for(|e| match e {
        Event::Binary(b) => Some(b.clone()),
        _ => None,
    });
    assert_eq!((bin.source, bin.version.as_deref()), (BinarySource::Env, Some("0.25.0")));
    let st = n.wait_for(loaded);
    assert_eq!(st.payload.as_ref().unwrap().accounts.len(), 4);
    assert_eq!(st.plans["you@example.com"].label, "Max 5x");
    assert!(!serde_json::to_string(&st).unwrap().contains("sk-secret"));
    assert!(n.dir.path().join("data/plans.json").exists());
    assert_eq!(n.native.binary().unwrap().version.as_deref(), Some("0.25.0"));
}

#[test]
fn native_mutations_are_followed_by_a_pushed_refresh() {
    let n = N::new(None);
    n.wait_for(loaded);
    let r = wait(n.native.switch_to("2", false)).unwrap();
    assert!(r.switched);
    let st = n.wait_for(|e| loaded(e).filter(|a| a.payload.as_ref().unwrap().active_account_number == Some(2)));
    assert!(st.payload.unwrap().accounts[1].active);
    assert!(n.native.command_log().iter().any(|e| e.args == ["switch", "2", "--json"]));
}

#[test]
fn native_concurrent_refreshes_share_one_list_call() {
    let n = N::new(None);
    n.wait_for(loaded);
    let before = n.native.command_log().iter().filter(|e| e.args == ["list", "--json"]).count();
    let (a, b) = (n.native.refresh(), n.native.refresh());
    let (a, b) = (wait(a), wait(b));
    assert_eq!(a, b);
    let after = n.native.command_log().iter().filter(|e| e.args == ["list", "--json"]).count();
    assert_eq!(after - before, 1);
}

#[test]
fn native_mappings_join_the_account_by_email() {
    let n = N::new(None);
    n.wait_for(loaded);
    // The fake has no vault folder of its own, so this goes through `cswap map`.
    let m = wait(n.native.list_mappings()).unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].email, "work@company.com");
    assert_eq!(m[0].account.as_ref().unwrap().number, 2);
}

#[test]
fn native_settings_persist_and_auto_start_remembers_dry_run() {
    let n = N::new(Some(r#"{"refreshSeconds":0,"theme":"dark"}"#));
    n.wait_for(loaded);
    assert_eq!(n.native.settings().refresh_seconds, 0);
    let s = n.native.edit_settings(Box::new(|s| s.mask_emails = true));
    assert!(s.mask_emails);
    let st = wait(n.native.auto_start(true));
    assert!(st.running);
    assert!(n.native.settings().auto_dry_run);
    let raw: Value = serde_json::from_slice(&std::fs::read(n.dir.path().join("data/settings.json")).unwrap()).unwrap();
    assert_eq!(
        (raw["maskEmails"].clone(), raw["autoDryRun"].clone(), raw["theme"].clone()),
        (true.into(), true.into(), "dark".into())
    );
    n.native.shutdown();
    assert!(!n.native.auto_status().running);
}

#[test]
fn native_autostarts_auto_switch_when_the_setting_says_so() {
    let n = N::new(Some(r#"{"autoStartAutoSwitch":true,"autoDryRun":true}"#));
    let st = n.wait_for(|e| match e {
        Event::AutoStatus(s) if s.running => Some(s.clone()),
        _ => None,
    });
    assert_eq!(st.dry_run, Some(true));
    n.native.shutdown();
    assert!(!n.native.auto_status().running);
}

#[test]
fn native_launch_session_uses_the_detected_binary() {
    let n = N::new(None);
    n.wait_for(|e| matches!(e, Event::Binary(_)).then_some(()));
    let d = n.dir.path().display().to_string();
    let r = wait(n.native.launch_session("2", Some(&d))).unwrap();
    assert_eq!(r.command, "cswap run 2");
}

/// The app's convention: one runtime, `start` inside `rt.enter()`, then trait calls from
/// threads that are in no runtime at all (tray, second launch, router). Any ambient
/// `tokio::spawn` / `Handle::current()` behind a trait method panics here.
#[test]
fn native_trait_methods_work_from_a_plain_thread() {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let n = N::start(Some(r#"{"refreshSeconds":15}"#), Some(rt));
    assert!(crate::lock(&n.native.own_rt).is_none(), "started inside a runtime: no runtime of its own");
    let native = n.native.clone();
    let dir = n.dir.path().display().to_string();
    std::thread::spawn(move || {
        assert!(tokio::runtime::Handle::try_current().is_err(), "the test thread must be outside any runtime");
        let info = wait(native.detect_binary());
        assert_eq!(info.version.as_deref(), Some("0.25.0"));
        assert_eq!(wait(native.refresh()).payload.unwrap().accounts.len(), 4);
        assert!(wait(native.switch_to("2", false)).unwrap().switched);
        wait(native.status()).unwrap();
        wait(native.config()).unwrap();
        assert_eq!(wait(native.list_mappings()).unwrap().len(), 1);
        assert!(wait(native.list_unclaimed()).unwrap().is_empty());
        wait(native.launch_session("2", Some(&dir))).unwrap();
        native.edit_settings(Box::new(|s| s.refresh_seconds = 30)); // reschedules the timer
        // A changed path re-detects in the background, through the captured handle.
        let f = fake().display().to_string();
        native.edit_settings(Box::new(move |s| s.cswap_path = Some(f)));
        let deadline = Instant::now() + Duration::from_secs(20);
        while native.binary().map(|b| b.source) != Some(BinarySource::Settings) {
            assert!(Instant::now() < deadline, "re-detection after a path change never finished");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(wait(native.auto_start(true)).running);
        assert!(!wait(native.auto_stop()).running);
        // Account 2 (active since the switch above) is at 100%: the fake would switch.
        let once = wait(native.auto_once(true)).unwrap();
        assert_eq!(once.outcome, AutoOutcome::Switched);
        assert!(once.events.iter().any(|e| e.event == "switch"));
        native.shutdown();
    })
    .join()
    .expect("trait calls from a plain thread");
}

/// Opt-in (`CSWAP_REAL=1 cargo test -p cswap-driver -- --ignored real_`): proves
/// detection against the cswap installed on this machine. Read-only: `--version` and
/// `list --json`, never a mutation.
#[test]
#[ignore]
fn real_cswap_is_found_without_any_override() {
    if std::env::var_os("CSWAP_REAL").is_none() {
        return;
    }
    let rt = Runtime::new().unwrap();
    let env = crate::cli::base_env();
    let info = rt.block_on(crate::resolve::resolve_cswap(None, None, &env));
    println!("{info:#?}");
    assert!(info.version.as_deref().is_some_and(|v| v.chars().next().unwrap().is_ascii_digit()));
    let cli = Cli::new(rt.handle(), None, vec![]);
    cli.set_binary(info.path.map(PathBuf::from));
    let list = rt.block_on(cli.list()).unwrap();
    assert_eq!(list.schema_version, 1);
    println!("{} accounts", list.accounts.len());
}
