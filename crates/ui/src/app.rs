//! The window's root: caches pushed state, applies the theme, lays out the shell and the
//! current page. OWNER: ui-shell. The signature of [`CswapApp::new`] is the contract the
//! app crate builds against.

use std::sync::Mutex;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use cswap_core::{
    AccountsState, AppSettings, AutoStatus, CswapBinaryInfo, Event, Page, Pending, ThemeSetting, UpdaterState,
    UpdaterStatus,
};
use egui::{Frame, Key, KeyboardShortcut, Modifiers, Ui};

use crate::Services;
use crate::cx::Cx;
use crate::pages::{
    accounts::AccountsPage, auto::AutoPage, log::LogPage, mappings::MappingsPage, onboarding::OnboardingPage,
    settings::SettingsPage,
};
use crate::palette::Palette;
use crate::shell::{self, ShellAction, ShellView};
use crate::style;
use crate::toast::Toasts;

/// Window shown or focused: re-read the list once, but not more often than this. Kept for
/// the whole process (a static), because the app builds a fresh window each time it opens.
pub const FOCUS_REFRESH_SECS: f64 = 10.0;
static LAST_FOCUS_REFRESH: Mutex<Option<Instant>> = Mutex::new(None);

/// Whether a focus/show should refresh, given when the last one did (seconds ago).
pub fn focus_refresh_due(enabled: bool, since_last: Option<f64>) -> bool {
    enabled && since_last.is_none_or(|s| s >= FOCUS_REFRESH_SECS)
}

pub const SHORTCUT_PALETTE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::K);
pub const SHORTCUT_REFRESH: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::R);
pub const SHORTCUT_SETTINGS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma);

pub struct CswapApp {
    services: Services,
    events: Receiver<Event>,
    /// Exposed for tests: the page on screen.
    pub page: Page,
    accounts: AccountsState,
    settings: AppSettings,
    applied_theme: Option<ThemeSetting>,
    binary: Option<CswapBinaryInfo>,
    auto: AutoStatus,
    updater: UpdaterState,
    pub toasts: Toasts,
    pub palette: Palette,
    accounts_page: AccountsPage,
    auto_page: AutoPage,
    mappings_page: MappingsPage,
    settings_page: SettingsPage,
    log_page: LogPage,
    onboarding: OnboardingPage,
    p_refresh: Option<Pending<AccountsState>>,
    focused: Option<bool>,
    started: bool,
    chrome: crate::WindowChrome,
}

impl CswapApp {
    /// `events` carries everything pushed while this window is open; current state comes
    /// from the services' getters, because the app may build a fresh window each time.
    pub fn new(cc: &eframe::CreationContext<'_>, services: Services, events: Receiver<Event>) -> Self {
        Self::with_context(&cc.egui_ctx, services, events)
    }

    /// [`CswapApp::new`] without eframe (tests, screenshots): installs fonts and style on `ctx`.
    pub fn with_context(ctx: &egui::Context, services: Services, events: Receiver<Event>) -> Self {
        let settings = services.cswap.settings();
        style::setup(ctx, settings.theme);
        let mut accounts = services.cswap.accounts();
        if accounts.payload.is_none() && accounts.error.is_none() {
            // Nothing read yet: the first refresh is running (0.1.23's store started so).
            accounts.refreshing = true;
        }
        CswapApp {
            page: Page::Accounts,
            accounts,
            applied_theme: Some(settings.theme),
            binary: services.cswap.binary(),
            auto: services.cswap.auto_status(),
            updater: services.updates.state(),
            settings,
            services,
            events,
            toasts: Toasts::default(),
            palette: Palette::default(),
            accounts_page: AccountsPage::new(),
            auto_page: AutoPage::new(),
            mappings_page: MappingsPage::new(),
            settings_page: SettingsPage::new(),
            log_page: LogPage::new(),
            onboarding: OnboardingPage::new(),
            p_refresh: None,
            focused: None,
            started: false,
            chrome: crate::WindowChrome::Native,
        }
    }

    /// Fixes the clock the pages read (Unix ms; `None` keeps the wall clock) and the UTC
    /// offset of every wall-clock text, so tests and README renders come out the same on
    /// any machine.
    pub fn pin_clock(&mut self, now_ms: Option<i64>, utc_offset: i64) {
        self.accounts_page.now_ms = now_ms;
        self.accounts_page.utc_offset = Some(utc_offset);
        self.auto_page.now_ms = now_ms;
        self.auto_page.utc_offset = Some(utc_offset);
        self.log_page.utc_offset = Some(utc_offset);
        self.settings_page.utc_offset = Some(utc_offset);
    }

    /// Who draws the title bar; the app crate sets this to match how it built the window
    /// (undecorated on Windows/Linux, a transparent title bar on macOS). `Native` by default,
    /// so tests and the demo keep the OS bar.
    pub fn set_chrome(&mut self, chrome: crate::WindowChrome) {
        self.chrome = chrome;
    }

    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    fn on_event(&mut self, ev: Event) {
        match &ev {
            Event::Accounts(a) => {
                self.accounts = a.clone();
            }
            Event::AutoStatus(s) => self.auto = s.clone(),
            Event::Settings(s) => self.settings = s.clone(),
            Event::Binary(b) => self.binary = Some(b.clone()),
            Event::Updater(u) => {
                if u.status == UpdaterStatus::Downloaded && self.updater.status != UpdaterStatus::Downloaded {
                    self.toasts.info(format!(
                        "Update {} downloaded\nRestart from Settings → About to install it; otherwise it installs when you quit.",
                        u.version.as_deref().unwrap_or("")
                    ));
                }
                self.updater = u.clone();
            }
            Event::Navigate(p) => self.page = *p,
            Event::AutoEvent(_) | Event::CommandLog(_) => {}
        }
        self.settings_page.on_event(&ev);
        self.log_page.on_event(&ev);
        self.onboarding.on_event(&ev);
        self.accounts_page.on_event(&ev);
        self.auto_page.on_event(&ev);
        self.mappings_page.on_event(&ev);
    }

    fn refresh(&mut self) {
        self.accounts.refreshing = true;
        self.p_refresh = Some(self.services.cswap.refresh());
    }

    fn refresh_on_focus(&mut self) {
        let mut last = LAST_FOCUS_REFRESH.lock().unwrap_or_else(|e| e.into_inner());
        let since = last.map(|t| t.elapsed().as_secs_f64());
        if focus_refresh_due(self.settings.refresh_on_focus, since) {
            *last = Some(Instant::now());
            drop(last);
            self.refresh();
        }
    }

    /// One frame of the whole window into `ui` (the root ui of eframe or of a test harness).
    pub fn frame(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        while let Ok(ev) = self.events.try_recv() {
            self.on_event(ev);
        }
        // Cheap (a clone under a lock), and it makes every edit visible on the next frame
        // whether or not the service also pushes `Event::Settings`.
        self.settings = self.services.cswap.settings();
        // Same for the binary: a detection started anywhere (Settings, onboarding, the
        // driver's own) shows in the sidebar and lifts onboarding without its event.
        self.binary = self.services.cswap.binary().or(self.binary.take());
        if self.applied_theme != Some(self.settings.theme) {
            style::apply_theme(&ctx, self.settings.theme);
            self.applied_theme = Some(self.settings.theme);
        }
        if let Some(p) = &mut self.p_refresh {
            if let Some(a) = p.take() {
                self.accounts = a;
                self.p_refresh = None;
            } else if p.is_lost() {
                self.accounts.refreshing = false;
                self.p_refresh = None;
            }
        }

        // Window shown (this window was just built) or focus regained.
        let focused = ctx.input(|i| i.focused);
        if !self.started || (focused && self.focused == Some(false)) {
            self.started = true;
            self.refresh_on_focus();
        }
        self.focused = Some(focused);

        // Shortcuts work wherever focus is; they are consumed so a text field never sees them.
        let (k_palette, k_refresh, k_settings) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&SHORTCUT_PALETTE),
                i.consume_shortcut(&SHORTCUT_REFRESH),
                i.consume_shortcut(&SHORTCUT_SETTINGS),
            )
        });
        if k_palette {
            self.palette.toggle();
        }
        if k_refresh {
            self.refresh();
        }
        if k_settings {
            self.page = Page::Settings;
        }

        let tokens = style::tokens(&ctx);
        let width = ctx.content_rect().width();
        let (narrow, forced) = shell::sidebar_narrow(width, self.settings.sidebar_collapsed);
        let target = if narrow { shell::SIDEBAR_NARROW } else { shell::SIDEBAR_WIDE };
        let sidebar_w =
            ctx.animate_value_with_time(egui::Id::unique("sidebar-w"), target, crate::theme::TRANSITION_SECS);

        let mut act = ShellAction::default();
        {
            let view = ShellView {
                page: self.page,
                settings: &self.settings,
                accounts: &self.accounts,
                auto: &self.auto,
                binary: self.binary.as_ref(),
                updater: &self.updater,
                mac: self.services.host.platform() == "macos",
                chrome: self.chrome,
                now_ms: cswap_core::format::now_ms(),
            };
            egui::Panel::top("titlebar")
                .exact_size(shell::TITLEBAR_H)
                .resizable(false)
                .show_separator_line(false)
                .frame(Frame::NONE)
                .show(ui, |ui| shell::title_bar(ui, &view, &mut act));
            egui::Panel::left("sidebar")
                .exact_size(sidebar_w)
                .resizable(false)
                .show_separator_line(false)
                .frame(Frame::NONE)
                .show(ui, |ui| shell::sidebar(ui, &view, narrow, forced, &mut act));
        }

        let mut navigate: Option<Page> = None;
        egui::CentralPanel::no_frame().show(ui, |ui| {
            ui.painter().rect_filled(ui.max_rect(), 0.0, tokens.bg);
            let needs_onboarding = self.binary.as_ref().is_some_and(|b| b.version.is_none())
                && !matches!(self.page, Page::Settings | Page::Log);
            let mut cx = Cx {
                services: &self.services,
                tokens: &tokens,
                settings: &self.settings,
                accounts: &self.accounts,
                auto: &self.auto,
                toasts: &mut self.toasts,
                navigate: &mut navigate,
            };
            let page = self.page;
            // Pages get the whole content rect and scroll themselves (vertically, and the
            // Accounts list also sideways in the 780–956 px band), so nothing here scrolls.
            ui.spacing_mut().item_spacing.y = 0.0;
            if needs_onboarding {
                self.onboarding.ui(ui, &mut cx);
            } else {
                match page {
                    Page::Settings => self.settings_page.ui(ui, &mut cx),
                    Page::Log => self.log_page.ui(ui, &mut cx),
                    Page::Accounts => self.accounts_page.ui(ui, &mut cx),
                    Page::Auto => self.auto_page.ui(ui, &mut cx),
                    Page::Mappings => self.mappings_page.ui(ui, &mut cx),
                }
            }
            self.palette.poll(&mut cx);
            self.palette.show(&ctx, &mut cx);
        });

        if let Some(p) = act.navigate.or(navigate) {
            self.page = p;
        }
        if act.toggle_collapsed {
            let now = self.settings.sidebar_collapsed;
            self.settings =
                self.services.cswap.edit_settings(Box::new(move |s: &mut AppSettings| s.sidebar_collapsed = !now));
        }
        if act.open_palette {
            self.palette.open_fresh();
        }
        if act.refresh {
            self.refresh();
        }
        self.toasts.show(&ctx);
        // Last, so the resize bands sit above everything drawn this frame.
        if self.chrome == crate::WindowChrome::Custom {
            crate::chrome::window_edges(&ctx);
        }
    }
}

impl eframe::App for CswapApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // macOS: centre the traffic lights in the 38 px header, 14 px in (0.1.23's
        // `trafficLightPosition`); AppKit's default puts them in a 28 px bar, off the header's
        // middle. AppKit moves them back on every layout, so this runs every frame.
        if self.chrome == crate::WindowChrome::MacOs {
            frame.set_traffic_lights_position(ui.ctx(), egui::Rangef::new(0.0, shell::TITLEBAR_H), 14.0);
        }
        self.frame(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_refresh_is_throttled_to_ten_seconds() {
        assert!(focus_refresh_due(true, None), "first focus refreshes");
        assert!(!focus_refresh_due(true, Some(9.9)));
        assert!(focus_refresh_due(true, Some(10.0)));
        assert!(!focus_refresh_due(false, None), "the setting turns it off");
    }
}
