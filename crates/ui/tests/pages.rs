//! The Accounts, Auto and Mappings pages against `MockCswap` (egui_kittest). The parity
//! gate for 0.1.23's e2e cases that cover these pages (`actions.spec.ts`), plus the rules
//! of the 1.1.0 Accounts redesign: the narrow shape at a 780 px CONTENT width, readiness
//! order, the hero owning the active account, Best next, per-model windows.
//!
//! `CSWAP_UI_SHOTS=<dir>` also writes PNGs of the dialogs, menus and the other pages.
//! Without it nothing is rendered, so the tests need no GPU. The Accounts page itself is
//! rendered inside the whole window by `cswap-ui`'s `accounts_renders`.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use cswap_core::mock::{MockCswap, MockUpdates, auto_event};
use cswap_core::view::PageShape;
use cswap_core::{
    AccountsLayout, AccountsState, AppSettings, AutoStatus, Cswap, Event, EventSink, Host, Page, ThemeSetting,
};
use cswap_ui::Services;
use cswap_ui::cx::Cx;
use cswap_ui::pages::{accounts::AccountsPage, auto::AutoPage, mappings::MappingsPage};
use cswap_ui::theme::Tokens;
use cswap_ui::toast::Toasts;
use egui::accesskit::Role;
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use serde_json::json;

const NOW: i64 = 1_795_800_000_000;
const HEIGHT: f32 = 780.0;

#[derive(Default)]
struct FakeHost {
    directory: Mutex<Option<String>>,
}

impl Host for FakeHost {
    fn pick_directory(&self, _title: &str) -> Option<String> {
        self.directory.lock().unwrap().clone()
    }
    fn pick_file(&self, _: &str, _: &[(&str, &[&str])]) -> Option<String> {
        Some("/home/you/claude-swap.cswap".into())
    }
    fn save_file(&self, _: &str, name: &str) -> Option<String> {
        Some(format!("/home/you/{name}"))
    }
    fn open_path(&self, _: &str) {}
    fn open_external(&self, _: &str) {}
    fn app_version(&self) -> String {
        "1.0.0".into()
    }
    fn platform(&self) -> &'static str {
        "linux"
    }
    fn close_window(&self) {}
    fn quit(&self) {}
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Which {
    Accounts,
    Auto,
    Mappings,
}

/// What the app would hold: cached state from the getters, updated from events.
struct World {
    mock: Arc<MockCswap>,
    host: Arc<FakeHost>,
    rx: Receiver<Event>,
    services: Services,
    tokens: Tokens,
    settings: AppSettings,
    accounts: AccountsState,
    auto: AutoStatus,
    toasts: Toasts,
    which: Which,
    accounts_page: AccountsPage,
    auto_page: AutoPage,
    mappings_page: MappingsPage,
}

impl World {
    fn new(which: Which, dark: bool) -> Self {
        let (sink, rx) = EventSink::new();
        let mock = Arc::new(MockCswap::at(sink, NOW));
        let host = Arc::new(FakeHost::default());
        let services = Services { cswap: mock.clone(), updates: Arc::new(MockUpdates), host: host.clone() };
        let mut accounts_page = AccountsPage::new();
        accounts_page.now_ms = Some(NOW);
        accounts_page.utc_offset = Some(0);
        let mut auto_page = AutoPage::new();
        auto_page.now_ms = Some(NOW + 3_000);
        auto_page.utc_offset = Some(0);
        Self {
            settings: mock.settings(),
            accounts: mock.accounts(),
            auto: mock.auto_status(),
            mock,
            host,
            rx,
            services,
            tokens: if dark { Tokens::dark() } else { Tokens::light() },
            toasts: Toasts::default(),
            which,
            accounts_page,
            auto_page,
            mappings_page: MappingsPage::new(),
        }
    }

    fn frame(&mut self, ui: &mut egui::Ui) {
        while let Ok(ev) = self.rx.try_recv() {
            match &ev {
                Event::Accounts(a) => self.accounts = a.clone(),
                Event::Settings(s) => self.settings = s.clone(),
                Event::AutoStatus(s) => self.auto = s.clone(),
                _ => {}
            }
            self.accounts_page.on_event(&ev);
            self.auto_page.on_event(&ev);
            self.mappings_page.on_event(&ev);
        }
        // The harness frames its ui with a margin; the real window has none.
        let rect = ui.ctx().content_rect();
        ui.painter().rect_filled(rect, 0.0, self.tokens.bg);
        let mut whole = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        whole.set_clip_rect(rect);
        let mut navigate: Option<Page> = None;
        let mut cx = Cx {
            services: &self.services,
            tokens: &self.tokens,
            settings: &self.settings,
            accounts: &self.accounts,
            auto: &self.auto,
            toasts: &mut self.toasts,
            navigate: &mut navigate,
        };
        match self.which {
            Which::Accounts => self.accounts_page.ui(&mut whole, &mut cx),
            Which::Auto => self.auto_page.ui(&mut whole, &mut cx),
            Which::Mappings => self.mappings_page.ui(&mut whole, &mut cx),
        }
    }

    fn toast_texts(&self) -> Vec<String> {
        self.toasts.items.iter().map(|t| t.text.clone()).collect()
    }
}

fn harness(which: Which, width: f32, dark: bool) -> Harness<'static, World> {
    harness_with(World::new(which, dark), width)
}

fn harness_with(world: World, width: f32) -> Harness<'static, World> {
    let dark = world.tokens.dark;
    let mut h = Harness::builder()
        .with_size(vec2(width, HEIGHT))
        .with_render_cursor(false)
        .with_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light })
        // The app's fonts, icons and widget style, as the real window has them.
        .with_context_setup(move |ctx| {
            cswap_ui::style::setup(ctx, if dark { ThemeSetting::Dark } else { ThemeSetting::Light })
        })
        .build_ui_state(|ui, w: &mut World| w.frame(ui), world);
    h.run();
    h
}

fn shot(h: &mut Harness<'_, World>, name: &str) {
    let Some(dir) = std::env::var_os("CSWAP_UI_SHOTS") else { return };
    std::fs::create_dir_all(&dir).unwrap();
    let img = h.render().expect("render");
    img.save(std::path::Path::new(&dir).join(format!("{name}.png"))).unwrap();
}

/// At least one node with this label ("Pro" is both a row's chip and the Best next's).
fn has_label(h: &Harness<'_, World>, label: &str) -> bool {
    h.query_all_by_label(label).next().is_some()
}

// The content is the page minus its 24 px padding on both sides.
const PAD: f32 = 48.0;

fn with_settings(f: impl FnOnce(&mut AppSettings), width: f32) -> Harness<'static, World> {
    let mut world = World::new(Which::Accounts, false);
    let mut s = AppSettings::default();
    f(&mut s);
    world.mock.set_settings(s);
    world.settings = world.mock.settings();
    harness_with(world, width)
}

// TESTED RULE: the page goes narrow below 780 px of CONTENT width (not the window's), the
// hero's rings centre below 600; a narrow row has no readiness text.
#[test]
fn shape_follows_the_content_width() {
    for (content, shape) in [
        (520.0, PageShape::Compact),
        (650.0, PageShape::Narrow),
        (779.0, PageShape::Narrow),
        (781.0, PageShape::Wide),
        (1300.0, PageShape::Wide),
    ] {
        let h = harness(Which::Accounts, content + PAD, false);
        assert_eq!(h.state().accounts_page.last_shape, shape, "content {content}px");
        assert!((h.state().accounts_page.last_content_width - content).abs() < 1.5, "measured the content");
        let readiness = has_label(&h, "6 readiness: Ready · 66% left");
        assert_eq!(readiness, shape == PageShape::Wide, "readiness text only in a wide row ({content}px)");
        // every shape still carries both windows of every row
        assert!(has_label(&h, "6 5h: 8%") && has_label(&h, "6 7d: 34%"));
    }
}

// TESTED RULES: Ready, Limited, Held out, Re-login needed; the hero takes the live
// account; hiding it puts the account back on top as "Active now" and drops Best next.
#[test]
fn list_is_sorted_by_readiness_and_the_hero_owns_the_active_account() {
    let h = harness(Which::Accounts, 1400.0, false);
    // 6 ready, 4 ready (API key, usage unknown), 2 limited, 3 disabled, 5 login expired
    assert_eq!(h.state().accounts_page.last_order, [6, 4, 2, 3, 5]);
    assert!(has_label(&h, "2 readiness: Limited · free again in 30m"));
    assert!(has_label(&h, "3 readiness: Held out · disabled"));
    assert!(has_label(&h, "5 readiness: Re-login needed · login expired"));
    assert!(!has_label(&h, "1 readiness: Active now · 55% left"), "the hero shows it");

    let h = with_settings(|s| s.show_hero = false, 1400.0);
    assert_eq!(h.state().accounts_page.last_order, [1, 6, 4, 2, 3, 5]);
    assert!(has_label(&h, "1 readiness: Active now · 55% left"));
    assert_eq!(h.state().accounts_page.last_best, None, "no hero, no Best next");
    assert!(h.query_by_label("Switch to the best next account").is_none());
}

// TESTED RULE: Best next names cswap's `best` pick (with `autoswitch.model` folded in) and
// its Switch runs `cswap switch --strategy best`, not a switch to the named slot.
#[test]
fn best_next_runs_cswaps_own_pick() {
    let mut h = harness(Which::Accounts, 1400.0, false);
    assert_eq!(h.state().accounts_page.last_best, Some(6));
    h.get_by_role_and_label(Role::Button, "Switch to the best next account").click();
    h.run();
    assert_eq!(h.state().mock.command_log().last().unwrap().args, ["switch", "--json", "--strategy", "best"]);
    assert_eq!(h.state().mock.accounts().payload.unwrap().active_account_number, Some(6));
    h.run();
    assert_eq!(h.state().accounts_page.last_best, None, "on the best account now: no strip");
}

// TESTED RULES: per-model 7-day windows in the hero, in an opened row (a click on the row,
// not its buttons), "No per-model limits reported" without any, and always on tiles.
#[test]
fn per_model_windows_in_hero_rows_and_tiles() {
    let mut h = harness(Which::Accounts, 1400.0, false);
    assert!(has_label(&h, "1 model-Fable: 17%"), "hero");
    assert!(!has_label(&h, "6 model-Sonnet: 41%"), "a closed row shows none");
    h.get_by_role_and_label(Role::Button, "Show per-model windows for account 6").click();
    h.run();
    assert!(has_label(&h, "6 model-Fable: 20%") && has_label(&h, "6 model-Sonnet: 41%"));
    assert!(h.query_by_role_and_label(Role::Button, "Hide per-model windows for account 6").is_some());
    h.get_by_role_and_label(Role::Button, "Show per-model windows for account 3").click();
    h.run();
    assert!(has_label(&h, "No per-model limits reported"));
    // the row's own buttons do not open it
    h.get_by_role_and_label(Role::Button, "Actions for account 2").click();
    h.run();
    assert!(h.query_by_role_and_label(Role::Button, "Hide per-model windows for account 2").is_none());

    let h = with_settings(|s| s.accounts_layout = AccountsLayout::Tiles, 1400.0);
    assert!(has_label(&h, "6 model-Sonnet: 41%") && has_label(&h, "2 model-Fable: 100%"));
    assert!(has_label(&h, "6 5h: 8%"), "tiles name their rings");
}

// TESTED RULE: cswap's `plan` badges every account that has one (a seat with its tier),
// plans.json covers the rest, and canceled / lapsed get a chip of their own, with a
// tooltip, in rows, narrow rows and tiles (one `chips_of`, every layout calling it).
#[test]
fn plan_chips_in_every_layout() {
    for (width, tiles) in [(1400.0, false), (700.0, false), (1400.0, true)] {
        let mut h = with_settings(
            |s| {
                if tiles {
                    s.accounts_layout = AccountsLayout::Tiles
                }
            },
            width,
        );
        // tall enough that every row is on screen for the hover
        h.set_size(vec2(width, 1400.0));
        h.run();
        for label in ["Max 5x", "Team · Max 5x", "Max 20x", "canceled", "Free", "lapsed", "Pro"] {
            assert!(has_label(&h, label), "{label} at {width}px tiles={tiles}");
        }
        h.get_by_label("canceled").hover();
        h.run_steps(8);
        assert!(h.query_by_label_contains("until the paid period ends").is_some(), "tooltip at {width}px");
    }
}

// TESTED RULE: the stored login's expiry is a chip only within 7 days or once past (hero
// and list); its hover says how long and what to do. A wide row says "login expired" in
// its readiness text instead of a chip (and "disabled" likewise): said twice, the chips
// only wrapped the name line.
#[test]
fn login_expiry_chips() {
    for width in [1400.0, 700.0] {
        let mut h = harness(Which::Accounts, width, false);
        assert!(has_label(&h, "login 3d 5h"), "account 1 at {width}px");
        let wide = width > 1000.0;
        assert_eq!(has_label(&h, "login expired"), !wide, "account 5's chip at {width}px");
        assert_eq!(has_label(&h, "disabled"), !wide, "account 3's chip at {width}px");
        assert_eq!(has_label(&h, "5 readiness: Re-login needed · login expired"), wide);
        assert!(h.query_by_label_contains("login 19d").is_none(), "20 days off: no chip");
        h.get_by_label("login 3d 5h").hover();
        h.run_steps(8);
        assert!(h.query_by_label_contains("Before then: run /login").is_some(), "tooltip at {width}px");
    }
}

// e2e "switch, rotate ... change the active account through the CLI".
#[test]
fn switch_and_rotate_go_through_the_cli() {
    let mut h = harness(Which::Accounts, 1400.0, false);
    h.get_by_role_and_label(Role::Button, "Switch to account 2").click();
    h.run();
    let s = h.state().mock.accounts();
    assert_eq!(s.payload.unwrap().active_account_number, Some(2));
    assert!(h.state().toast_texts().iter().any(|t| t.contains("Switched to Account-2")));
    assert_eq!(h.state().mock.command_log().last().unwrap().args, ["switch", "2", "--json"]);
    // account 2 is the hero now; account 1 is a row with a Switch
    assert!(h.query_by_label("Switch to account 2").is_none());
    assert!(h.query_by_label("Switch to account 1").is_some());

    h.get_by_role_and_label(Role::Button, "Rotate").click();
    h.run();
    h.get_by_label("Best (most quota left)").click();
    h.run();
    assert_eq!(h.state().mock.command_log().last().unwrap().args, ["switch", "--json", "--strategy", "best"]);
}

// e2e "the pace marker can be switched off" and the 1.1.0 toggles: the header's buttons
// are app settings, saved through the service.
#[test]
fn view_toggles_are_settings() {
    let mut h = harness(Which::Accounts, 1400.0, false);
    h.get_by_role_and_label(Role::Button, "Show accounts as tiles").click();
    h.run();
    assert_eq!(h.state().mock.settings().accounts_layout, AccountsLayout::Tiles);
    h.get_by_role_and_label(Role::Button, "Show accounts as rows").click();
    h.run();
    assert_eq!(h.state().mock.settings().accounts_layout, AccountsLayout::Rows);
    h.get_by_role_and_label(Role::Button, "Hide the active account card").click();
    h.run();
    assert!(!h.state().mock.settings().show_hero);
    assert!(has_label(&h, "1 readiness: Active now · 55% left"));
    h.get_by_role_and_label(Role::Button, "Show the active account on top").click();
    h.run();
    assert!(h.state().mock.settings().show_hero);
    h.get_by_role_and_label(Role::Button, "Hide email addresses (keeps the first three characters)").click();
    h.run();
    assert!(h.state().mock.settings().mask_emails);
    h.get_by_role_and_label(Role::Button, "Hide the pace marker").click();
    h.run();
    assert!(!h.state().mock.settings().show_pace);
}

// e2e "remove needs the slot or email typed, then confirms".
#[test]
fn remove_needs_the_slot_or_email_typed() {
    let mut h = harness(Which::Accounts, 1400.0, false);
    h.get_by_role_and_label(Role::Button, "Actions for account 3").click();
    h.run();
    h.get_by_label("Remove account").click();
    h.run();
    let submit = |h: &Harness<'_, World>| {
        h.get_by_role_and_label(Role::Button, "Remove permanently").accesskit_node().is_disabled()
    };
    assert!(submit(&h), "nothing typed");
    h.get_by_role(Role::TextInput).set_text_value_accesskit("nope");
    h.run();
    assert!(submit(&h));
    shot(&mut h, "accounts-remove-dialog-light");
    h.get_by_role(Role::TextInput).set_text_value_accesskit("spare@example.com");
    h.run();
    assert!(!submit(&h));
    h.get_by_role_and_label(Role::Button, "Remove permanently").click();
    h.run();
    let s = h.state().mock.accounts().payload.unwrap();
    assert!(s.accounts.iter().all(|a| a.number != 3));
    assert!(h.state().toast_texts().iter().any(|t| t == "Removed Account-3"));
    assert!(h.query_by_role_and_label(Role::Button, "Remove permanently").is_none(), "dialog closed");
}

// e2e "open terminal asks where to start": a directory already mapped to the account is
// filled in, and launching passes it.
#[test]
fn open_terminal_prefills_the_mapped_directory() {
    let mut h = harness(Which::Accounts, 1400.0, false);
    h.get_by_role_and_label(Role::Button, "Actions for account 2").click();
    h.run();
    h.get_by_label("Open terminal as this account").click();
    h.run();
    h.run();
    h.get_by_role_and_label(Role::Button, "Open terminal").click();
    h.run();
    let toasts = h.state().toast_texts();
    assert!(toasts.iter().any(|t| t == "Opened cswap run 2\n/home/you/work/client-app"), "{toasts:?}");
}

#[test]
fn auto_starts_streams_checks_and_stops() {
    let mut h = harness(Which::Auto, 1240.0, false);
    h.get_by_role_and_label(Role::Button, "Start").click();
    h.run();
    assert!(h.state().mock.auto_status().running);
    assert!(h.query_by_role_and_label(Role::Button, "Stop").is_some());
    h.get_by_role_and_label(Role::Button, "Stop").click();
    h.run();
    assert!(!h.state().mock.auto_status().running);
    assert!(h.state().toast_texts().iter().any(|t| t == "Auto-switch stopped"));
    h.get_by_role_and_label(Role::Button, "Check now").click();
    h.run();
    assert!(h.state().toast_texts().iter().any(|t| t == "Nothing to do — below threshold"));
}

#[test]
fn mappings_add_with_the_picker_and_unmap() {
    let mut h = harness(Which::Mappings, 1240.0, false);
    *h.state().host.directory.lock().unwrap() = Some("/tmp/picked-project".into());
    h.get_by_role_and_label(Role::Button, "Map a directory").click();
    h.run();
    h.get_by_role_and_label(Role::Button, "Browse").click();
    h.run();
    h.get_by_role_and_label(Role::Button, "Map").click();
    h.run();
    h.run();
    let rows = h.state().mock.list_mappings().take().unwrap().unwrap();
    assert!(
        rows.iter().any(|m| m.path == "/tmp/picked-project" && m.email == "you@example.com"),
        "mapped to the active account"
    );
    assert!(has_label(&h, "Unmap /tmp/picked-project"), "the list reloaded");
    h.get_by_role_and_label(Role::Button, "Unmap /tmp/picked-project").click();
    h.run();
    h.run();
    assert!(!has_label(&h, "Unmap /tmp/picked-project"));
}

/// Renders for review: the Auto and Mappings pages, the Accounts dialogs, menus and empty
/// and error states.
#[test]
fn renders() {
    if std::env::var_os("CSWAP_UI_SHOTS").is_none() {
        return;
    }
    for dark in [false, true] {
        let theme = if dark { "dark" } else { "light" };
        let world = World::new(Which::Auto, dark);
        world.mock.auto_start(false);
        world.mock.push_auto_events(vec![
            auto_event(NOW - 2_000, "no-switch", json!({ "reason": "below-threshold", "detail": "active headroom 55%" })),
            auto_event(NOW - 1_000, "sleep", json!({ "seconds": 60, "until": "2026-11-27T17:21:00Z" })),
            auto_event(
                NOW,
                "switch",
                json!({ "from": { "number": 1, "email": "you@example.com" }, "to": { "number": 3, "email": "spare@example.com" }, "trigger": "threshold" }),
            ),
        ]);
        let mut world = world;
        world.auto = world.mock.auto_status();
        let mut h = harness_with(world, 1240.0);
        shot(&mut h, &format!("auto-{theme}"));
        h.get_by_role_and_label(Role::Button, "All").click();
        h.run();
        shot(&mut h, &format!("auto-all-{theme}"));

        let mut h = harness(Which::Mappings, 1240.0, dark);
        shot(&mut h, &format!("mappings-{theme}"));
    }
    // dialogs
    for (menu, item, name) in [
        ("Actions for account 3", "Remove account", "dialog-remove"),
        ("Actions for account 2", "Open terminal as this account", "dialog-run"),
        ("Actions for account 2", "Set alias", "dialog-alias"),
        ("Add account", "From current Claude login", "dialog-add"),
        ("Add account", "From setup-token or API key", "dialog-token"),
        ("More", "Export all accounts", "dialog-export"),
    ] {
        let mut h = harness(Which::Accounts, 1240.0, false);
        h.get_by_role_and_label(Role::Button, menu).click();
        h.run();
        h.get_by_label(item).click();
        h.run();
        h.run();
        shot(&mut h, &format!("accounts-{name}-light"));
    }
    let mut h = harness(Which::Accounts, 1240.0, true);
    h.get_by_role_and_label(Role::Button, "Rotate").click();
    h.run();
    shot(&mut h, "accounts-menu-rotate-dark");
    // empty / error states
    let world = World::new(Which::Accounts, false);
    world.mock.set_list(Some(Default::default()), None);
    let mut h = harness_with(world, 1240.0);
    shot(&mut h, "accounts-empty-light");
    let world = World::new(Which::Accounts, false);
    let payload = world.mock.accounts().payload;
    world
        .mock
        .set_list(payload, Some(cswap_core::CswapError::new("Timeout", "cswap list --json took longer than 30 s")));
    let mut h = harness_with(world, 1240.0);
    shot(&mut h, "accounts-stale-light");
}
