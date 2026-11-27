//! Port of `src/renderer/src/pages/Settings.tsx`. OWNER: ui-shell.
//!
//! Four cards in a two-column grid from 1024 px of window (Tailwind `lg`), one column below:
//! the cswap binary (path, detection, upgrade, install with uv), appearance & behaviour (the
//! app's own settings), data (cswap's vault root, unclaimed credential entries) and about
//! (version, updater). Cards in one grid row share a height, as CSS grid rows do.

use cswap_core::{
    AppSettings, BinarySource, CsResult, CswapBinaryInfo, Event, Pending, PlainOutput, ThemeSetting, UnclaimedEntry,
    UpdaterState, UpdaterStatus,
};
use egui::{Align, CornerRadius, Frame, Layout, Margin, Stroke, Ui, text::LayoutJob, vec2};

use crate::cx::Cx;
use crate::style;
use crate::widgets::{self, Button, Tone, icon};

const GITHUB: &str = "https://github.com/TheSkinnyRat/cswap-desktop";
const UPSTREAM: &str = "https://github.com/realiti4/claude-swap";
/// Tailwind `lg`: two columns from here (window width).
const TWO_COLUMNS_FROM: f32 = 1024.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Busy {
    Path,
    Upgrade,
    Install,
    Update,
}

#[derive(Default)]
pub struct SettingsPage {
    init: bool,
    path_draft: String,
    /// The setting the draft was last synced from, so a change elsewhere resets the draft.
    synced_path: Option<Option<String>>,
    binary: Option<CswapBinaryInfo>,
    updater: UpdaterState,
    vault: Option<String>,
    unclaimed: Option<Vec<UnclaimedEntry>>,
    unclaimed_for: Option<Option<String>>,
    busy: Option<Busy>,
    show_locations: bool,
    p_detect: Option<Pending<CswapBinaryInfo>>,
    p_out: Option<(Busy, Pending<CsResult<PlainOutput>>)>,
    p_unclaimed: Option<Pending<CsResult<Vec<UnclaimedEntry>>>>,
    p_purge: Option<(String, Pending<CsResult<PlainOutput>>)>,
    p_update: Option<Pending<UpdaterState>>,
    /// After a manual check on a disabled updater: the version found, to offer its page.
    found: Option<String>,
    /// Natural heights of the four cards last frame, for equal-height grid rows.
    heights: [f32; 4],
}

impl SettingsPage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_event(&mut self, ev: &Event) {
        match ev {
            Event::Binary(b) => self.binary = Some(b.clone()),
            Event::Updater(u) => self.updater = u.clone(),
            _ => {}
        }
    }

    fn poll(&mut self, cx: &mut Cx) {
        if !self.init {
            self.init = true;
            self.binary = cx.services.cswap.binary();
            self.updater = cx.services.updates.state();
            self.vault = cx.services.cswap.vault_path();
        }
        // The draft follows the setting whenever the setting itself changes.
        if self.synced_path.as_ref() != Some(&cx.settings.cswap_path) {
            self.synced_path = Some(cx.settings.cswap_path.clone());
            self.path_draft = cx.settings.cswap_path.clone().unwrap_or_default();
        }
        // Vault and unclaimed entries are re-read when the binary changes (0.1.23: on
        // `binary?.path`), since another cswap may keep another vault.
        let path = self.binary.as_ref().and_then(|b| b.path.clone());
        if self.unclaimed_for.as_ref() != Some(&path) {
            self.unclaimed_for = Some(path);
            self.vault = cx.services.cswap.vault_path();
            self.p_unclaimed = Some(cx.services.cswap.list_unclaimed());
        }
        if let Some(p) = &mut self.p_unclaimed
            && let Some(r) = p.take()
        {
            self.unclaimed = Some(r.unwrap_or_default());
            self.p_unclaimed = None;
        }
        if let Some(p) = &mut self.p_detect
            && let Some(b) = p.take()
        {
            self.binary = Some(b);
            self.p_detect = None;
            if self.busy == Some(Busy::Path) {
                self.busy = None;
            }
        }
        if let Some((kind, p)) = &mut self.p_out {
            let kind = *kind;
            if let Some(r) = p.take() {
                self.p_out = None;
                self.busy = None;
                match (kind, r) {
                    (Busy::Upgrade, Ok(o)) => {
                        let last = o.stdout.trim().lines().last().unwrap_or("").to_owned();
                        cx.toasts.success(format!("cswap upgrade finished\n{last}"));
                    }
                    (Busy::Upgrade, Err(e)) => cx.toasts.error(format!("Upgrade failed\n{}", e.message)),
                    (_, Ok(_)) => {
                        cx.toasts.success("claude-swap installed with uv");
                        self.p_detect = Some(cx.services.cswap.detect_binary());
                    }
                    (_, Err(e)) => cx.toasts.error(format!("Install failed\n{}", e.message)),
                }
            } else if p.is_lost() {
                self.p_out = None;
                self.busy = None;
            }
        }
        if let Some((id, p)) = &mut self.p_purge
            && let Some(r) = p.take()
        {
            match r {
                Ok(_) => cx.toasts.success(format!("Purged {id}")),
                Err(e) => cx.toasts.error(format!("Purge failed\n{}", e.message)),
            }
            self.p_purge = None;
            self.p_unclaimed = Some(cx.services.cswap.list_unclaimed());
        }
        if let Some(p) = &mut self.p_update
            && let Some(u) = p.take()
        {
            self.p_update = None;
            self.busy = None;
            let manual = self.updater.status == UpdaterStatus::Disabled;
            self.updater = u.clone();
            if manual {
                self.report_manual_check(&u, cx);
            }
        }
    }

    /// The disabled updater's "Check for updates": say what was found, like 0.1.23's
    /// GitHub-API check did, and offer the release page when there is something newer.
    fn report_manual_check(&mut self, u: &UpdaterState, cx: &mut Cx) {
        let newer = u.version.as_ref().filter(|v| newer_than(v, &u.current)).cloned();
        match (u.status, newer) {
            (UpdaterStatus::Error, _) => cx
                .toasts
                .error(format!("Could not check for updates\n{}", u.error.as_deref().unwrap_or("unknown error"))),
            (_, Some(v)) => {
                cx.toasts.info(format!("Version {v} is available\nOpen the releases page to download it."));
                self.found = Some(v);
            }
            (UpdaterStatus::NotAvailable, None) => cx.toasts.success("You are on the latest version"),
            (_, None) if u.version.is_some() => cx.toasts.success("You are on the latest version"),
            _ => {
                cx.toasts.info("Updates are off for this copy\nClaude Swap on GitHub (below) lists the latest release.")
            }
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        self.poll(cx);
        widgets::page_header(ui, "Settings", None, |_| {});
        let two = ui.ctx().content_rect().width() >= TWO_COLUMNS_FROM;
        let mut heights = self.heights;
        Frame::new().inner_margin(Margin { left: 24, right: 24, top: 0, bottom: 32 }).show(ui, |ui| {
            let full = ui.available_width();
            let gap = 16.0;
            let col = if two { (full - gap) / 2.0 } else { full };
            let rows: [[usize; 2]; 2] = [[0, 1], [2, 3]];
            if two {
                for row in rows {
                    let min = heights[row[0]].max(heights[row[1]]);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for i in row {
                            ui.allocate_ui_with_layout(vec2(col, 0.0), Layout::top_down(Align::Min), |ui| {
                                ui.set_width(col);
                                heights[i] = self.card(i, ui, cx, min);
                            });
                        }
                    });
                    ui.add_space(gap);
                }
            } else {
                for (i, h) in heights.iter_mut().enumerate() {
                    *h = self.card(i, ui, cx, 0.0);
                    ui.add_space(gap);
                }
            }
        });
        if heights != self.heights {
            self.heights = heights;
            ui.ctx().request_repaint();
        }
    }

    fn card(&mut self, i: usize, ui: &mut Ui, cx: &mut Cx, min: f32) -> f32 {
        match i {
            0 => self.binary_card(ui, cx, min),
            1 => appearance_card(ui, cx, min),
            2 => self.data_card(ui, cx, min),
            _ => self.about_card(ui, cx, min),
        }
    }

    // ---- cswap binary ------------------------------------------------------------------------

    fn binary_card(&mut self, ui: &mut Ui, cx: &mut Cx, min: f32) -> f32 {
        let t = cx.tokens.clone();
        let version = self.binary.as_ref().and_then(|b| b.version.clone());
        let (_, natural) = widgets::card(
            ui,
            Some("cswap binary"),
            |ui| {
                match &version {
                    Some(v) => widgets::chip(ui, &format!("v{v}"), Tone::Ok),
                    None => widgets::chip(ui, "not found", Tone::Danger),
                };
            },
            min,
            |ui| {
                body_frame(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 12.0;
                    match self.binary.as_ref().and_then(|b| b.path.clone().map(|p| (b, p))) {
                        Some((b, path)) => {
                            let mut job = LayoutJob::default();
                            let f = |font, color| egui::TextFormat::simple(font, color);
                            job.append("Using ", 0.0, f(style::body(12.5), t.text_2));
                            job.append(&path, 0.0, f(style::mono(12.0), t.text));
                            job.append(&format!(" ({})", source_text(b.source)), 0.0, f(style::body(12.5), t.text_3));
                            job.wrap.max_width = ui.available_width();
                            ui.add(egui::Label::new(job).selectable(true));
                            if let Some(e) = &b.error {
                                widgets::para(ui, e, style::body(12.5), t.danger);
                            }
                        }
                        None => {
                            widgets::para(
                                ui,
                                "No cswap executable was found in the usual places or on PATH.",
                                style::body(12.5),
                                t.text_2,
                            );
                        }
                    }
                    widgets::field(
                        ui,
                        "Custom path",
                        Some("Leave empty to auto-detect (uv / pipx / PATH)."),
                        false,
                        |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                let browse_w = 92.0;
                                let save_w = 56.0;
                                let w = (ui.available_width() - browse_w - save_w - 16.0).max(120.0);
                                let placeholder = if cx.services.host.platform() == "windows" {
                                    r"C:\Users\you\.local\bin\cswap.exe"
                                } else {
                                    "~/.local/bin/cswap"
                                };
                                widgets::input(ui, "Custom path", &mut self.path_draft, placeholder, true, w);
                                if Button::new("Browse").icon(icon::FOLDER_OPEN).show(ui).clicked()
                                    && let Some(p) = cx.services.host.pick_file("Locate cswap", &[])
                                {
                                    self.path_draft = p.clone();
                                    self.save_path(Some(p), cx);
                                }
                                let draft = Some(self.path_draft.clone()).filter(|s| !s.is_empty());
                                let changed = draft != cx.settings.cswap_path;
                                let save = Button::new("Save")
                                    .primary()
                                    .loading(self.busy == Some(Busy::Path))
                                    .enabled(changed)
                                    .show(ui);
                                if save.clicked() {
                                    let p = Some(self.path_draft.trim().to_owned()).filter(|s| !s.is_empty());
                                    self.save_path(p, cx);
                                }
                            });
                        },
                    );
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                        if Button::new("Detect again").small().icon(icon::REFRESH_CW).show(ui).clicked() {
                            self.p_detect = Some(cx.services.cswap.detect_binary());
                        }
                        let up = Button::new("Upgrade cswap")
                            .small()
                            .loading(self.busy == Some(Busy::Upgrade))
                            .enabled(version.is_some())
                            .show(ui);
                        if up.clicked() {
                            self.busy = Some(Busy::Upgrade);
                            self.p_out = Some((Busy::Upgrade, cx.services.cswap.upgrade_cswap()));
                        }
                        if version.is_none() {
                            let inst = Button::new("Install with uv")
                                .small()
                                .primary()
                                .loading(self.busy == Some(Busy::Install))
                                .show(ui);
                            if inst.clicked() {
                                self.busy = Some(Busy::Install);
                                self.p_out = Some((Busy::Install, cx.services.cswap.install_cswap()));
                            }
                        }
                    });
                    if let Some(b) = self.binary.as_ref().filter(|b| !b.candidates.is_empty()) {
                        let caret = if self.show_locations { icon::CHEVRON_DOWN } else { icon::CHEVRON_RIGHT };
                        let r = ui
                            .horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                widgets::icon(ui, caret, 11.0, t.text_3);
                                widgets::text(ui, "Locations checked", style::body(11.5), t.text_3);
                            })
                            .response
                            .interact(egui::Sense::click());
                        r.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::Button,
                                true,
                                self.show_locations,
                                "Locations checked",
                            )
                        });
                        if r.clicked() {
                            self.show_locations = !self.show_locations;
                        }
                        if self.show_locations {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            for c in b.candidates.iter().map(String::as_str).chain(["PATH"]) {
                                widgets::selectable(ui, c, style::mono(11.5), t.text_3);
                            }
                        }
                    }
                });
            },
        );
        natural
    }

    fn save_path(&mut self, p: Option<String>, cx: &mut Cx) {
        self.busy = Some(Busy::Path);
        cx.services.cswap.edit_settings(Box::new(move |s: &mut AppSettings| s.cswap_path = p));
        // Detect at once with the new path; the driver may also redetect on its own, which
        // only costs one more `--version`.
        self.p_detect = Some(cx.services.cswap.detect_binary());
    }

    // ---- data --------------------------------------------------------------------------------

    fn data_card(&mut self, ui: &mut Ui, cx: &mut Cx, min: f32) -> f32 {
        let t = cx.tokens.clone();
        let (_, natural) = widgets::card(
            ui,
            Some("Data"),
            |_| {},
            min,
            |ui| {
                body_frame(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    caps(ui, "cswap vault (backup root)", &t);
                    ui.horizontal(|ui| {
                        let vault = self.vault.clone();
                        let open_w = if vault.is_some() { 72.0 } else { 0.0 };
                        let text = vault.clone().unwrap_or_else(|| "unknown".into());
                        let r = ui
                            .allocate_ui_with_layout(
                                vec2((ui.available_width() - open_w).max(60.0), 28.0),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    let mut job = widgets::job(&text, style::mono(12.0), t.text);
                                    job.wrap.max_rows = 1;
                                    job.wrap.break_anywhere = true;
                                    job.wrap.max_width = ui.available_width();
                                    ui.add(egui::Label::new(job).selectable(true).truncate())
                                },
                            )
                            .inner;
                        widgets::tooltip(ui, &r, &text, widgets::Side::Top);
                        if let Some(v) = vault
                            && Button::new("Open").small().ghost().icon(icon::FOLDER_OPEN).show(ui).clicked()
                        {
                            cx.services.host.open_path(&v);
                        }
                    });
                    widgets::para(
                        ui,
                        "Read for mappings and change detection only. Every write goes through the cswap CLI so the vault stays consistent with the terminal and the TUI.",
                        style::body(11.5),
                        t.text_3,
                    );
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        caps(ui, "Unclaimed credential entries", &t);
                        if let Some(u) = self.unclaimed.as_ref().filter(|u| !u.is_empty()) {
                            widgets::chip(ui, &u.len().to_string(), Tone::Warn);
                        }
                    });
                    match &self.unclaimed {
                        None => {
                            widgets::text(ui, "Loading…", style::body(12.5), t.text_3);
                        }
                        Some(u) if u.is_empty() => {
                            widgets::para(
                                ui,
                                "None — nothing stashed that cswap could not attribute to a slot.",
                                style::body(12.5),
                                t.text_3,
                            );
                        }
                        Some(list) => {
                            let list = list.clone();
                            for u in list {
                                Frame::new()
                                    .stroke(Stroke::new(1.0, t.border))
                                    .corner_radius(CornerRadius::same(crate::theme::RADIUS_MD))
                                    .inner_margin(Margin::symmetric(8, 4))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            widgets::text(ui, &u.id, style::mono(12.0), t.text);
                                            widgets::text(ui, format!("slot {}", u.slot), style::body(12.5), t.text_3);
                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                let purge = Button::new("Purge")
                                                    .small()
                                                    .ghost()
                                                    .icon(icon::TRASH_2)
                                                    .loading(self.p_purge.as_ref().is_some_and(|(id, _)| *id == u.id))
                                                    .show(ui);
                                                if purge.clicked() {
                                                    self.p_purge =
                                                        Some((u.id.clone(), cx.services.cswap.purge_unclaimed(&u.id)));
                                                }
                                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                                    let mut j = widgets::job(&u.reason, style::body(12.5), t.text_3);
                                                    j.wrap.max_rows = 1;
                                                    j.wrap.max_width = ui.available_width();
                                                    ui.add(egui::Label::new(j).selectable(false).truncate());
                                                });
                                            });
                                        });
                                    });
                            }
                        }
                    }
                });
            },
        );
        natural
    }

    // ---- about -------------------------------------------------------------------------------

    fn about_card(&mut self, ui: &mut Ui, cx: &mut Cx, min: f32) -> f32 {
        let t = cx.tokens.clone();
        let (_, natural) = widgets::card(
            ui,
            Some("About"),
            |_| {},
            min,
            |ui| {
                body_frame(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    let mut job = LayoutJob::default();
                    job.append("Claude Swap ", 0.0, egui::TextFormat::simple(style::body_medium(12.5), t.text));
                    job.append(
                        &cx.services.host.app_version(),
                        0.0,
                        egui::TextFormat::simple(style::body(12.5), t.text_2),
                    );
                    ui.add(egui::Label::new(job).selectable(true));
                    widgets::para(
                        ui,
                        "A desktop front-end for claude-swap. It drives the CLI you already have installed; nothing here talks to Anthropic directly.",
                        style::body(12.5),
                        t.text_2,
                    );
                    self.updater_box(ui, cx);
                    ui.horizontal_wrapped(|ui| {
                        if Button::new("Claude Swap on GitHub")
                            .small()
                            .ghost()
                            .icon(icon::EXTERNAL_LINK)
                            .show(ui)
                            .clicked()
                        {
                            cx.services.host.open_external(GITHUB);
                        }
                        if Button::new("claude-swap (upstream)")
                            .small()
                            .ghost()
                            .icon(icon::EXTERNAL_LINK)
                            .show(ui)
                            .clicked()
                        {
                            cx.services.host.open_external(UPSTREAM);
                        }
                    });
                });
            },
        );
        natural
    }

    fn updater_box(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let t = cx.tokens.clone();
        let u = self.updater.clone();
        let version = u.version.clone().unwrap_or_default();
        let (title, sub) = updater_text(&u);
        let r = Frame::new()
            .fill(t.bg.gamma_multiply(0.5))
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(CornerRadius::same(crate::theme::RADIUS_MD))
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 8.0;
                ui.horizontal(|ui| {
                    let full = ui.available_width();
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let buttons = ui
                            .horizontal(|ui| {
                                if let (Some(v), Some(url)) = (&self.found, &u.release_url)
                                    && Button::new(&format!("Get {v}"))
                                        .small()
                                        .primary()
                                        .icon(icon::EXTERNAL_LINK)
                                        .show(ui)
                                        .clicked()
                                {
                                    cx.services.host.open_external(url);
                                }
                                match u.status {
                                    UpdaterStatus::Downloaded => {
                                        if Button::new("Restart & install")
                                            .small()
                                            .primary()
                                            .icon(icon::CIRCLE_ARROW_UP)
                                            .show(ui)
                                            .clicked()
                                        {
                                            cx.services.updates.install();
                                        }
                                    }
                                    UpdaterStatus::Disabled => {
                                        let b = Button::new("Check for updates")
                                            .small()
                                            .icon(icon::CIRCLE_ARROW_UP)
                                            .loading(self.busy == Some(Busy::Update))
                                            .show(ui);
                                        if b.clicked() {
                                            self.busy = Some(Busy::Update);
                                            self.p_update = Some(cx.services.updates.check());
                                        }
                                    }
                                    s => {
                                        let working = matches!(
                                            s,
                                            UpdaterStatus::Checking
                                                | UpdaterStatus::Downloading
                                                | UpdaterStatus::Available
                                        ) || self.p_update.is_some();
                                        if Button::new("Check now")
                                            .small()
                                            .icon(icon::REFRESH_CW)
                                            .loading(working)
                                            .show(ui)
                                            .clicked()
                                        {
                                            self.p_update = Some(cx.services.updates.check());
                                        }
                                    }
                                }
                            })
                            .response
                            .rect
                            .width();
                        ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            ui.set_max_width((full - buttons - 12.0).max(120.0));
                            ui.spacing_mut().item_spacing.y = 1.0;
                            widgets::para(ui, &title, style::body_medium(12.5), t.text);
                            widgets::para(ui, &sub, style::body(11.5), t.text_3);
                        });
                    });
                });
                if matches!(u.status, UpdaterStatus::Downloading | UpdaterStatus::Available) {
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 4.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, CornerRadius::same(2), t.surface_3);
                    let p = u.percent.unwrap_or(0.0).clamp(0.0, 100.0) / 100.0;
                    let target = ui.ctx().animate_value_with_time(ui.scope_id().with("upd-pct"), p, 0.3);
                    let mut f = r;
                    f.set_width(r.width() * target);
                    ui.painter().rect_filled(f, CornerRadius::same(2), t.accent);
                }
                ui.horizontal(|ui| {
                    widgets::text(ui, "Download updates automatically", style::body(12.0), t.text_2);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let on = cx.settings.auto_update;
                        let disabled = u.status == UpdaterStatus::Disabled;
                        if widgets::switch(ui, "Download updates automatically", on, !disabled).clicked() {
                            cx.services.cswap.edit_settings(Box::new(move |s: &mut AppSettings| s.auto_update = !on));
                        }
                    });
                });
            });
        let _ = version;
        widgets::name(ui, &r.response, "Updater");
    }
}

/// The two lines of the updater box for a state (titles of 0.1.23).
fn updater_text(u: &UpdaterState) -> (String, String) {
    let v = u.version.as_deref().unwrap_or("");
    let title = match u.status {
        UpdaterStatus::Disabled => "Automatic updates are off".to_owned(),
        UpdaterStatus::Idle => "Automatic updates on".to_owned(),
        UpdaterStatus::Checking => "Checking for updates…".to_owned(),
        UpdaterStatus::NotAvailable => "You are on the latest version".to_owned(),
        UpdaterStatus::Available => format!("Version {v} found — downloading"),
        UpdaterStatus::Downloading => format!("Downloading {v}… {}%", u.percent.unwrap_or(0.0).round()),
        UpdaterStatus::Downloaded => format!("Version {v} is ready to install"),
        UpdaterStatus::Error => "Update check failed".to_owned(),
    };
    let sub = match u.status {
        UpdaterStatus::Error => u.error.clone().unwrap_or_default(),
        UpdaterStatus::Disabled => u
            .disabled_reason
            .clone()
            .unwrap_or_else(|| "Running from source or a dev build — use the release page.".into()),
        _ => match u.checked_at.as_deref().and_then(local_hm) {
            Some(hm) => format!("Checked {hm} · installs on quit or on request"),
            None => "Checks at start and every 6 hours; installs on quit or on request.".into(),
        },
    };
    (title, sub)
}

/// `a.b.c` > `x.y.z`, numerically; a leading `v` is ignored.
pub fn newer_than(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.trim_start_matches('v').split(['.', '-', '+']).map_while(|p| p.parse().ok()).collect()
    };
    let (a, b) = (parse(latest), parse(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

/// `HH:MM` in local time for an ISO timestamp.
pub fn local_hm(iso: &str) -> Option<String> {
    let dt = chrono::DateTime::parse_from_rfc3339(iso).ok()?;
    Some(dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
}

fn source_text(s: BinarySource) -> String {
    match s {
        BinarySource::Settings => "set here".into(),
        BinarySource::Path => "found on PATH".into(),
        BinarySource::Env => "set by CSWAP_DESKTOP_BIN".into(),
        BinarySource::Uv => "found in uv install dir".into(),
        BinarySource::Pipx => "found in pipx install dir".into(),
        BinarySource::None => "not found".into(),
    }
}

fn body_frame(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    Frame::new().inner_margin(Margin::symmetric(16, 12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
}

/// The small uppercase label of the Data card.
fn caps(ui: &mut Ui, s: &str, t: &crate::theme::Tokens) {
    let mut job = widgets::job(&s.to_uppercase(), style::body_medium(11.0), t.text_3);
    job.sections[0].format.extra_letter_spacing = 0.3;
    ui.add(egui::Label::new(job).selectable(false));
}

fn appearance_card(ui: &mut Ui, cx: &mut Cx, min: f32) -> f32 {
    let s = cx.settings.clone();
    let cswap = cx.services.cswap.clone();
    let edit = move |f: Box<dyn FnOnce(&mut AppSettings) + Send>| {
        cswap.edit_settings(f);
    };
    let (_, natural) = widgets::card(
        ui,
        Some("Appearance & behaviour"),
        |_| {},
        min,
        |ui| {
            body_frame(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 16.0;
                widgets::field(ui, "Theme", None, true, |ui| {
                    let opts = [
                        (ThemeSetting::Light, Some(icon::SUN), "Light"),
                        (ThemeSetting::Dark, Some(icon::MOON), "Dark"),
                        (ThemeSetting::System, Some(icon::MONITOR), "System"),
                    ];
                    if let Some(v) = widgets::segmented(ui, s.theme, &opts) {
                        edit(Box::new(move |s| s.theme = v));
                    }
                });
                widgets::field(
                    ui,
                    "Refresh usage every",
                    Some("cswap itself throttles calls to Anthropic; this only decides how often the list is re-read."),
                    true,
                    |ui| {
                        let opts: Vec<(u32, String)> = [30u32, 60, 120, 300, 600, 0]
                            .into_iter()
                            .map(|v| {
                                let l = match v {
                                    0 => "Never".to_owned(),
                                    v if v < 60 => format!("{v}s"),
                                    v => format!("{} min", v / 60),
                                };
                                (v, l)
                            })
                            .collect();
                        let mut v = s.refresh_seconds;
                        if widgets::select(ui, "Refresh interval", &mut v, &opts, 120.0) {
                            edit(Box::new(move |s| s.refresh_seconds = v));
                        }
                    },
                );
                type Flag = fn(&mut AppSettings) -> &mut bool;
                let toggles: [(&str, Option<&str>, bool, Flag); 7] = [
                    (
                        "Refresh when the window opens or gets focus",
                        Some("At most once every 10 seconds."),
                        s.refresh_on_focus,
                        |s| &mut s.refresh_on_focus,
                    ),
                    (
                        "Hide email addresses",
                        Some("Keeps the first three characters — for screenshots and streams."),
                        s.mask_emails,
                        |s| &mut s.mask_emails,
                    ),
                    (
                        "Close to tray",
                        Some("Closing the window keeps the app running in the tray."),
                        s.close_to_tray,
                        |s| &mut s.close_to_tray,
                    ),
                    ("Start minimized", Some("Start hidden in the tray."), s.start_minimized, |s| {
                        &mut s.start_minimized
                    }),
                    ("Launch at login", None, s.launch_at_login, |s| &mut s.launch_at_login),
                    (
                        "Notify on auto-switch",
                        Some("Desktop notification whenever the auto-switcher changes account."),
                        s.notify_on_auto_switch,
                        |s| &mut s.notify_on_auto_switch,
                    ),
                    ("Start auto-switch with the app", None, s.auto_start_auto_switch, |s| {
                        &mut s.auto_start_auto_switch
                    }),
                ];
                for (label, hint, on, flag) in toggles {
                    widgets::field(ui, label, hint, true, |ui| {
                        if widgets::switch(ui, label, on, true).clicked() {
                            edit(Box::new(move |s| *flag(s) = !on));
                        }
                    });
                }
            });
        },
    );
    natural
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(newer_than("1.0.10", "1.0.9"));
        assert!(newer_than("v1.1.0", "1.0.99"));
        assert!(!newer_than("1.0.0", "1.0.0"));
        assert!(!newer_than("0.9.0", "1.0.0"));
    }

    #[test]
    fn updater_lines_follow_the_state() {
        let mut u = UpdaterState {
            status: UpdaterStatus::Downloading,
            version: Some("1.2.0".into()),
            percent: Some(41.6),
            ..Default::default()
        };
        assert_eq!(updater_text(&u).0, "Downloading 1.2.0… 42%");
        u.status = UpdaterStatus::Disabled;
        u.disabled_reason = Some("Installed from a .deb — update with your package manager.".into());
        assert_eq!(
            updater_text(&u),
            ("Automatic updates are off".into(), "Installed from a .deb — update with your package manager.".into())
        );
        u.status = UpdaterStatus::Error;
        u.error = Some("signature mismatch".into());
        assert_eq!(updater_text(&u).1, "signature mismatch");
    }
}
