//! The Accounts, Auto and Mappings pages against `MockCswap` (egui_kittest). The parity
//! gate for 0.1.23's e2e cases that cover these pages (`alignment.spec.ts`,
//! `actions.spec.ts`), asserting only the rules the project notes mark as tested: the
//! column set, the empty cell, and the card/table switch at a 780 px LIST.
//!
//! `CSWAP_UI_SHOTS=<dir>` also writes a PNG of each page at the widths that matter (cards,
//! the 780–956 scroll band, the table) in light and dark. Without it nothing is rendered,
//! so the tests need no GPU.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use cswap_core::mock::{MockCswap, MockUpdates, auto_event};
use cswap_core::{AccountsState, AppSettings, AutoStatus, Cswap, Event, EventSink, Host, Page, UsageView};
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
        // Inputs are named by the field label painted above them, which accesskit does
        // not see; the shared widgets (ui-shell) will link the two.
        .with_accessibility_check(false)
        // egui's bundled Ubuntu-Light lacks "→"; the app's Inter (ui-shell) has it.
        .allow_missing_glyphs()
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

fn has_label(h: &Harness<'_, World>, label: &str) -> bool {
    h.query_by_label(label).is_some()
}

// The list is the window minus the page's 24 px padding on both sides.
const PAD: f32 = 48.0;

// TESTED RULE: cards below a fixed 780 px LIST width, a table from 780 up, whatever the
// columns would want (e2e "a narrow window changes the shape instead of scrolling").
#[test]
fn shape_switches_at_a_780px_list() {
    for (list, cards) in [(560.0, true), (779.0, true), (781.0, false), (900.0, false), (1300.0, false)] {
        let h = harness(Which::Accounts, list + PAD, false);
        assert_eq!(h.state().accounts_page.last_cards, cards, "list {list}px");
        assert!((h.state().accounts_page.last_list_width - list).abs() < 1.5, "measured the list, not the window");
    }
}

// TESTED RULES: the window columns are the union of models across accounts; an account
// without a model keeps an empty `-` cell in a table; a card drops it instead.
#[test]
fn model_columns_and_empty_cells() {
    let h = harness(Which::Accounts, 1400.0, false);
    // Fable is live on 1 and 2; Sonnet only in account 5's last-good usage.
    assert!(has_label(&h, "1 model-Fable: 17%"));
    assert!(has_label(&h, "5 model-Sonnet: 34%"));
    assert!(has_label(&h, "3 model-Fable: -"), "empty cell holds the Fable column");
    assert!(has_label(&h, "3 model-Sonnet: -"));
    assert!(has_label(&h, "1 model-Sonnet: -"));
    assert!(has_label(&h, "4 5h: -"), "no usage at all: still a 5-hour cell");

    let h = harness(Which::Accounts, 700.0, false);
    assert!(h.state().accounts_page.last_cards);
    assert!(has_label(&h, "1 model-Fable: 17%"));
    assert!(!has_label(&h, "3 model-Fable: -"), "a card drops a window the account has not got");
    assert!(!has_label(&h, "1 model-Sonnet: -"));
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
    // the active row's Switch is gone (hidden, keeping its space), the old one's is back
    assert!(h.query_by_label("Switch to account 2").is_none());
    assert!(h.query_by_label("Switch to account 1").is_some());

    h.get_by_role_and_label(Role::Button, "Rotate").click();
    h.run();
    h.get_by_label("Best (most quota left)").click();
    h.run();
    assert_eq!(h.state().mock.command_log().last().unwrap().args, ["switch", "--json", "--strategy", "best"]);
}

// e2e "the usage view switches to rings" / "the pace marker can be switched off": the
// header toggles are app settings, saved through the service.
#[test]
fn view_toggles_are_settings() {
    let mut h = harness(Which::Accounts, 1400.0, false);
    h.get_by_role_and_label(Role::Button, "Show usage as rings").click();
    h.run();
    assert_eq!(h.state().mock.settings().usage_view, UsageView::Rings);
    assert!(has_label(&h, "1 5h: 45%"), "rings still name their cells");
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
    let submit = |h: &Harness<'_, World>| h.get_by_role_and_label(Role::Button, "Remove permanently").accesskit_node().is_disabled();
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
    assert!(rows.iter().any(|m| m.path == "/tmp/picked-project" && m.email == "you@example.com"), "mapped to the active account");
    assert!(has_label(&h, "Unmap /tmp/picked-project"), "the list reloaded");
    h.get_by_role_and_label(Role::Button, "Unmap /tmp/picked-project").click();
    h.run();
    h.run();
    assert!(!has_label(&h, "Unmap /tmp/picked-project"));
}

/// Renders for review. Widths: cards (700), the scroll band (880: list 832 between 780
/// and the template's minimum), and the table (1400, and 1240 = the README floor).
#[test]
fn renders() {
    if std::env::var_os("CSWAP_UI_SHOTS").is_none() {
        return;
    }
    for dark in [false, true] {
        let theme = if dark { "dark" } else { "light" };
        for (w, name) in [(700.0, "cards"), (880.0, "scroll"), (1240.0, "1240"), (1400.0, "table")] {
            let mut h = harness(Which::Accounts, w, dark);
            shot(&mut h, &format!("accounts-{name}-{theme}"));
        }
        let mut world = World::new(Which::Accounts, dark);
        world.mock.set_settings(AppSettings { usage_view: UsageView::Rings, ..AppSettings::default() });
        world.settings = world.mock.settings();
        let mut h = harness_with(world, 1400.0);
        shot(&mut h, &format!("accounts-rings-{theme}"));
        let mut world = World::new(Which::Accounts, dark);
        world.mock.set_settings(AppSettings { usage_view: UsageView::Rings, ..AppSettings::default() });
        world.settings = world.mock.settings();
        let mut h = harness_with(world, 700.0);
        shot(&mut h, &format!("accounts-rings-cards-{theme}"));

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
    world.mock.set_list(payload, Some(cswap_core::CswapError::new("Timeout", "cswap list --json took longer than 30 s")));
    let mut h = harness_with(world, 1240.0);
    shot(&mut h, "accounts-stale-light");
}
