//! The window against in-memory services (egui_kittest). Ports the e2e cases of 0.1.23 that
//! cover the shell and the Settings / Log / Onboarding pages (`smoke.spec.ts`,
//! `actions.spec.ts`: palette, settings persistence, log, onboarding; `alignment.spec.ts`:
//! sidebar). Renders go to `tests/snapshots/` through kittest's snapshot check.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use cswap_core::mock::{Fixture as MockFixture, MockCswap};
use cswap_core::{
    AppSettings, CommandLogEntry, Cswap, CswapBinaryInfo, Event, EventSink, Page, ThemeSetting, UpdaterState,
    UpdaterStatus,
};
use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use crate::testing::{DisabledUpdates, HostCall, RecordingHost};
use crate::{CswapApp, Services};

struct World {
    app: Option<CswapApp>,
    init: Option<(Services, Receiver<Event>)>,
}

struct Fixture {
    cswap: Arc<MockCswap>,
    host: Arc<RecordingHost>,
    updates: Arc<DisabledUpdates>,
    sink: EventSink,
}

fn fixture() -> (Fixture, Receiver<Event>) {
    fixture_of(MockFixture::TwoModels)
}

fn fixture_of(which: MockFixture) -> (Fixture, Receiver<Event>) {
    let (sink, rx) = EventSink::new();
    let cswap = Arc::new(MockCswap::with_fixture(sink.clone(), cswap_core::format::now_ms(), which));
    // Off, so a test's command log does not depend on whether another test in this process
    // already spent the process-wide focus refresh.
    cswap.set_settings(AppSettings { refresh_on_focus: false, ..Default::default() });
    // The command log is pushed as the driver pushes it, with FIXED times: the mock stamps
    // its own entries with the wall clock, which made the log renders differ now and then.
    while rx.try_recv().is_ok() {}
    sink.send(Event::CommandLog(entry(1, &["list", "--json"], true)));
    sink.send(Event::CommandLog(entry(2, &["switch", "1", "--json"], true)));
    (Fixture { cswap, host: RecordingHost::new("linux"), updates: DisabledUpdates::new("1.0.0"), sink }, rx)
}

/// A logged CLI call at a fixed time.
fn entry(id: u64, args: &[&str], ok: bool) -> CommandLogEntry {
    CommandLogEntry {
        id,
        at: "2026-11-27T17:06:51.000Z".into(),
        args: args.iter().map(|s| s.to_string()).collect(),
        exit_code: Some(if ok { 0 } else { 1 }),
        duration_ms: 120 + id * 37,
        ok,
        stdout: ok.then(|| "{\"schemaVersion\": 1}".into()),
        stderr: (!ok).then(|| "No account found with identifier: 9".into()),
        error: None,
    }
}

/// A failed call, as the driver logs one (the mock's calls all succeed).
fn failed_entry(id: u64) -> CommandLogEntry {
    entry(id, &["switch", "9", "--json"], false)
}

fn cli_calls(f: &Fixture, args: &[&str]) -> usize {
    f.cswap.command_log().iter().filter(|e| e.args == args).count()
}

fn harness(f: &Fixture, rx: Receiver<Event>, width: f32) -> Harness<'static, World> {
    harness_sized(f, rx, width, 780.0)
}

fn harness_sized(f: &Fixture, rx: Receiver<Event>, width: f32, height: f32) -> Harness<'static, World> {
    let services = Services { cswap: f.cswap.clone(), updates: f.updates.clone(), host: f.host.clone() };
    let theme = f.cswap.settings().theme;
    let world = World { app: None, init: Some((services, rx)) };
    // The fixture's clock and UTC: every wall-clock text renders the same on any machine.
    let now = f.cswap.now_ms();
    Harness::builder()
        .with_size(vec2(width, height))
        .with_render_cursor(false)
        .with_step_dt(1.0 / 30.0)
        .with_max_steps(30)
        .with_theme(if theme == ThemeSetting::Dark { egui::Theme::Dark } else { egui::Theme::Light })
        .build_ui_state(
            move |ui, w: &mut World| {
                if w.app.is_none() {
                    let (s, rx) = w.init.take().expect("services");
                    let mut app = CswapApp::with_context(ui.ctx(), s, rx);
                    app.pin_clock(Some(now), 0);
                    w.app = Some(app);
                }
                // The harness frames its ui with a margin; the real window has none.
                let rect = ui.ctx().content_rect();
                let mut whole = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                whole.set_clip_rect(rect);
                w.app.as_mut().expect("app").frame(&mut whole);
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
    assert_eq!(f.cswap.settings().theme, ThemeSetting::Dark);
    assert_eq!(h.ctx.theme(), egui::Theme::Dark);
    shot(&mut h, "settings-dark");
    f.sink.send(Event::CommandLog(failed_entry(100)));
    nav(&mut h, "Log");
    h.get_by_role_and_label(Role::Button, "cswap switch 9 --json").click();
    settle(&mut h);
    h.get_by_label_contains("No account found with identifier: 9");
    shot(&mut h, "log-dark-expanded");
}

#[test]
fn settings_edits_reach_the_service() {
    let (f, rx) = fixture();
    let mut h = harness_sized(&f, rx, 1240.0, 1100.0);
    settle(&mut h);
    nav(&mut h, "Settings");
    h.get_by_role_and_label(Role::ComboBox, "Refresh interval").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::MenuItem, "5 min").click();
    settle(&mut h);
    assert_eq!(f.cswap.settings().refresh_seconds, 300);

    h.get_by_role_and_label(Role::CheckBox, "Hide email addresses").click();
    settle(&mut h);
    assert!(f.cswap.settings().mask_emails);
    h.get_by_role_and_label(Role::CheckBox, "Close to tray").click();
    settle(&mut h);
    assert!(!f.cswap.settings().close_to_tray);

    // the vault opens through the Host, the links go out through it too
    h.get_by_role_and_label(Role::Button, "Open").click();
    h.get_by_role_and_label(Role::Button, "claude-swap (upstream)").click();
    settle(&mut h);
    assert_eq!(
        f.host.calls(),
        vec![
            HostCall::OpenPath("/home/you/.local/share/claude-swap".into()),
            HostCall::OpenExternal("https://github.com/realiti4/claude-swap".into())
        ]
    );

    // Upgrade reports the last line of cswap's output
    h.get_by_role_and_label(Role::Button, "Upgrade cswap").click();
    settle(&mut h);
    assert!(app(&h).toasts.items.iter().any(|t| t.text.ends_with("claude-swap is already at the latest version")));

    // an unclaimed credential entry is listed and purged through the CLI
    h.get_by_label("a1b2c3");
    h.get_by_role_and_label(Role::Button, "Purge").click();
    settle(&mut h);
    assert!(app(&h).toasts.items.iter().any(|t| t.text == "Purged a1b2c3"));
}

#[test]
fn updater_box_follows_the_state() {
    let (f, rx) = fixture();
    let mut h = harness_sized(&f, rx, 1240.0, 1100.0);
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
    let ready = UpdaterState {
        status: UpdaterStatus::Downloaded,
        current: "1.0.0".into(),
        version: Some("1.0.1".into()),
        ..Default::default()
    };
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
    let rows = |h: &Harness<'_, World>| -> Vec<String> {
        h.query_all_by_role(Role::Button)
            .filter_map(|n| n.accesskit_node().label())
            .filter(|l| l.starts_with("cswap "))
            .collect()
    };
    assert_eq!(rows(&h), ["cswap switch 1 --json", "cswap list --json"]);
    // pushed while open, on top
    f.sink.send(Event::CommandLog(failed_entry(100)));
    settle(&mut h);
    assert_eq!(rows(&h)[0], "cswap switch 9 --json");
    h.get_by_label("exit 1");
    h.get_by_role_and_label(Role::Button, "Clear").click();
    settle(&mut h);
    h.get_by_label("Nothing yet");
    assert!(f.cswap.command_log().is_empty(), "cleared in the service too");
}

#[test]
fn without_cswap_onboarding_shows_and_a_custom_path_recovers() {
    let (f, rx) = fixture();
    f.cswap.set_binary(Some(CswapBinaryInfo {
        path: Some("/definitely/not/here/cswap".into()),
        error: Some("spawn ENOENT".into()),
        candidates: vec!["/home/you/.local/bin".into()],
        ..Default::default()
    }));
    let mut h = harness(&f, rx, 1240.0);
    settle(&mut h);
    h.get_by_label("cswap is not installed yet");
    h.get_by_label("cswap not found");
    shot(&mut h, "onboarding-light");

    // Install with uv reports back and detects again (still nothing found here)
    h.get_by_role_and_label(Role::Button, "Install with uv").click();
    settle(&mut h);
    assert!(app(&h).toasts.items.iter().any(|t| t.text == "claude-swap installed"));
    h.get_by_label("cswap is not installed yet");

    // the Settings button leaves onboarding; a saved path is detected at once
    // the card's own Settings button, not the sidebar's
    h.query_all_by_role_and_label(Role::Button, "Settings")
        .find(|n| n.rect().left() > crate::shell::SIDEBAR_WIDE)
        .expect("onboarding Settings button")
        .click();
    settle(&mut h);
    assert_eq!(app(&h).page, Page::Settings);
    f.cswap.set_binary(Some(CswapBinaryInfo {
        path: Some("/opt/fake/cswap".into()),
        version: Some("0.25.0".into()),
        source: cswap_core::BinarySource::Settings,
        ..Default::default()
    }));
    let input = h
        .query_all_by_label("Custom path")
        .find(|n| n.accesskit_node().role() == Role::TextInput)
        .expect("custom path input");
    input.focus();
    input.type_text("/opt/fake/cswap");
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "Save").click();
    settle(&mut h);
    assert_eq!(f.cswap.settings().cswap_path.as_deref(), Some("/opt/fake/cswap"));
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
    h.get_by_role_and_label(Role::TextInput, "Switch to account").type_text("company");
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(cli_calls(&f, &["switch", "2", "--json"]), 1);
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
    let before = cli_calls(&f, &["list", "--json"]);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    h.key_press_modifiers(Modifiers::COMMAND, Key::R);
    settle(&mut h);
    assert_eq!(app(&h).page, Page::Settings);
    assert_eq!(cli_calls(&f, &["list", "--json"]), before + 1);
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
    assert!(f.cswap.settings().sidebar_collapsed, "persisted as a setting");
    h.get_by_role_and_label(Role::Button, "Expand the sidebar");
}

// Purwa saw the refresh tooltip sit on the refresh button (the title bar is the top edge,
// there is no room above): every title-bar tooltip must keep off its own control.
#[test]
fn title_bar_tooltips_keep_off_their_buttons() {
    for chrome in [crate::WindowChrome::Native, crate::WindowChrome::Custom] {
        let (f, rx) = fixture();
        let mut h = harness(&f, rx, 900.0);
        settle(&mut h);
        h.state_mut().app.as_mut().expect("app").set_chrome(chrome);
        settle(&mut h);
        let labels: &[&str] = if chrome == crate::WindowChrome::Custom {
            &["Refresh usage", "Minimize", "Maximize", "Close"]
        } else {
            &["Refresh usage"]
        };
        for label in labels {
            h.get_by_role_and_label(Role::Button, label).hover();
            settle(&mut h);
            let button = h.get_by_role_and_label(Role::Button, label).rect();
            let tip = h.get_by_role(Role::Tooltip).rect();
            assert!(!tip.intersects(button), "{chrome:?} {label}: tip {tip:?} over button {button:?}");
            assert!(tip.right() <= 900.0 && tip.top() >= 0.0, "{chrome:?} {label}: inside the window: {tip:?}");
        }
    }
}

// The header as the window's title bar (Windows / Linux), in both themes: caption buttons at
// the right (close hovered, red), nothing of the header under them. Checked by eye.
#[test]
fn title_bar_with_caption_buttons_light() {
    title_bar_with_caption_buttons(ThemeSetting::Light, "titlebar-custom-light");
}

#[test]
fn title_bar_with_caption_buttons_dark() {
    title_bar_with_caption_buttons(ThemeSetting::Dark, "titlebar-custom-dark");
}

fn title_bar_with_caption_buttons(theme: ThemeSetting, name: &str) {
    let (f, rx) = fixture();
    f.cswap.edit_settings(Box::new(move |s: &mut AppSettings| s.theme = theme));
    let mut h = harness_sized(&f, rx, 900.0, 560.0);
    settle(&mut h);
    h.state_mut().app.as_mut().expect("app").set_chrome(crate::WindowChrome::Custom);
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "Close").hover();
    settle(&mut h);
    shot(&mut h, name);
}

/// Every tooltip on screen right now: its rect, its text and the rect it was placed against
/// (egui's own: `Popup::default_response_id` = owner.with("popup"), placed against the owner's
/// `interact_rect`; ours, `widgets::tooltip`: owner.with("tip"), against `rect`).
fn tooltips_on_screen(h: &Harness<'_, World>) -> Vec<(egui::Rect, String, Option<egui::Rect>)> {
    let layers: Vec<(egui::Id, egui::Rect)> = h.ctx.memory(|m| {
        m.areas()
            .visible_layer_ids()
            .into_iter()
            .filter(|l| l.order == egui::Order::Tooltip)
            .filter_map(|l| m.area_rect(l.id).map(|r| (l.id, r)))
            .collect()
    });
    let hovered = h.ctx.interaction_snapshot(|i| i.hovered.clone());
    // Drawn this frame = has a `Role::Tooltip` node (egui's tooltip Ui, our tooltip's label);
    // egui still lists an area as visible for a frame after it was last shown.
    // (egui's tooltip Ui node has no bounds of its own: its contents do.)
    let any = || egui_kittest::kittest::by().predicate(|n| n.bounding_box().is_some());
    let mut drawn: Vec<egui::Rect> = Vec::new();
    for t in h.query_all(egui_kittest::kittest::by().role(Role::Tooltip)) {
        if t.accesskit_node().bounding_box().is_some() {
            drawn.push(t.rect());
        }
        drawn.extend(t.query_all(any()).map(|n| n.rect()));
    }
    layers
        .into_iter()
        .filter(|(_, r)| drawn.iter().any(|t| r.expand(1.0).contains_rect(*t)))
        .map(|(id, r)| {
            let text = h
                .query_all(egui_kittest::kittest::by().predicate(|n| n.bounding_box().is_some()))
                .filter(|n| r.expand(1.0).contains_rect(n.rect()))
                .filter_map(|n| n.accesskit_node().value().or(n.accesskit_node().label()))
                .collect::<Vec<_>>()
                .join(" ");
            let anchor = hovered.iter().find_map(|w| {
                let resp = h.ctx.read_response(*w)?;
                if w.with("popup") == id {
                    Some(resp.interact_rect)
                } else if w.with("tip") == id {
                    Some(resp.rect)
                } else {
                    None
                }
            });
            (r, text, anchor)
        })
        .collect()
}

/// Rects of the widgets that could own a tooltip summoned from `at`: egui's hovered widgets and
/// the accessibility nodes within its 5 px hover radius (`interact_radius`).
fn candidates_under(h: &Harness<'_, World>, at: egui::Pos2) -> Vec<egui::Rect> {
    let hovered = h.ctx.interaction_snapshot(|i| i.hovered.clone());
    let mut out: Vec<egui::Rect> =
        hovered.into_iter().filter_map(|id| h.ctx.read_response(id)).flat_map(|r| [r.interact_rect, r.rect]).collect();
    out.extend(h.query_all(egui_kittest::kittest::by().predicate(|n| n.bounding_box().is_some())).map(|n| n.rect()));
    out.retain(|r| r.expand(5.0).contains(at));
    out
}

/// `tip` sits `gap` off one side of `anchor` (egui's tooltips: 4; ours: 6 or 14), as placed
/// against it rather than clamped onto it.
fn placed_beside(tip: egui::Rect, anchor: egui::Rect) -> bool {
    let near = |a: f32, b: f32| [4.0, 6.0, 14.0].iter().any(|g| (a - b - g).abs() < 0.75);
    (near(tip.top(), anchor.bottom())
        || near(anchor.top(), tip.bottom())
        || near(tip.left(), anchor.right())
        || near(anchor.left(), tip.right()))
        && !tip.intersects(anchor.shrink(0.5))
}

/// Hovers a grid over the window and checks every tooltip that appears: inside the window and
/// clear of the widget it describes. Returns the tooltips seen, for the audit list.
fn audit_tooltips(h: &mut Harness<'_, World>, size: egui::Vec2, step: f32) -> std::collections::BTreeSet<String> {
    let mut seen = std::collections::BTreeSet::new();
    let window = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
    let mut y = step / 2.0;
    while y < size.y {
        let mut x = step / 2.0;
        while x < size.x {
            let at = egui::pos2(x, y);
            h.hover_at(at);
            h.run_steps(6);
            for (r, text, anchor) in tooltips_on_screen(h) {
                match anchor {
                    Some(a) => {
                        assert!(
                            !r.intersects(a.shrink(0.5)),
                            "{text:?} at {at:?}: tip {r:?} on its anchor {a:?} ({size:?})"
                        )
                    }
                    // Owner not among egui's hovered set (a widget that re-interacts under
                    // another id): some widget at the pointer must have the tip placed beside it.
                    None => {
                        let c = candidates_under(h, at);
                        assert!(
                            c.iter().any(|a| placed_beside(r, *a)),
                            "{text:?} at {at:?}: tip {r:?} is beside none of {c:?} ({size:?})"
                        );
                    }
                }
                assert!(window.expand(0.5).contains_rect(r), "{text:?} at {at:?} leaves the window: {r:?} ({size:?})");
                seen.insert(text);
            }
            x += step;
        }
        y += step;
    }
    seen
}

// Every tooltip in the app, on every page, at a wide, the minimum and a narrow window (cards,
// icon sidebar): none may sit on what it describes or run off the window. Slow (a few
// thousand hovers), so on demand: `cargo test -p cswap-ui tooltip_audit -- --ignored --nocapture`.
#[test]
#[ignore]
fn tooltip_audit() {
    let step: f32 = std::env::var("TIP_AUDIT_STEP").ok().and_then(|s| s.parse().ok()).unwrap_or(10.0);
    // (width, height, usage as rings, sidebar collapsed by hand + the header's own caption buttons)
    let mut configs = vec![];
    for (w, hgt) in [(1240.0, 780.0), (760.0, 520.0), (560.0, 520.0)] {
        configs.push((w, hgt, false, false));
        configs.push((w, hgt, true, false));
    }
    configs.push((1240.0, 780.0, false, true));
    // TIP_AUDIT_ONLY=560 or =Mappings: one width or page, to re-run a failure quickly
    let only = std::env::var("TIP_AUDIT_ONLY").unwrap_or_default();
    for (w, hgt, rings, custom) in configs {
        for page in ["Accounts", "Auto-switch", "Mappings", "Log", "Settings"] {
            if (rings && page != "Accounts") || !(only.is_empty() || only == w.to_string() || only == page) {
                continue;
            }
            let (f, rx) = fixture();
            f.cswap.edit_settings(Box::new(move |s| {
                if rings {
                    s.accounts_layout = cswap_core::AccountsLayout::Tiles;
                }
                s.sidebar_collapsed = custom;
            }));
            let mut h = harness_sized(&f, rx, w, hgt);
            settle(&mut h);
            if custom {
                h.state_mut().app.as_mut().expect("app").set_chrome(crate::WindowChrome::Custom);
            }
            nav(&mut h, page);
            let seen = audit_tooltips(&mut h, vec2(w, hgt), step);
            let what = match (rings, custom) {
                (true, _) => " (tiles)",
                (_, true) => " (sidebar collapsed, caption buttons)",
                _ => "",
            };
            println!("{w}x{hgt} {page}{what}: {seen:?}");
        }
    }
}

/// Every shared control in both themes, with a menu open, and a confirm dialog: the look of
/// what ui-accounts builds on, checked by eye in the snapshots.
#[test]
fn widget_gallery_light() {
    gallery(egui::Theme::Light, "light");
}

#[test]
fn widget_gallery_dark() {
    gallery(egui::Theme::Dark, "dark");
}

fn gallery(theme: egui::Theme, name: &str) {
    use crate::widgets::{self, Button, Size, Tone, Variant, icon};
    {
        let mut sel = 60u32;
        let mut text = String::from("work");
        let mut seg = 1u8;
        let mut h = Harness::builder()
            .with_size(vec2(760.0, 560.0))
            .with_render_cursor(false)
            .with_theme(theme)
            .with_context_setup(move |ctx| {
                crate::style::setup(
                    ctx,
                    if theme == egui::Theme::Dark { ThemeSetting::Dark } else { ThemeSetting::Light },
                );
            })
            .build_ui(move |ui| {
                let t = widgets::tokens(ui);
                ui.painter().rect_filled(ui.ctx().content_rect(), 0.0, t.bg);
                ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
                ui.horizontal(|ui| {
                    for v in [Variant::Primary, Variant::Default, Variant::Subtle, Variant::Ghost, Variant::Danger] {
                        Button::new(&format!("{v:?}")).variant(v).show(ui);
                    }
                    Button::new("Disabled").enabled(false).show(ui);
                });
                ui.horizontal(|ui| {
                    Button::new("Add account").primary().icon(icon::PLUS).show(ui);
                    Button::new("Rotate").small().icon(icon::SHUFFLE).show(ui);
                    let more = Button::icon_only(icon::ELLIPSIS, "More").size(Size::Md).show(ui);
                    widgets::menu(&more, 220.0, |ui| {
                        widgets::menu_item(ui, "Set alias…", Some(icon::TAG), false, false, true, None);
                        widgets::menu_item(ui, "Best", Some(icon::SPARKLES), true, false, true, Some("most headroom"));
                        widgets::menu_item(ui, "Disabled item", None, false, false, false, None);
                        widgets::menu_separator(ui);
                        widgets::menu_item(ui, "Remove", Some(icon::TRASH_2), false, true, true, None);
                    });
                });
                ui.horizontal(|ui| {
                    for (l, tone) in [
                        ("OK", Tone::Ok),
                        ("Token expired", Tone::Danger),
                        ("Keychain locked", Tone::Warn),
                        ("API key", Tone::Muted),
                        ("Max 5x", Tone::Accent),
                    ] {
                        widgets::chip(ui, l, tone);
                    }
                    widgets::kbd(ui, "Ctrl K");
                    widgets::kbd(ui, icon::CORNER_DOWN_LEFT);
                });
                ui.horizontal(|ui| {
                    widgets::input(ui, "Alias", &mut text, "alias", false, 200.0);
                    widgets::select(
                        ui,
                        "Interval",
                        &mut sel,
                        &[(60, "1 min".to_owned()), (300, "5 min".to_owned())],
                        120.0,
                    );
                    widgets::switch(ui, "on", true, true);
                    widgets::switch(ui, "off", false, true);
                    widgets::switch(ui, "disabled", true, false);
                    if let Some(v) =
                        widgets::segmented(ui, seg, &[(0u8, Some(icon::SUN), "Light"), (1, Some(icon::MOON), "Dark")])
                    {
                        seg = v;
                    }
                });
                widgets::card(
                    ui,
                    Some("Card title"),
                    |ui| {
                        widgets::chip(ui, "v0.25.0", Tone::Ok);
                    },
                    0.0,
                    |ui| {
                        egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
                            widgets::field(
                                ui,
                                "Inline field",
                                Some("A hint that explains the setting in one line."),
                                true,
                                |ui| widgets::switch(ui, "Inline field", true, true),
                            );
                        });
                    },
                );
                widgets::empty(
                    ui,
                    Some(icon::SCROLL_TEXT),
                    "Nothing yet",
                    "Commands will show up here as the app talks to cswap.",
                    |_| {},
                );
            });
        h.run_steps(4);
        h.get_by_role_and_label(Role::Button, "More").click();
        h.run_steps(6);
        h.get_by_role_and_label(Role::MenuItem, "Best");
        h.snapshot(format!("widgets-{name}"));
    }
}

#[test]
fn confirm_dialog() {
    use crate::widgets::{self, Button, icon};
    let mut h = Harness::builder()
        .with_size(vec2(760.0, 420.0))
        .with_render_cursor(false)
        .with_theme(egui::Theme::Light)
        .with_context_setup(|ctx| crate::style::setup(ctx, ThemeSetting::Light))
        .build_ui(|ui| {
            let t = widgets::tokens(ui);
            ui.painter().rect_filled(ui.ctx().content_rect(), 0.0, t.bg);
            let add = Button::new("Add account").primary().icon(icon::PLUS);
            let w = add.width(ui);
            widgets::page_header(
                ui,
                "Accounts",
                |_| {},
                w,
                |ui| {
                    add.show(ui);
                },
            );
            widgets::confirm(
                ui.ctx(),
                "rm",
                "Remove Account-2?",
                "work@example.com is removed from cswap's vault. This cannot be undone.",
                "Remove",
                true,
            );
        });
    h.run_steps(4);
    h.get_by_role_and_label(Role::Button, "Remove");
    h.snapshot("dialog-confirm");
}

/// Review renders of the Accounts page in the whole window (sidebar and title bar as the
/// app has them), at the mockup's widths 1120 / 860 / 760, light and dark, rows and tiles,
/// hero on and off, plus an opened row: `CSWAP_UI_SHOTS=<dir> cargo test -p cswap-ui
/// accounts_renders`. A rendering job for looking at, not a check.
#[test]
fn accounts_renders() {
    let Some(dir) = std::env::var_os("CSWAP_UI_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("shots dir");
    for dark in [false, true] {
        for tiles in [false, true] {
            for hero in [true, false] {
                for w in [1120.0, 860.0, 760.0] {
                    let (f, rx) = fixture();
                    f.cswap.edit_settings(Box::new(move |s| {
                        s.theme = if dark { ThemeSetting::Dark } else { ThemeSetting::Light };
                        s.accounts_layout =
                            if tiles { cswap_core::AccountsLayout::Tiles } else { cswap_core::AccountsLayout::Rows };
                        s.show_hero = hero;
                    }));
                    let mut h = harness_sized(&f, rx, w, 900.0);
                    settle(&mut h);
                    let name = format!(
                        "acc-{}-{}-{}-{w}",
                        if tiles { "tiles" } else { "rows" },
                        if hero { "hero" } else { "nohero" },
                        if dark { "dark" } else { "light" }
                    );
                    let img = h.render().expect("render");
                    img.save(dir.join(format!("{name}.png"))).expect("save png");
                }
            }
        }
    }
    // an opened row, wide and narrow
    for w in [1120.0, 760.0] {
        let (f, rx) = fixture();
        let mut h = harness_sized(&f, rx, w, 1000.0);
        settle(&mut h);
        h.get_by_role_and_label(Role::Button, "Show per-model windows for account 6").click();
        h.get_by_role_and_label(Role::Button, "Show per-model windows for account 3").click();
        settle(&mut h);
        let img = h.render().expect("render");
        img.save(dir.join(format!("acc-rows-open-light-{w}.png"))).expect("save png");
    }
}

/// The README images (`docs/screenshots/*.png`, 0.1.23's `docs.spec.ts`), regenerated with
/// `DOCS=1 cargo test -p cswap-ui readme_screenshots`. A rendering job, not a check: without
/// `DOCS` it returns at once. Everything shown comes from the in-memory fixture
/// (`@example.com`, `/home/you`), never from this machine.
#[test]
fn readme_screenshots() {
    if std::env::var_os("DOCS").is_none() {
        return;
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/screenshots");
    std::fs::create_dir_all(&dir).expect("docs dir");
    let save = |h: &mut Harness<'_, World>, name: &str| {
        let img = h.render().expect("render");
        img.save(dir.join(format!("{name}.png"))).expect("save png");
    };
    // ONE scoped model, as real cswap reports today
    let (f, rx) = fixture_of(MockFixture::OneModel);
    // the README shows the settings a new user gets
    f.cswap.set_settings(AppSettings::default());
    let mut h = harness(&f, rx, 1240.0);
    settle(&mut h);
    save(&mut h, "accounts-light");

    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    settle(&mut h);
    save(&mut h, "palette");
    h.key_press(Key::Escape);
    settle(&mut h);

    // auto-switch running, every event shown, as docs.spec.ts staged it
    nav(&mut h, "Auto-switch");
    h.get_by_role_and_label(Role::Button, "Start").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "All").click();
    settle(&mut h);
    save(&mut h, "auto");
    h.get_by_role_and_label(Role::Button, "Stop").click();
    settle(&mut h);
    // the toasts of start/stop must not land in the next frames
    h.state_mut().app.as_mut().expect("app").toasts.items.clear();

    nav(&mut h, "Settings");
    save(&mut h, "settings-light");

    h.get_by_role_and_label(Role::RadioButton, "Dark").click();
    settle(&mut h);
    nav(&mut h, "Accounts");
    save(&mut h, "accounts-dark");

    // the tiles, and the rows once the window is narrow
    f.cswap.edit_settings(Box::new(|s| s.accounts_layout = cswap_core::AccountsLayout::Tiles));
    settle(&mut h);
    save(&mut h, "accounts-tiles");
    f.cswap.edit_settings(Box::new(|s| {
        s.accounts_layout = cswap_core::AccountsLayout::Rows;
        s.theme = ThemeSetting::Light;
    }));
    h.set_size(vec2(760.0, 900.0));
    settle(&mut h);
    save(&mut h, "accounts-compact");

    h.set_size(vec2(1240.0, 780.0));
    nav(&mut h, "Mappings");
    save(&mut h, "mappings");
}
