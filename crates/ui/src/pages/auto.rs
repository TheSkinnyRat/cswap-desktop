//! Port of `src/renderer/src/pages/Auto.tsx`. OWNER: ui-accounts.
//!
//! The driver runs `cswap auto --json` as a child and pushes each line as
//! [`Event::AutoEvent`]; this page shows them, starts/stops the child, runs one-shot checks
//! (`auto --once`) and edits cswap's own `autoswitch.*` settings through `cswap config`.

use std::collections::HashMap;

use egui::{Align, CornerRadius, Layout, Margin, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};

use cswap_core::format::{now_ms, rel_time, time_of};
use cswap_core::view::{self, ChipTone, SpecKind};
use cswap_core::{AutoEvent, AutoOnceResult, AutoStatus, ConfigPayload, ConfigSetting, CsResult, Event, Pending, PlainOutput};

use super::accounts::{local_offset, notify};
use super::kit::{self, Btn, Icon, Variant};
use crate::cx::Cx;
use crate::theme::Tokens;
use crate::toast::ToastKind;

/// The window keeps the last 500 events, as 0.1.23's store did.
const MAX_EVENTS: usize = 500;

#[derive(Default)]
pub struct AutoPage {
    /// Fixed clock and UTC offset for tests and screenshots.
    pub now_ms: Option<i64>,
    pub utc_offset: Option<i64>,
    events: Option<Vec<AutoEvent>>,
    once_events: Vec<AutoEvent>,
    pub(crate) show_all: bool,
    config: Option<Result<Vec<ConfigSetting>, String>>,
    cfg_load: Option<Pending<CsResult<ConfigPayload>>>,
    /// Input buffers of the number/text settings, by key, while being edited.
    edits: HashMap<String, String>,
    saving: Vec<(String, Option<String>, Pending<CsResult<PlainOutput>>)>,
    toggling: Option<(bool, Pending<AutoStatus>)>,
    checking: Option<(bool, Pending<CsResult<AutoOnceResult>>)>,
}

impl AutoPage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_event(&mut self, ev: &Event) {
        if let (Event::AutoEvent(e), Some(list)) = (ev, self.events.as_mut()) {
            list.push(e.clone());
            if list.len() > MAX_EVENTS {
                let extra = list.len() - MAX_EVENTS;
                list.drain(..extra);
            }
        }
    }

    fn now(&self) -> i64 {
        self.now_ms.unwrap_or_else(now_ms)
    }

    fn offset(&self) -> i64 {
        self.utc_offset.unwrap_or_else(local_offset)
    }

    fn load_config(&mut self, cx: &Cx) {
        self.cfg_load = Some(cx.services.cswap.config());
    }

    fn poll(&mut self, cx: &mut Cx) {
        if self.events.is_none() {
            // A fresh window starts from what the driver kept, then follows the events.
            self.events = Some(cx.services.cswap.auto_events());
            self.load_config(cx);
        }
        if let Some(p) = self.cfg_load.as_mut() {
            match p.take() {
                Some(Ok(c)) => {
                    self.config = Some(Ok(c.settings));
                    self.edits.clear();
                    self.cfg_load = None;
                }
                Some(Err(e)) => {
                    self.config = Some(Err(e.message));
                    self.cfg_load = None;
                }
                None if p.is_lost() => self.cfg_load = None,
                None => {}
            }
        }
        let mut i = 0;
        let mut reload = false;
        while i < self.saving.len() {
            let (key, value, p) = &mut self.saving[i];
            match p.take() {
                Some(r) => {
                    match r {
                        Err(e) => notify(cx, ToastKind::Error, &format!("Could not set {key}"), Some(&e.message)),
                        Ok(_) => {
                            let title = match value {
                                None => format!("{key} reset to default"),
                                Some(v) => format!("{key} = {v}"),
                            };
                            let body = cx.auto.running.then_some("Takes effect the next time auto-switch is started.");
                            notify(cx, ToastKind::Success, &title, body);
                        }
                    }
                    self.saving.remove(i);
                    reload = true;
                }
                None if p.is_lost() => {
                    self.saving.remove(i);
                }
                None => i += 1,
            }
        }
        if reload {
            self.load_config(cx);
        }
        if let Some((stopping, p)) = self.toggling.as_mut() {
            if let Some(st) = p.take() {
                if *stopping {
                    notify(cx, ToastKind::Info, "Auto-switch stopped", None);
                } else if !st.running {
                    notify(cx, ToastKind::Error, "Auto-switch did not start", st.last_error.as_deref());
                }
                self.toggling = None;
            } else if p.is_lost() {
                self.toggling = None;
            }
        }
        if let Some((dry, p)) = self.checking.as_mut() {
            let dry = *dry;
            match p.take() {
                Some(Ok(r)) => {
                    self.once_events.extend(r.events.iter().cloned());
                    let (tone, label) = view::once_outcome(r.outcome, r.exit_code);
                    let kind = match tone {
                        ChipTone::Ok => ToastKind::Success,
                        ChipTone::Danger => ToastKind::Error,
                        _ => ToastKind::Info,
                    };
                    notify(cx, kind, &label, dry.then_some("dry run"));
                    self.checking = None;
                }
                Some(Err(e)) => {
                    notify(cx, ToastKind::Error, "Check failed", Some(&e.message));
                    self.checking = None;
                }
                None if p.is_lost() => self.checking = None,
                None => {}
            }
        }
    }

    fn save(&mut self, cx: &Cx, key: &str, value: Option<String>) {
        let p = cx.services.cswap.set_config(key, value.as_deref());
        self.saving.push((key.to_string(), value, p));
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        self.poll(cx);
        let t = cx.tokens.clone();
        let auto = cx.auto.clone();
        let dry_setting = cx.settings.auto_dry_run;
        let (now, off) = (self.now(), self.offset());
        if self.now_ms.is_none() {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs(15));
        }
        let mut all: Vec<AutoEvent> = self.events.clone().unwrap_or_default();
        all.extend(self.once_events.iter().cloned());
        all.sort_by(|a, b| a.ts.cmp(&b.ts));

        let mut dry = dry_setting;
        let (mut check, mut toggle) = (false, false);
        let mut saves: Vec<(String, Option<String>)> = Vec::new();
        egui::ScrollArea::vertical().id_salt("auto-page").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
            let start = Btn::new(if auto.running { "Stop" } else { "Start" })
                .variant(if auto.running { Variant::Default } else { Variant::Primary })
                .icon(if auto.running { Icon::Pause } else { Icon::Play })
                .loading(self.toggling.is_some());
            let once = Btn::new("Check now").icon(Icon::Zap).loading(self.checking.is_some()).enabled(!auto.running);
            let dry_w = 30.0 + 8.0 + kit::measure(ui, "Dry run", kit::font(12.5)).x;
            let actions_w = start.width(ui) + once.width(ui) + dry_w + 16.0;
            kit::page_header(
                ui,
                &t,
                "Auto-switch",
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                        kit::text(ui, "Runs ", 12.5, t.text_2);
                        kit::mono_text(ui, "cswap auto --json", 12.0, t.text_2);
                        kit::text(ui, " as a child process and switches before you hit a limit.", 12.5, t.text_2);
                    });
                },
                actions_w,
                |ui| {
                    toggle = start.show(ui, &t).clicked();
                    check = once.show(ui, &t).on_hover_text("cswap auto --once: evaluate once and switch if needed").clicked();
                    kit::text(ui, "Dry run", 12.5, t.text_2);
                    ui.add_space(-4.0);
                    kit::switch(ui, &t, &mut dry, "Dry run", !auto.running);
                },
            );
            egui::Frame::new().inner_margin(Margin { left: 24, right: 24, top: 0, bottom: 32 }).show(ui, |ui| {
                let w = ui.available_width();
                ui.set_width(w);
                // lg:grid-cols-[minmax(0,1fr)_340px]: side by side when there is room.
                let wide = w >= 760.0;
                let left_w = if wide { w - 340.0 - 16.0 } else { w };
                let top = ui.cursor().min;
                let left = ui
                    .scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(top, vec2(left_w, 100_000.0))), |ui| {
                        ui.set_width(left_w);
                        ui.spacing_mut().item_spacing = vec2(12.0, 16.0);
                        self.stats(ui, &t, &auto, &all, now);
                        self.feed(ui, &t, &auto, &all, off, now);
                    })
                    .response
                    .rect;
                let right_top = if wide { pos2(top.x + left_w + 16.0, top.y) } else { pos2(top.x, left.max.y + 16.0) };
                let right_w = if wide { 340.0 } else { w };
                let right = ui
                    .scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(right_top, vec2(right_w, 100_000.0))), |ui| {
                        ui.set_width(right_w);
                        self.settings_card(ui, &t, &mut saves);
                    })
                    .response
                    .rect;
                ui.allocate_space(vec2(w, left.max.y.max(right.max.y) - top.y));
            });
        });
        if dry != dry_setting {
            cx.services.cswap.edit_settings(Box::new(move |s| s.auto_dry_run = dry));
        }
        if toggle && self.toggling.is_none() {
            let p = if auto.running { cx.services.cswap.auto_stop() } else { cx.services.cswap.auto_start(dry_setting) };
            self.toggling = Some((auto.running, p));
        }
        if check && self.checking.is_none() {
            self.checking = Some((dry_setting, cx.services.cswap.auto_once(dry_setting)));
        }
        for (k, v) in saves {
            self.save(cx, &k, v);
        }
    }

    fn stats(&self, ui: &mut Ui, t: &Tokens, auto: &AutoStatus, all: &[AutoEvent], now: i64) {
        let (state, tone) = view::auto_state(auto);
        let state_sub = if auto.running {
            auto.started_at
                .as_deref()
                .map(|s| format!("since {} · pid {}", cswap_core::format::clock_of(Some(s), now, self.offset()), auto.pid.map(|p| p.to_string()).unwrap_or_default()))
        } else {
            auto.last_error.clone()
        };
        let last_poll = all.iter().rev().find(|e| e.event == "poll");
        let (poll_v, poll_sub) = match last_poll {
            Some(p) => (
                rel_time(Some(&p.ts), now).replace("in ", ""),
                format!("threshold {}%", view::config_text(p.extra.get("threshold").unwrap_or(&serde_json::Value::Null))),
            ),
            None => ("—".into(), "no polls yet".into()),
        };
        let switches = all.iter().filter(|e| e.event == "switch").count();
        let w = ui.available_width();
        let cw = (w - 24.0) / 3.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            stat(ui, t, cw, "STATE", &state, state_sub.as_deref(), tone);
            stat(ui, t, cw, "LAST POLL", &poll_v, Some(&poll_sub), ChipTone::Muted);
            stat(ui, t, cw, "SWITCHES THIS SESSION", &switches.to_string(), Some(&format!("{} events", all.len())), ChipTone::Muted);
        });
    }

    fn feed(&mut self, ui: &mut Ui, t: &Tokens, auto: &AutoStatus, all: &[AutoEvent], off: i64, now: i64) {
        let w = ui.available_width();
        let shown: Vec<&AutoEvent> = all.iter().filter(|e| self.show_all || view::is_important(e)).collect();
        let max_h = (ui.ctx().content_rect().height() * 0.52).max(260.0);
        kit::card_frame(t).show(ui, |ui| {
            ui.set_width(w - 2.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            // header with the Important / All segmented control
            let (hr, _) = ui.allocate_exact_size(vec2(w - 2.0, 44.0), Sense::hover());
            ui.painter().text(pos2(hr.min.x + 16.0, hr.center().y), egui::Align2::LEFT_CENTER, "Event feed", kit::font(13.0), t.text);
            ui.painter().hline(hr.x_range(), hr.max.y - 0.5, Stroke::new(1.0, t.border));
            let seg_w = kit::measure(ui, "Important", kit::font(11.5)).x + kit::measure(ui, "All", kit::font(11.5)).x + 16.0 * 2.0 + 6.0;
            let seg = egui::Rect::from_min_size(pos2(hr.max.x - 16.0 - seg_w, hr.center().y - 12.0), vec2(seg_w, 24.0));
            ui.painter().rect(seg, CornerRadius::same(crate::theme::RADIUS_MD), t.surface_2, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
            let mut x = seg.min.x + 3.0;
            for (label, all_v) in [("Important", false), ("All", true)] {
                let bw = kit::measure(ui, label, kit::font(11.5)).x + 16.0;
                let r = egui::Rect::from_min_size(pos2(x, seg.min.y + 3.0), vec2(bw, 18.0));
                let resp = ui.interact(r, ui.scope_id().with(("feed-filter", label)), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::accesskit::Role::Button, true, self.show_all == all_v, label));
                let on = self.show_all == all_v;
                if on {
                    ui.painter().rect_filled(r, CornerRadius::same(4), t.surface);
                }
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, kit::font(11.5), if on { t.text } else { t.text_2 });
                if resp.clicked() {
                    self.show_all = all_v;
                }
                x += bw;
            }
            egui::ScrollArea::vertical().id_salt("auto-feed").max_height(max_h).min_scrolled_height(260.0).stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(w - 2.0);
                if shown.is_empty() {
                    let (r, _) = ui.allocate_exact_size(vec2(w - 2.0, 260.0), Sense::hover());
                    let msg = if auto.running { "Waiting for the first event…" } else { "Start auto-switch to see polls, decisions and switches here." };
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, msg, kit::font(12.5), t.text_3);
                    return;
                }
                for (i, ev) in shown.iter().enumerate() {
                    feed_line(ui, t, ev, i + 1 == shown.len(), off, now);
                }
            });
        });
    }

    fn settings_card(&mut self, ui: &mut Ui, t: &Tokens, saves: &mut Vec<(String, Option<String>)>) {
        let w = ui.available_width();
        kit::card_frame(t).show(ui, |ui| {
            ui.set_width(w - 2.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let (hr, _) = ui.allocate_exact_size(vec2(w - 2.0, 44.0), Sense::hover());
            ui.painter().text(pos2(hr.min.x + 16.0, hr.center().y), egui::Align2::LEFT_CENTER, "cswap settings", kit::font(13.0), t.text);
            ui.painter().text(pos2(hr.max.x - 16.0, hr.center().y), egui::Align2::RIGHT_CENTER, "settings.json", kit::font(11.0), t.text_3);
            ui.painter().hline(hr.x_range(), hr.max.y - 0.5, Stroke::new(1.0, t.border));
            egui::Frame::new().inner_margin(Margin::symmetric(16, 12)).show(ui, |ui| {
                ui.set_width(w - 34.0);
                ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
                match &self.config {
                    Some(Err(e)) => {
                        kit::para(ui, e.clone(), 12.0, t.danger);
                    }
                    None => {
                        kit::text(ui, "Loading…", 12.0, t.text_3);
                    }
                    Some(Ok(rows)) => {
                        let rows = rows.clone();
                        for spec in view::CONFIG_SPECS {
                            if let Some(row) = rows.iter().find(|r| r.key == spec.key) {
                                setting_row(ui, t, spec, row, &mut self.edits, saves);
                            }
                        }
                    }
                }
                ui.add_space(4.0);
                kit::para(
                    ui,
                    "These are cswap's own settings (cswap config), shared with the CLI and TUI. Changes apply when auto-switch is (re)started.",
                    11.5,
                    t.text_3,
                );
            });
        });
    }
}

fn stat(ui: &mut Ui, t: &Tokens, w: f32, label: &str, value: &str, sub: Option<&str>, tone: ChipTone) {
    ui.allocate_ui_with_layout(vec2(w, 80.0), Layout::top_down(Align::Min), |ui| {
        kit::card_frame(t).inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
            ui.set_width(w - 30.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            kit::text(ui, label, 11.0, t.text_3);
            let color = match tone {
                ChipTone::Ok => t.ok,
                ChipTone::Danger => t.danger,
                _ => t.text,
            };
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if tone == ChipTone::Ok {
                    let (r, _) = ui.allocate_exact_size(vec2(7.0, 20.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.5, t.ok);
                }
                kit::text(ui, value, 15.0, color);
            });
            kit::text(ui, sub.unwrap_or(" "), 11.5, t.text_3);
        });
    });
}

fn feed_line(ui: &mut Ui, t: &Tokens, ev: &AutoEvent, last: bool, off: i64, now: i64) {
    let w = ui.available_width();
    let top = ui.cursor().min;
    let time = time_of(&ev.ts, off);
    let time_w = kit::measure(ui, "00:00:00", kit::mono(11.5)).x;
    let chip_w = kit::chip_width(ui, &ev.event);
    let text_x = top.x + 12.0 + time_w + 8.0 + chip_w + 8.0;
    let text_w = (top.x + w - 12.0 - text_x).max(40.0);
    let g = kit::wrapped(ui, view::describe_event(ev, now, off), kit::mono(11.5), t.text_2, text_w);
    let h = g.size().y.max(18.0) + 12.0;
    let rect = Rect::from_min_size(top, vec2(w, h));
    if ui.rect_contains_pointer(rect) {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, t.surface_2.gamma_multiply(0.4));
    }
    ui.painter().text(pos2(top.x + 12.0, top.y + 6.0 + 9.0), egui::Align2::LEFT_CENTER, time, kit::mono(11.5), t.text_3);
    ui.scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(pos2(top.x + 12.0 + time_w + 8.0, top.y + 6.0), vec2(chip_w, 18.0))), |ui| {
        kit::chip(ui, t, &ev.event, view::event_tone(&ev.event))
    });
    ui.painter().galley(pos2(text_x, top.y + 6.0 + 1.0), g, t.text_2);
    if !last {
        ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, t.border.gamma_multiply(0.6)));
    }
    ui.allocate_rect(rect, Sense::hover());
}

fn setting_row(
    ui: &mut Ui,
    t: &Tokens,
    spec: &view::ConfigSpec,
    row: &ConfigSetting,
    edits: &mut HashMap<String, String>,
    saves: &mut Vec<(String, Option<String>)>,
) {
    let w = ui.available_width();
    let control_w = 120.0;
    let text_w = w - control_w - 12.0;
    let top = ui.cursor().min;
    let label_rect = ui
        .scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(top, vec2(text_w, 200.0))), |ui| {
            ui.set_width(text_w);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                kit::text(ui, spec.label, 12.5, t.text);
                if row.is_set {
                    let g = kit::galley(ui, "reset", kit::font(10.5), t.text_3, 60.0);
                    let (r, resp) = ui.allocate_exact_size(g.size(), Sense::click());
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Button, true, format!("Reset {}", spec.label)));
                    ui.painter().galley(r.min, g, t.text_3);
                    if resp.hovered() {
                        ui.painter().hline(r.x_range(), r.max.y, Stroke::new(1.0, t.text_3));
                    }
                    if resp.on_hover_text("Back to the default").clicked() {
                        saves.push((spec.key.to_string(), None));
                    }
                }
            });
            kit::para(ui, spec.hint, 11.5, t.text_3);
        })
        .response
        .rect;
    let current = view::config_text(&row.value);
    let crect = Rect::from_min_size(pos2(top.x + w - control_w, top.y), vec2(control_w, 32.0));
    ui.scope_builder(UiBuilder::new().max_rect(crect).layout(Layout::right_to_left(Align::Center)), |ui| match spec.kind {
        SpecKind::Bool => {
            let mut on = row.value == serde_json::Value::Bool(true) || row.value == serde_json::Value::String("true".into());
            if kit::switch(ui, t, &mut on, spec.label, true).changed() {
                saves.push((spec.key.to_string(), Some(if on { "true" } else { "false" }.to_string())));
            }
        }
        SpecKind::Select(options) => {
            egui::ComboBox::from_id_salt(("cfg", spec.key)).width(control_w - 8.0).selected_text(current.clone()).show_ui(ui, |ui| {
                for o in options {
                    if ui.selectable_label(current == *o, *o).clicked() && current != *o {
                        saves.push((spec.key.to_string(), Some(o.to_string())));
                    }
                }
            });
        }
        SpecKind::Number | SpecKind::Text => {
            let buf = edits.entry(spec.key.to_string()).or_insert_with(|| current.clone());
            let dirty = *buf != current;
            let r = kit::input(ui, t, buf, spec.key, "", control_w, false, false);
            if dirty {
                ui.painter().rect_stroke(r.rect, CornerRadius::same(crate::theme::RADIUS_MD), Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
            }
            // Committed on Enter or when the field loses focus, only if it changed.
            if r.lost_focus() && *buf != current {
                saves.push((spec.key.to_string(), Some(buf.clone())));
            }
        }
    });
    ui.allocate_space(vec2(w, label_rect.height().max(32.0)));
}
