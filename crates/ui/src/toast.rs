//! Transient messages (bottom of the window, ~4 s, errors stay until dismissed).
//! The queue is the contract; drawing it belongs to ui-shell.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub text: String,
    /// `ctx.input(|i| i.time)` when it was pushed; 0.0 until the shell stamps it.
    pub at: f64,
}

#[derive(Debug, Default)]
pub struct Toasts {
    pub items: Vec<Toast>,
}

impl Toasts {
    pub fn push(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.items.push(Toast { kind, text: text.into(), at: 0.0 });
    }
    pub fn info(&mut self, text: impl Into<String>) {
        self.push(ToastKind::Info, text)
    }
    pub fn success(&mut self, text: impl Into<String>) {
        self.push(ToastKind::Success, text)
    }
    pub fn error(&mut self, text: impl Into<String>) {
        self.push(ToastKind::Error, text)
    }
}

// ---- drawing (ui-shell) -------------------------------------------------------------------
//
// A toast's first line is its title and anything after the first `\n` its detail (the
// `notify(kind, title, detail)` of 0.1.23), so pages can say `toasts.error(format!("Switch
// failed\n{msg}"))` without a wider contract.

/// How long a non-error toast stays, seconds (0.1.23: 4500 ms). Errors stay until dismissed,
/// because an error that vanishes while the user looks elsewhere was never reported.
pub const TOAST_SECS: f64 = 4.5;

impl Toasts {
    /// Stamps new toasts with `now` and drops the expired ones. Returns how long until the
    /// next one expires, for a repaint.
    pub fn tick(&mut self, now: f64) -> Option<f64> {
        for t in &mut self.items {
            if t.at == 0.0 {
                t.at = now;
            }
        }
        self.items.retain(|t| t.kind == ToastKind::Error || now - t.at < TOAST_SECS);
        self.items
            .iter()
            .filter(|t| t.kind != ToastKind::Error)
            .map(|t| TOAST_SECS - (now - t.at))
            .fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.min(x))))
    }

    /// Bottom-right stack, newest at the bottom, 360 px wide.
    pub fn show(&mut self, ctx: &egui::Context) {
        use egui::{Align2, CornerRadius, Frame, Margin, Stroke, vec2};

        use crate::style;
        use crate::widgets::{self, icon};

        let now = ctx.input(|i| i.time);
        if let Some(wait) = self.tick(now) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait.max(0.0)));
        }
        if self.items.is_empty() {
            return;
        }
        let t = style::tokens(ctx);
        let screen = ctx.content_rect();
        let width = 360f32.min(screen.width() - 24.0);
        let mut dismiss = None;
        egui::Area::new(egui::Id::new("toasts"))
            .order(egui::Order::Foreground)
            .anchor(Align2::RIGHT_BOTTOM, vec2(-12.0, -12.0))
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 8.0;
                for (i, toast) in self.items.iter().enumerate() {
                    let (title, detail) = match toast.text.split_once('\n') {
                        Some((a, b)) => (a, Some(b.trim())),
                        None => (toast.text.as_str(), None),
                    };
                    let (glyph, color) = match toast.kind {
                        ToastKind::Success => (icon::CIRCLE_CHECK, t.ok),
                        ToastKind::Error => (icon::CIRCLE_ALERT, t.danger),
                        ToastKind::Info => (icon::INFO, t.accent),
                    };
                    // `anim-pop`: fade + a hint of scale over 150 ms.
                    let age = (now - toast.at).max(0.0) as f32;
                    let a = (age / crate::theme::TRANSITION_SECS).clamp(0.0, 1.0);
                    if a < 1.0 {
                        ctx.request_repaint();
                    }
                    ui.scope(|ui| {
                        ui.set_opacity(a);
                        let r = Frame::new()
                            .fill(t.surface)
                            .stroke(Stroke::new(1.0, t.border))
                            .corner_radius(CornerRadius::same(crate::theme::RADIUS_LG))
                            .shadow(style::pop_shadow(&t))
                            .inner_margin(Margin { left: 12, right: 8, top: 10, bottom: 10 })
                            .show(ui, |ui| {
                                ui.set_width(width - 20.0);
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = 10.0;
                                    ui.add_space(0.0);
                                    widgets::icon(ui, glyph, 16.0, color);
                                    let text_w = ui.available_width() - 30.0;
                                    ui.vertical(|ui| {
                                        ui.set_width(text_w);
                                        ui.spacing_mut().item_spacing.y = 2.0;
                                        widgets::para(ui, title, style::body_medium(13.0), t.text);
                                        if let Some(d) = detail.filter(|d| !d.is_empty()) {
                                            widgets::selectable(ui, d, style::body(12.0), t.text_2);
                                        }
                                    });
                                    if widgets::Button::icon_only(icon::X, "Dismiss").size(widgets::Size::Sm).show(ui).clicked() {
                                        dismiss = Some(i);
                                    }
                                });
                            });
                        ui.ctx().accesskit_node_builder(r.response.id, |n| {
                            n.set_role(egui::accesskit::Role::Status);
                            n.set_label(toast.text.clone());
                        });
                    });
                }
            });
        if let Some(i) = dismiss {
            self.items.remove(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Errors must outlive the timeout: one that vanishes while the user looks elsewhere was
    // never reported at all.
    #[test]
    fn info_expires_error_stays() {
        let mut t = Toasts::default();
        t.info("saved");
        t.error("Switch failed\nno such account");
        assert_eq!(t.tick(10.0), Some(TOAST_SECS));
        t.success("later");
        assert!(t.tick(12.0).is_some());
        t.tick(10.0 + TOAST_SECS + 0.01);
        let kinds: Vec<_> = t.items.iter().map(|x| x.kind).collect();
        assert_eq!(kinds, vec![ToastKind::Error, ToastKind::Success]);
        t.tick(100.0);
        assert_eq!(t.items.len(), 1);
        assert_eq!(t.tick(1000.0), None);
    }
}
