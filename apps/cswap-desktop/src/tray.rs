//! The tray icon and its menu, as 0.1.23's `src/main/tray.ts`. The menu is plain data
//! ([`menu`]) so its states are tested anywhere; [`Tray`] only turns it into a tray-icon
//! menu and rebuilds it when the data changes.

use cswap_core::{Account, AccountsState, AutoStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Open,
    Switch(u32),
    RotateNext,
    SwitchBest,
    Refresh,
    ToggleAuto,
    Quit,
    /// Left click on the icon: show, or hide when shown (not on macOS, where a click opens
    /// the menu).
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Model {
    pub accounts: Vec<Account>,
    pub auto: AutoStatus,
}

impl Model {
    pub fn from_state(state: &AccountsState, auto: &AutoStatus) -> Model {
        Model { accounts: state.payload.as_ref().map(|p| p.accounts.clone()).unwrap_or_default(), auto: auto.clone() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Separator,
    Item { text: String, enabled: bool, action: Action },
}

fn item(text: impl Into<String>, enabled: bool, action: Action) -> Entry {
    Entry::Item { text: text.into(), enabled, action }
}

/// JS `Math.round` for the non-negative percentages cswap reports.
fn round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

/// One account's menu line (tray.ts `label`).
pub fn label(a: &Account) -> String {
    let who = match a.alias.as_deref().filter(|s| !s.is_empty()) {
        Some(alias) => format!("{alias} · {}", a.email),
        None => a.email.clone(),
    };
    let u = a.usage.as_ref();
    let (five, seven) = (u.and_then(|u| u.five_hour.as_ref()), u.and_then(|u| u.seven_day.as_ref()));
    let pct = if five.is_some() || seven.is_some() {
        format!(" — 5h {}% · 7d {}%", round(five.map_or(0.0, |w| w.pct)), round(seven.map_or(0.0, |w| w.pct)))
    } else if a.usage_status != "ok" {
        format!(" — {}", a.usage_status)
    } else {
        String::new()
    };
    format!(
        "{}{}. {who}{pct}{}",
        if a.active { "● " } else { "   " },
        a.number,
        if a.disabled == Some(true) { " (disabled)" } else { "" }
    )
}

pub fn tooltip(m: &Model) -> String {
    match m.accounts.iter().find(|a| a.active) {
        Some(a) => {
            let name = a.alias.as_deref().filter(|s| !s.is_empty()).unwrap_or(&a.email);
            let usage = a.usage.as_ref();
            let five = usage.and_then(|u| u.five_hour.as_ref());
            let seven = usage.and_then(|u| u.seven_day.as_ref()).map_or(0.0, |w| w.pct);
            let line2 = five.map(|f| format!("\n5h {}% · 7d {}%", round(f.pct), round(seven))).unwrap_or_default();
            format!("Claude Swap — Account-{} {name}{line2}", a.number)
        }
        None => "Claude Swap — no active account".into(),
    }
}

pub fn menu(m: &Model) -> Vec<Entry> {
    let mut v = vec![item("Open Claude Swap", true, Action::Open), Entry::Separator];
    for a in &m.accounts {
        v.push(item(label(a), !a.active, Action::Switch(a.number)));
    }
    if !m.accounts.is_empty() {
        v.push(Entry::Separator);
    }
    let several = m.accounts.len() > 1;
    v.push(item("Rotate to next", several, Action::RotateNext));
    v.push(item("Switch to best", several, Action::SwitchBest));
    v.push(item("Refresh usage", true, Action::Refresh));
    v.push(Entry::Separator);
    let auto = if m.auto.running {
        format!("Stop auto-switch{}", if m.auto.dry_run == Some(true) { " (dry run)" } else { "" })
    } else {
        "Start auto-switch".into()
    };
    v.push(item(auto, true, Action::ToggleAuto));
    v.push(Entry::Separator);
    v.push(item("Quit", true, Action::Quit));
    v
}

/// RGBA pixels of a PNG (the tray and window icons ship as PNG).
pub fn decode_png(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    Some((buf, info.width, info.height))
}

pub const APP_ICON: &[u8] = include_bytes!("../../../resources/icon-256.png");
// macOS wants a template (black on transparent, tinted by the OS); Windows and Linux get
// the white glyph with a dark outline, which survives a light *and* a dark tray. The 2x
// files: the OS scales down on a 100% display and stays sharp on 150-200%.
#[cfg(target_os = "macos")]
const TRAY_ICON: &[u8] = include_bytes!("../../../resources/trayTemplate@2x.png");
#[cfg(not(target_os = "macos"))]
const TRAY_ICON: &[u8] = include_bytes!("../../../resources/tray@2x.png");

pub use native::Tray;

mod native {
    use std::sync::mpsc::{Receiver, Sender, channel};

    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

    use super::{Action, Entry, Model, TRAY_ICON, decode_png, menu, tooltip};

    pub(super) fn action_id(a: &Action) -> String {
        match a {
            Action::Open => "open".into(),
            Action::Switch(n) => format!("switch:{n}"),
            Action::RotateNext => "rotate".into(),
            Action::SwitchBest => "best".into(),
            Action::Refresh => "refresh".into(),
            Action::ToggleAuto => "auto".into(),
            Action::Quit => "quit".into(),
            Action::Toggle => "toggle".into(),
        }
    }

    pub(super) fn action_of(id: &str) -> Option<Action> {
        Some(match id {
            "open" => Action::Open,
            "rotate" => Action::RotateNext,
            "best" => Action::SwitchBest,
            "refresh" => Action::Refresh,
            "auto" => Action::ToggleAuto,
            "quit" => Action::Quit,
            s => Action::Switch(s.strip_prefix("switch:")?.parse().ok()?),
        })
    }

    pub struct Tray {
        icon: TrayIcon,
        rx: Receiver<Action>,
        shown: Option<Model>,
    }

    impl Tray {
        /// On the main thread (Windows and macOS deliver the tray's events there). `wake`
        /// makes whoever runs the loop look at [`Tray::actions`], window or not.
        pub fn new(wake: impl Fn() + Send + Sync + Clone + 'static) -> Result<Tray, String> {
            let (tx, rx) = channel::<Action>();
            let (menu_tx, menu_wake): (Sender<Action>, _) = (tx.clone(), wake.clone());
            MenuEvent::set_event_handler(Some(std::sync::Arc::new(move |e: MenuEvent| {
                if let Some(a) = action_of(e.id().0.as_str()) {
                    let _ = menu_tx.send(a);
                    menu_wake();
                }
            })));
            TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
                let a = match e {
                    TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } => {
                        Action::Toggle
                    }
                    TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => Action::Open,
                    _ => return,
                };
                let _ = tx.send(a);
                wake();
            }));
            let (rgba, w, h) = decode_png(TRAY_ICON).ok_or("tray icon PNG did not decode")?;
            let image = tray_icon::Icon::from_rgba(rgba, w, h).map_err(|e| e.to_string())?;
            let builder = TrayIconBuilder::new().with_tooltip("Claude Swap").with_menu(Box::new(Menu::new()));
            #[cfg(target_os = "macos")]
            let builder = builder.with_icon_templated(image).with_menu_on_left_click(true);
            #[cfg(not(target_os = "macos"))]
            let builder = builder.with_icon(image).with_menu_on_left_click(false);
            // StatusNotifierItem hosts fall back to the id when there is no title, and
            // tray-icon's default id is "<pid>-<n>" (craft-launcher measured that).
            #[cfg(target_os = "linux")]
            let builder = builder.with_id("claude-swap").with_title("Claude Swap");
            let icon = builder.build().map_err(|e| e.to_string())?;
            Ok(Tray { icon, rx, shown: None })
        }

        pub fn actions(&self) -> Vec<Action> {
            self.rx.try_iter().collect()
        }

        /// Brings tooltip and menu up to date; a no-op when nothing changed.
        pub fn sync(&mut self, m: &Model) {
            if self.shown.as_ref() == Some(m) {
                return;
            }
            let _ = self.icon.set_tooltip(Some(tooltip(m)));
            let built = Menu::new();
            for entry in menu(m) {
                let _ = match entry {
                    Entry::Separator => built.append(&PredefinedMenuItem::separator()),
                    Entry::Item { text, enabled, action } => {
                        built.append(&MenuItem::with_id(action_id(&action), text, enabled, None))
                    }
                };
            }
            self.icon.set_menu(Some(Box::new(built)));
            self.shown = Some(m.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cswap_core::{Usage, UsageWindow};

    fn acct(n: u32, email: &str, active: bool) -> Account {
        Account { number: n, email: email.into(), active, usage_status: "ok".into(), ..Default::default() }
    }

    fn usage(five: f64, seven: f64) -> Option<Usage> {
        Some(Usage {
            five_hour: Some(UsageWindow { pct: five, ..Default::default() }),
            seven_day: Some(UsageWindow { pct: seven, ..Default::default() }),
            ..Default::default()
        })
    }

    #[test]
    fn labels_as_0_1_23() {
        let mut a = acct(1, "a@example.com", true);
        a.usage = usage(12.5, 40.4);
        assert_eq!(label(&a), "● 1. a@example.com — 5h 13% · 7d 40%");
        let mut b = acct(2, "b@example.com", false);
        b.alias = Some("work".into());
        b.usage_status = "token_expired".into();
        b.disabled = Some(true);
        assert_eq!(label(&b), "   2. work · b@example.com — token_expired (disabled)");
        let c = acct(3, "c@example.com", false);
        assert_eq!(label(&c), "   3. c@example.com");
    }

    #[test]
    fn tooltip_names_the_active_account() {
        let mut a = acct(4, "a@example.com", true);
        a.alias = Some(String::new());
        a.usage = usage(7.0, 81.6);
        let m = Model { accounts: vec![a, acct(5, "b@example.com", false)], auto: AutoStatus::default() };
        assert_eq!(tooltip(&m), "Claude Swap — Account-4 a@example.com\n5h 7% · 7d 82%");
        assert_eq!(tooltip(&Model::default()), "Claude Swap — no active account");
    }

    #[test]
    fn menu_switches_to_inactive_accounts_only_and_follows_auto() {
        let m = Model {
            accounts: vec![acct(1, "a@example.com", true), acct(2, "b@example.com", false)],
            auto: AutoStatus::default(),
        };
        let items: Vec<(String, bool, Action)> = menu(&m)
            .into_iter()
            .filter_map(|e| match e {
                Entry::Item { text, enabled, action } => Some((text, enabled, action)),
                Entry::Separator => None,
            })
            .collect();
        let texts: Vec<&str> = items.iter().map(|i| i.0.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Open Claude Swap",
                "● 1. a@example.com",
                "   2. b@example.com",
                "Rotate to next",
                "Switch to best",
                "Refresh usage",
                "Start auto-switch",
                "Quit"
            ]
        );
        assert!(!items[1].1, "the active account is not offered");
        assert_eq!((items[2].1, &items[2].2), (true, &Action::Switch(2)));
        assert!(items[3].1 && items[4].1);

        let one = Model {
            accounts: vec![acct(1, "a@example.com", true)],
            auto: AutoStatus { running: true, dry_run: Some(true), ..Default::default() },
        };
        let m1 = menu(&one);
        assert!(m1.contains(&item("Rotate to next", false, Action::RotateNext)));
        assert!(m1.contains(&item("Stop auto-switch (dry run)", true, Action::ToggleAuto)));
        // No accounts: no account block and no doubled separator.
        let empty = menu(&Model::default());
        assert!(!empty.windows(2).any(|w| w[0] == Entry::Separator && w[1] == Entry::Separator));
    }

    #[test]
    fn menu_ids_round_trip() {
        for a in [
            Action::Open,
            Action::Switch(12),
            Action::RotateNext,
            Action::SwitchBest,
            Action::Refresh,
            Action::ToggleAuto,
            Action::Quit,
        ] {
            assert_eq!(native::action_of(&native::action_id(&a)), Some(a));
        }
        assert_eq!(native::action_of("switch:x"), None);
    }

    #[test]
    fn bundled_icons_decode() {
        for bytes in [TRAY_ICON, APP_ICON] {
            let (rgba, w, h) = decode_png(bytes).expect("decodes");
            assert_eq!(rgba.len(), (w * h * 4) as usize);
        }
    }
}
