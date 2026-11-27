//! The window against in-memory services (egui_kittest). Ports the e2e cases of 0.1.23 that
//! cover the shell and the Settings / Log / Onboarding pages (`smoke.spec.ts`,
//! `actions.spec.ts`: palette, settings persistence, log, onboarding; `alignment.spec.ts`:
//! sidebar). Renders go to `tests/snapshots/` through kittest's snapshot check.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use cswap_core::{CswapBinaryInfo, Event, EventSink, Page, ThemeSetting, UpdaterState, UpdaterStatus};
use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use crate::testing::{DisabledUpdates, HostCall, RecordingHost, StubCswap};
use crate::{CswapApp, Services};

struct World {
    app: Option<CswapApp>,
    init: Option<(Services, Receiver<Event>)>,
}

struct Fixture {
    cswap: Arc<StubCswap>,
    host: Arc<RecordingHost>,
    updates: Arc<DisabledUpdates>,
    sink: EventSink,
}

fn fixture() -> (Fixture, Receiver<Event>) {
    let (sink, rx) = EventSink::new();
    let cswap = StubCswap::new(Some(sink.clone()));
    (Fixture { cswap, host: RecordingHost::new("linux"), updates: DisabledUpdates::new("1.0.0"), sink }, rx)
}

fn harness(f: &Fixture, rx: Receiver<Event>, width: f32) -> Harness<'static, World> {
    let services = Services { cswap: f.cswap.clone(), updates: f.updates.clone(), host: f.host.clone() };
    let theme = f.cswap.st().settings.theme;
    let world = World { app: None, init: Some((services, rx)) };
    Harness::builder()
        .with_size(vec2(width, 780.0))
        .with_render_cursor(false)
        .with_step_dt(1.0 / 30.0)
        .with_max_steps(30)
        .with_theme(if theme == ThemeSetting::Dark { egui::Theme::Dark } else { egui::Theme::Light })
        .build_ui_state(
            |ui, w: &mut World| {
                if w.app.is_none() {
                    let (s, rx) = w.init.take().expect("services");
                    w.app = Some(CswapApp::with_context(ui.ctx(), s, rx));
                }
                w.app.as_mut().expect("app").frame(ui);
            },
            world,
        )
}

fn app<'a>(h: &'a Harness<'_, World>) -> &'a CswapApp {
    h.state().app.as_ref().expect("app")
}

/// A few frames: enough for clicks, popups and the 120 ms tooltip, without waiting on
/// anything that animates forever.
fn settle(h: &mut Harness<'_, World>) {
    h.run_steps(8);
}

fn nav(h: &mut Harness<'_, World>, label: &str) {
    h.get_by_role_and_label(Role::Button, label).click();
    settle(h);
}

fn shot(h: &mut Harness<'_, World>, name: &str) {
    h.snapshot(name);
}

#[test]
fn renders_settings_and_log_in_light_and_dark() {
    let (f, rx) = fixture();
    let mut h = harness(&f, rx, 1240.0);
    settle(&mut h);
    h.get_by_label("cswap 0.25.0");
    nav(&mut h, "Settings");
    assert_eq!(app(&h).page, Page::Settings);
    h.get_by_label("cswap binary");
    h.get_by_label("Appearance & behaviour");
    shot(&mut h, "settings-light");
    nav(&mut h, "Log");
    h.get_by_role_and_label(Role::Button, "cswap list --json");
    shot(&mut h, "log-light");

    // dark: through the Theme control, like a user
    nav(&mut h, "Settings");
    h.get_by_role_and_label(Role::RadioButton, "Dark").click();
    settle(&mut h);
    assert_eq!(f.cswap.st().settings.theme, ThemeSetting::Dark);
    assert_eq!(h.ctx.theme(), egui::Theme::Dark);
    shot(&mut h, "settings-dark");
    nav(&mut h, "Log");
    h.get_by_role_and_label(Role::Button, "cswap switch 9 --json").click();
    settle(&mut h);
    h.get_by_label_contains("No account found with identifier: 9");
    shot(&mut h, "log-dark-expanded");
}

#[test]
fn settings_edits_reach_the_service() {
    let (f, rx) = fixture();
    let mut h = harness(&f, rx, 1240.0);
    settle(&mut h);
    nav(&mut h, "Settings");
    h.get_by_role_and_label(Role::ComboBox, "Refresh interval").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::MenuItem, "5 min").click();
    settle(&mut h);
    assert_eq!(f.cswap.st().settings.refresh_seconds, 300);

    h.get_by_role_and_label(Role::CheckBox, "Hide email addresses").click();
    settle(&mut h);
    assert!(f.cswap.st().settings.mask_emails);
    h.get_by_role_and_label(Role::CheckBox, "Close to tray").click();
    settle(&mut h);
    assert!(!f.cswap.st().settings.close_to_tray);

    // the vault opens through the Host, the links go out through it too
    h.get_by_role_and_label(Role::Button, "Open").click();
    h.get_by_role_and_label(Role::Button, "claude-swap (upstream)").click();
    settle(&mut h);
    assert_eq!(
        f.host.calls(),
        vec![
            HostCall::OpenPath("/home/you/.claude-swap-backup".into()),
            HostCall::OpenExternal("https://github.com/realiti4/claude-swap".into())
        ]
    );

    // Upgrade reports the last line of cswap's output
    h.get_by_role_and_label(Role::Button, "Upgrade cswap").click();
    settle(&mut h);
    assert!(app(&h).toasts.items.iter().any(|t| t.text.ends_with("claude-swap is already at the latest version")));
}

#[test]
fn updater_box_follows_the_state() {
    let (f, rx) = fixture();
    let mut h = harness(&f, rx, 1240.0);
    settle(&mut h);
    nav(&mut h, "Settings");
    h.get_by_label("Automatic updates are off");
    // a disabled updater has no auto-download switch to turn
    let sw = h.get_by_role_and_label(Role::CheckBox, "Download updates automatically");
    assert!(sw.accesskit_node().is_disabled());
    h.get_by_role_and_label(Role::Button, "Check for updates").click();
    settle(&mut h);
    assert_eq!(*f.updates.checks.lock().unwrap(), 1);

    // a downloaded update: a toast once, a dot on Settings, and the install button
    let ready = UpdaterState { status: UpdaterStatus::Downloaded, current: "1.0.0".into(), version: Some("1.0.1".into()), ..Default::default() };
    f.updates.set(ready.clone());
    f.sink.send(Event::Updater(ready));
    settle(&mut h);
    assert!(app(&h).toasts.items.iter().any(|t| t.text.starts_with("Update 1.0.1 downloaded")));
    h.get_by_label("Version 1.0.1 is ready to install");
    h.get_by_role_and_label(Role::Button, "Restart & install").click();
    settle(&mut h);
    assert_eq!(*f.updates.installs.lock().unwrap(), 1);
    shot(&mut h, "settings-update-ready");
}

#[test]
fn log_lists_newest_first_and_clears() {
    let (f, rx) = fixture();
    let mut h = harness(&f, rx, 1240.0);
    settle(&mut h);
    nav(&mut h, "Log");
    let rows: Vec<String> = h
        .query_all_by_role(Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.starts_with("cswap "))
        .collect();
    assert_eq!(rows, ["cswap switch 9 --json", "cswap config get --json", "cswap list --json"]);
    // pushed while open
    f.sink.send(Event::CommandLog(crate::testing::log_entry(4, &["status", "--json"], true)));
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "cswap status --json");
    h.get_by_role_and_label(Role::Button, "Clear").click();
    settle(&mut h);
    h.get_by_label("Nothing yet");
    assert!(f.cswap.calls().contains(&"clear_log".to_owned()));
}

#[test]
fn without_cswap_onboarding_shows_and_a_custom_path_recovers() {
    let (f, rx) = fixture();
    {
        let mut s = f.cswap.st();
        s.binary = Some(CswapBinaryInfo {
            path: Some("/definitely/not/here/cswap".into()),
            error: Some("spawn ENOENT".into()),
            candidates: vec!["/home/you/.local/bin".into()],
            ..Default::default()
        });
        s.detect = s.binary.clone();
    }
    let mut h = harness(&f, rx, 1240.0);
    settle(&mut h);
    h.get_by_label("cswap is not installed yet");
    h.get_by_label("cswap not found");
    shot(&mut h, "onboarding-light");

    // Install with uv fails here (no uv in the stub): the error is a toast that stays
    h.get_by_role_and_label(Role::Button, "Install with uv").click();
    settle(&mut h);
    assert!(app(&h).toasts.items.iter().any(|t| t.text.starts_with("Install failed")));

    // the Settings button leaves onboarding; a saved path is detected at once
    h.get_by_role_and_label(Role::Button, "Settings").click();
    settle(&mut h);
    assert_eq!(app(&h).page, Page::Settings);
    f.cswap.st().detect = Some(CswapBinaryInfo {
        path: Some("/opt/fake/cswap".into()),
        version: Some("0.25.0".into()),
        source: cswap_core::BinarySource::Settings,
        ..Default::default()
    });
    let input = h
        .query_all_by_label("Custom path")
        .find(|n| n.accesskit_node().role() == Role::TextInput)
        .expect("custom path input");
    input.focus();
    input.type_text("/opt/fake/cswap");
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "Save").click();
    settle(&mut h);
    assert_eq!(f.cswap.st().settings.cswap_path.as_deref(), Some("/opt/fake/cswap"));
    h.get_by_label("cswap 0.25.0");
    nav(&mut h, "Accounts");
    assert!(h.query_by_label("cswap is not installed yet").is_none());
}

#[test]
fn palette_switches_through_the_cli_and_closes_on_escape() {
    let (f, rx) = fixture();
    let mut h = harness(&f, rx, 1240.0);
    settle(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    settle(&mut h);
    assert!(app(&h).palette.open);
    let input = h.get_by_role_and_label(Role::TextInput, "Switch to account");
    assert!(input.accesskit_node().is_focused(), "the query field takes focus");
    shot(&mut h, "palette-light");
    h.get_by_role_and_label(Role::TextInput, "Switch to account").type_text("acme");
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(f.cswap.calls().contains(&"switch 2".to_owned()));
    assert!(!app(&h).palette.open);
    assert!(app(&h).toasts.items.iter().any(|t| t.text.starts_with("Switched to Account-2")));
    // the active pill follows the pushed list
    h.get_by_label("Active: work@example.com");

    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    settle(&mut h);
    assert!(app(&h).palette.open);
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(!app(&h).palette.open);

    // Ctrl+, opens Settings; Ctrl+R refreshes
    h.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    h.key_press_modifiers(Modifiers::COMMAND, Key::R);
    settle(&mut h);
    assert_eq!(app(&h).page, Page::Settings);
    assert!(f.cswap.calls().iter().filter(|c| *c == "refresh").count() >= 2, "{:?}", f.cswap.calls());
}

#[test]
fn sidebar_collapses_by_itself_when_narrow_and_by_hand_when_not() {
    let (f, rx) = fixture();
    let mut h = harness(&f, rx, 560.0);
    settle(&mut h);
    assert!(h.query_by_label("Collapse the sidebar").is_none(), "forced: nothing to choose");
    assert!(h.query_by_label("Expand the sidebar").is_none());
    // collapsed items explain themselves BESIDE the icon, inside the window
    h.get_by_role_and_label(Role::Button, "Accounts").hover();
    settle(&mut h);
    let tip = h.get_by_role(Role::Tooltip).rect();
    assert!(tip.left() >= crate::shell::SIDEBAR_NARROW, "beside the sidebar: {tip:?}");
    assert!(tip.top() >= 0.0 && tip.bottom() <= 780.0);
    shot(&mut h, "sidebar-tooltip");

    h.set_size(vec2(1240.0, 780.0));
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "Collapse the sidebar").click();
    settle(&mut h);
    assert!(f.cswap.st().settings.sidebar_collapsed, "persisted as a setting");
    h.get_by_role_and_label(Role::Button, "Expand the sidebar");
}
