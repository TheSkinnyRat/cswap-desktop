//! The window against in-memory services (egui_kittest). Ports the e2e cases of 0.1.23 that
//! cover the shell and the Settings / Log / Onboarding pages (`smoke.spec.ts`,
//! `actions.spec.ts`: palette, settings persistence, log, onboarding; `alignment.spec.ts`:
//! sidebar). Renders go to `tests/snapshots/` through kittest's snapshot check.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use cswap_core::mock::MockCswap;
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
    let (sink, rx) = EventSink::new();
    let cswap = Arc::new(MockCswap::new(sink.clone()));
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
    Harness::builder()
        .with_size(vec2(width, height))
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
    // Wall-clock text ("15:04:05" in the log) depends on the machine's time zone; masked
    // so a snapshot taken in one zone still matches in another.
    let clocks: Vec<egui::Rect> = h
        .query_all_by_role(Role::Label)
        .filter(|n| {
            n.accesskit_node().value().is_some_and(|v| is_clock(&v))
                || n.accesskit_node().label().is_some_and(|v| is_clock(&v))
        })
        // the whole time column, whatever the digits' widths, and a little bleed around it
        .map(|n| {
            let r = n.rect();
            egui::Rect::from_min_max(r.min - vec2(2.0, 2.0), egui::pos2(r.min.x + crate::pages::log::TIME_W + 2.0, r.max.y + 2.0))
        })
        .collect();
    for r in clocks {
        h.mask(r);
    }
    h.snapshot(name);
}

fn is_clock(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 8 && b[2] == b':' && b[5] == b':' && s.chars().filter(char::is_ascii_digit).count() == 6
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
                    icon::SCROLL_TEXT,
                    "Nothing yet",
                    "Commands will show up here as the app talks to cswap.",
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
            widgets::page_header(ui, "Accounts", None, |ui| {
                Button::new("Add account").primary().icon(icon::PLUS).show(ui);
            });
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

/// The README images (`docs/screenshots/*.png`, 0.1.23's `docs.spec.ts`), regenerated with
/// `DOCS=1 cargo test -p cswap-ui readme_screenshots`. A rendering job, not a check: without
/// `DOCS` it returns at once. 1240 px is the floor, not a taste: below a 1220 window the three
/// window columns hit their minimum and the Switch button leaves the frame. Everything shown
/// comes from the in-memory fixture (`@example.com`, `/home/you`), never from this machine.
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
    let (f, rx) = fixture();
    // ONE scoped model, as real cswap reports today, so the floor shows the whole table
    f.cswap.set_list(Some(crate::testing::one_model_payload(f.cswap.now_ms())), None);
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

    nav(&mut h, "Auto-switch");
    save(&mut h, "auto");

    nav(&mut h, "Settings");
    save(&mut h, "settings-light");

    h.get_by_role_and_label(Role::RadioButton, "Dark").click();
    settle(&mut h);
    nav(&mut h, "Accounts");
    save(&mut h, "accounts-dark");

    // the ring view, and the same page once the window is too narrow for a table
    f.cswap.edit_settings(Box::new(|s| s.usage_view = cswap_core::UsageView::Rings));
    settle(&mut h);
    save(&mut h, "accounts-rings");
    f.cswap.edit_settings(Box::new(|s| {
        s.usage_view = cswap_core::UsageView::Bars;
        s.theme = ThemeSetting::Light;
    }));
    h.set_size(vec2(620.0, 780.0));
    settle(&mut h);
    save(&mut h, "accounts-compact");

    h.set_size(vec2(1240.0, 780.0));
    nav(&mut h, "Mappings");
    save(&mut h, "mappings");
}
