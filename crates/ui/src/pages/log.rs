//! Port of `src/renderer/src/pages/Log.tsx`. OWNER: ui-shell.
//!
//! Every cswap invocation the app made, newest first: time, arguments, duration, exit code;
//! a click expands stdout/stderr. Tokens never appear here (they travel over stdin).

use cswap_core::{CommandLogEntry, Event};
use egui::{Align, CornerRadius, Frame, Layout, Margin, Sense, Stroke, Ui, pos2, vec2};

use crate::cx::Cx;
use crate::style::{self, mix};
use crate::widgets::{self, Button, Tone, icon};

/// Entries kept in the window (0.1.23's store kept the last 300).
pub const LOG_CAP: usize = 300;

#[derive(Default)]
pub struct LogPage {
    init: bool,
    log: Vec<CommandLogEntry>,
    open: Option<u64>,
}

/// Appends and keeps the newest [`LOG_CAP`].
pub fn push_capped(log: &mut Vec<CommandLogEntry>, e: CommandLogEntry) {
    log.push(e);
    if log.len() > LOG_CAP {
        let extra = log.len() - LOG_CAP;
        log.drain(..extra);
    }
}

/// `toLocaleTimeString([], { hour12: false })`.
fn local_hms(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|d| d.with_timezone(&chrono::Local).format("%H:%M:%S").to_string())
        .unwrap_or_default()
}

impl LogPage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_event(&mut self, ev: &Event) {
        if let Event::CommandLog(e) = ev {
            push_capped(&mut self.log, e.clone());
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        if !self.init {
            self.init = true;
            let mut all = cx.services.cswap.command_log();
            let extra = all.len().saturating_sub(LOG_CAP);
            all.drain(..extra);
            // Events that arrived before the first frame are already in the getter's copy.
            let known: std::collections::HashSet<u64> = all.iter().map(|e| e.id).collect();
            let pending: Vec<_> = self.log.drain(..).filter(|e| !known.contains(&e.id)).collect();
            self.log = all;
            for e in pending {
                push_capped(&mut self.log, e);
            }
        }
        let t = cx.tokens.clone();
        let sub = |ui: &mut Ui| {
            widgets::para(
                ui,
                "Every cswap invocation this app made — arguments, exit code, duration, output. Tokens never appear here (they travel over stdin).",
                style::body(12.5),
                t.text_2,
            );
        };
        let mut clear = false;
        let empty = self.log.is_empty();
        widgets::page_header(ui, "Command log", Some(&sub), |ui| {
            clear = Button::new("Clear").ghost().icon(icon::TRASH_2).enabled(!empty).show(ui).clicked();
        });
        if clear {
            cx.services.cswap.clear_command_log();
            self.log.clear();
            self.open = None;
        }
        Frame::new().inner_margin(Margin { left: 24, right: 24, top: 0, bottom: 32 }).show(ui, |ui| {
            if self.log.is_empty() {
                widgets::empty(ui, icon::SCROLL_TEXT, "Nothing yet", "Commands will show up here as the app talks to cswap.");
                return;
            }
            Frame::new()
                .fill(t.surface)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(crate::theme::RADIUS_LG))
                .shadow(style::card_shadow(&t))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let n = self.log.len();
                    let mut toggled = None;
                    for (k, e) in self.log.iter().rev().enumerate() {
                        if row(ui, e, self.open == Some(e.id), k + 1 == n, &t) {
                            toggled = Some(e.id);
                        }
                    }
                    if let Some(id) = toggled {
                        self.open = if self.open == Some(id) { None } else { Some(id) };
                    }
                });
        });
    }

}

/// One log row (and its output when `open`). Returns true when clicked.
fn row(ui: &mut Ui, e: &CommandLogEntry, open: bool, last: bool, t: &crate::theme::Tokens) -> bool {
    {
        let w = ui.available_width();
        let (r, resp) = ui.allocate_exact_size(vec2(w, 36.0), Sense::click());
        let cmd = format!("cswap {}", e.args.join(" "));
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, open, &cmd));
        let ht = widgets::hover_t(ui, resp.id, resp.hovered());
        ui.painter().rect_filled(r, 0.0, egui::Color32::TRANSPARENT.lerp_to_gamma(t.surface_2.gamma_multiply(0.5), ht));
        let rot = ui.ctx().animate_bool_with_time(resp.id.with("rot"), open, crate::theme::TRANSITION_SECS);
        let c = pos2(r.left() + 12.0 + 6.5, r.center().y);
        let glyph = egui::epaint::TextShape::new(
            c - vec2(6.5, 6.5),
            ui.painter().layout_no_wrap(icon::CHEVRON_RIGHT.into(), style::icon(13.0), t.text_3),
            t.text_3,
        );
        // Rotate around the glyph centre: TextShape turns around its `pos` (top-left).
        let a = rot * std::f32::consts::FRAC_PI_2;
        let off = vec2(6.5, 6.5);
        let rotated = egui::emath::Rot2::from_angle(a) * off;
        ui.painter().add(egui::epaint::TextShape { pos: c - rotated, angle: a, ..glyph });

        let mut child = ui.new_child(
            egui::UiBuilder::new().max_rect(r.shrink2(vec2(12.0, 0.0))).layout(Layout::left_to_right(Align::Center)),
        );
        child.spacing_mut().item_spacing.x = 12.0;
        child.add_space(13.0);
        widgets::text(&mut child, local_hms(&e.at), style::body(11.5), t.text_3);
        child.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let (label, tone) = if e.error.is_some() {
                ("spawn error".to_owned(), Tone::Danger)
            } else {
                let code = e.exit_code.map_or("—".to_owned(), |c| c.to_string());
                (format!("exit {code}"), if e.ok { Tone::Ok } else { Tone::Danger })
            };
            widgets::chip(ui, &label, tone);
            widgets::text(ui, format!("{} ms", e.duration_ms), style::body(11.5), t.text_3);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let mut j = widgets::job(&cmd, style::mono(12.0), t.text);
                j.wrap.max_rows = 1;
                j.wrap.break_anywhere = true;
                j.wrap.max_width = ui.available_width();
                ui.add(egui::Label::new(j).selectable(false).truncate());
            });
        });
        let clicked = resp.clicked();
        if open {
            let line = ui.available_rect_before_wrap();
            ui.painter().hline(line.x_range(), line.top(), Stroke::new(1.0, mix(t.border, t.surface, 0.4)));
            Frame::new().fill(t.bg.gamma_multiply(0.4)).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 8.0;
                if let Some(err) = &e.error {
                    widgets::selectable(ui, err, style::mono(11.5), t.danger);
                }
                let out = e.stdout.as_deref().map(str::trim).unwrap_or("");
                let err = e.stderr.as_deref().map(str::trim).unwrap_or("");
                for (name, text, color) in [("stdout", out, t.text_2), ("stderr", err, t.warn)] {
                    if text.is_empty() {
                        continue;
                    }
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let mut j = widgets::job(&name.to_uppercase(), style::body(10.5), t.text_3);
                        j.sections[0].format.extra_letter_spacing = 0.3;
                        ui.add(egui::Label::new(j).selectable(false));
                        egui::ScrollArea::both().id_salt((e.id, name)).max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
                            widgets::selectable(ui, text, style::mono(11.5), color);
                        });
                    });
                }
                if out.is_empty() && err.is_empty() && e.error.is_none() {
                    widgets::text(ui, "(no output)", style::mono(11.5), t.text_3);
                }
            });
        }
        if !last {
            let y = ui.cursor().top();
            ui.painter().hline(r.x_range(), y - 0.5, Stroke::new(1.0, t.border));
        }
        clicked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_newest_300() {
        let mut log = Vec::new();
        for id in 0..305 {
            push_capped(&mut log, CommandLogEntry { id, ..Default::default() });
        }
        assert_eq!(log.len(), LOG_CAP);
        assert_eq!(log.first().map(|e| e.id), Some(5));
        assert_eq!(log.last().map(|e| e.id), Some(304));
    }
}
