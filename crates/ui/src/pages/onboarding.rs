//! Port of `src/renderer/src/pages/Onboarding.tsx`. OWNER: ui-shell.
//!
//! Shown instead of the Accounts / Auto / Mappings pages while no working cswap is found.
//! The app does not bundle cswap (two versions writing one vault lose accounts silently),
//! so this explains how to install it and offers install-with-uv, detect again and locate.

use cswap_core::{AppSettings, CsResult, CswapBinaryInfo, Event, Page, Pending, PlainOutput};
use egui::{Align, CornerRadius, Frame, Layout, Margin, Stroke, Ui, vec2};

use crate::cx::Cx;
use crate::style;
use crate::widgets::{self, Button, icon};

#[derive(Default)]
pub struct OnboardingPage {
    init: bool,
    binary: Option<CswapBinaryInfo>,
    p_install: Option<Pending<CsResult<PlainOutput>>>,
    p_detect: Option<Pending<CswapBinaryInfo>>,
}

impl OnboardingPage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_event(&mut self, ev: &Event) {
        if let Event::Binary(b) = ev {
            self.binary = Some(b.clone());
        }
    }

    fn poll(&mut self, cx: &mut Cx) {
        if !self.init {
            self.init = true;
            self.binary = cx.services.cswap.binary();
        }
        if let Some(p) = &mut self.p_install
            && let Some(r) = p.take()
        {
            self.p_install = None;
            match r {
                Ok(_) => {
                    cx.toasts.success("claude-swap installed");
                    self.p_detect = Some(cx.services.cswap.detect_binary());
                }
                Err(e) => cx.toasts.error(format!("Install failed\n{}", e.message)),
            }
        }
        if let Some(p) = &mut self.p_detect
            && let Some(b) = p.take()
        {
            self.p_detect = None;
            self.binary = Some(b);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        self.poll(cx);
        let t = cx.tokens.clone();
        let avail = ui.available_size();
        let width = 560f32.min(avail.x - 48.0);
        // Vertically centred in the page (`flex h-full items-center`), once it is measured.
        let h_id = ui.scope_id().with("onboarding-h");
        let last_h: f32 = ui.ctx().data(|d| d.get_temp(h_id)).unwrap_or(0.0);
        ui.add_space(((avail.y - last_h) / 2.0).max(24.0));
        let r = ui.vertical_centered(|ui| {
            Frame::new()
                .fill(t.surface)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(12))
                .shadow(style::card_shadow(&t))
                .inner_margin(Margin::same(24))
                .show(ui, |ui| {
                    ui.set_width(width - 48.0);
                    ui.with_layout(Layout::top_down(Align::Min), |ui| self.card(ui, cx))
                })
        });
        let h = r.response.rect.height();
        if (h - last_h).abs() > 0.5 {
            ui.ctx().data_mut(|d| d.insert_temp(h_id, h));
            ui.ctx().request_repaint();
        }
        widgets::name(ui, &r.response, "Onboarding");
    }

    fn card(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let t = cx.tokens.clone();
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            widgets::icon(ui, icon::TERMINAL, 16.0, t.accent);
            widgets::text(ui, "cswap is not installed yet", style::body_semibold(15.0), t.text);
        });
        let mut job = egui::text::LayoutJob::default();
        let f = |font, color| egui::TextFormat::simple(font, color);
        job.append("This app is a front-end for the ", 0.0, f(style::body(12.5), t.text_2));
        job.append("claude-swap", 0.0, f(style::body_medium(12.5), t.text));
        job.append(
            " CLI — it does not bundle it, so one install serves the terminal, the TUI and this window. Install it with uv (recommended) or pipx, then detect again.",
            0.0,
            f(style::body(12.5), t.text_2),
        );
        job.wrap.max_width = ui.available_width();
        ui.add(egui::Label::new(job).selectable(false));

        if let Some(b) = &self.binary
            && let (Some(err), Some(path)) = (&b.error, &b.path)
        {
            ui.add_space(8.0);
            Frame::new()
                .fill(t.danger_soft)
                .stroke(Stroke::new(1.0, t.danger.gamma_multiply(0.3)))
                .corner_radius(CornerRadius::same(crate::theme::RADIUS_MD))
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let mut j = egui::text::LayoutJob::default();
                    j.append(path, 0.0, f(style::mono(12.0), t.danger));
                    j.append(&format!(": {err}"), 0.0, f(style::body(12.0), t.danger));
                    j.wrap.max_width = ui.available_width();
                    ui.add(egui::Label::new(j).selectable(true));
                });
        }

        ui.add_space(12.0);
        Frame::new()
            .fill(t.bg)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(CornerRadius::same(crate::theme::RADIUS_LG))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 4.0;
                let mut j = widgets::job("TERMINAL", style::body_medium(10.5), t.text_3);
                j.sections[0].format.extra_letter_spacing = 0.3;
                ui.add(egui::Label::new(j).selectable(false));
                widgets::selectable(ui, "uv tool install claude-swap", style::mono(12.0), t.text);
                widgets::selectable(ui, "# or: pipx install claude-swap", style::mono(12.0), t.text_3);
                widgets::selectable(ui, "cswap add   # after logging into Claude Code", style::mono(12.0), t.text_3);
            });

        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            let install = Button::new("Install with uv")
                .primary()
                .icon(icon::DOWNLOAD)
                .loading(self.p_install.is_some())
                .show(ui);
            if install.clicked() {
                self.p_install = Some(cx.services.cswap.install_cswap());
            }
            if Button::new("Detect again").icon(icon::REFRESH_CW).loading(self.p_detect.is_some()).show(ui).clicked() {
                self.p_detect = Some(cx.services.cswap.detect_binary());
            }
            let locate = if cx.services.host.platform() == "windows" { "Locate cswap.exe" } else { "Locate cswap" };
            if Button::new(locate).icon(icon::FOLDER_OPEN).show(ui).clicked()
                && let Some(p) = cx.services.host.pick_file("Locate cswap", &[])
            {
                cx.services.cswap.edit_settings(Box::new(move |s: &mut AppSettings| s.cswap_path = Some(p)));
                self.p_detect = Some(cx.services.cswap.detect_binary());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if Button::new("Settings").ghost().icon(icon::SETTINGS).show(ui).clicked() {
                    *cx.navigate = Some(Page::Settings);
                }
            });
        });

        ui.add_space(12.0);
        let looked = self
            .binary
            .as_ref()
            .map(|b| b.candidates.join(", "))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "—".into());
        widgets::para(
            ui,
            format!(
                "Looked in: {looked} and PATH. Apps launched from a desktop shortcut can see a shorter PATH than your shell — if cswap works in a terminal but not here, point to the executable directly."
            ),
            style::body(11.5),
            t.text_3,
        );
    }
}
