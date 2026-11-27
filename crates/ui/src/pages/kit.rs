//! Controls in the 0.1.23 look for the Accounts, Auto and Mappings pages, drawn with the
//! painter and [`Tokens`] so they look the same whatever `egui::Style` the shell applies.
//! OWNER: ui-accounts. PRIVATE STAND-IN for ui-shell's shared `widgets`, written in
//! parallel; the main session swaps these for those when unifying.

use egui::{
    Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, WidgetInfo,
    accesskit::Role, pos2, vec2,
};

use crate::theme::{RADIUS_LG, RADIUS_MD, RADIUS_SM, TRANSITION_SECS, Tokens};
use cswap_core::view::ChipTone;

pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// Text cut to `width` with an ellipsis, one line.
pub fn galley(ui: &Ui, text: impl Into<String>, font: FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.into(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(1.0));
    ui.painter().layout_job(job)
}

/// Wrapped text, `width` wide.
pub fn wrapped(ui: &Ui, text: impl Into<String>, font: FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    ui.painter().layout(text.into(), font, color, width.max(1.0))
}

/// A label that allocates its own size (no interaction), truncated to the available width.
pub fn text(ui: &mut Ui, s: impl Into<String>, size: f32, color: Color32) -> Response {
    let g = galley(ui, s, font(size), color, ui.available_width());
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(rect.min, g, color);
    resp
}

/// Wrapped paragraph across the available width.
pub fn para(ui: &mut Ui, s: impl Into<String>, size: f32, color: Color32) -> Response {
    let g = wrapped(ui, s, font(size), color, ui.available_width());
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(rect.min, g, color);
    resp
}

pub fn mono_text(ui: &mut Ui, s: impl Into<String>, size: f32, color: Color32) -> Response {
    let g = galley(ui, s, mono(size), color, ui.available_width());
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(rect.min, g, color);
    resp
}

pub fn tone_colors(t: &Tokens, tone: ChipTone) -> (Color32, Color32) {
    match tone {
        ChipTone::Ok => (t.ok_soft, t.ok),
        ChipTone::Warn => (t.warn_soft, t.warn),
        ChipTone::Danger => (t.danger_soft, t.danger),
        ChipTone::Muted => (t.surface_2, t.text_2),
        ChipTone::Accent => (t.accent_soft, t.accent),
    }
}

/// Width a chip with `label` takes.
pub fn chip_width(ui: &Ui, label: &str) -> f32 {
    ui.painter().layout_no_wrap(label.to_owned(), font(11.0), Color32::WHITE).size().x + 12.0
}

/// `Chip`: 18 px pill, 11 px text.
pub fn chip(ui: &mut Ui, t: &Tokens, label: &str, tone: ChipTone) -> Response {
    let (bg, fg) = tone_colors(t, tone);
    let max = ui.available_width().max(24.0);
    let g = galley(ui, label, font(11.0), fg, max - 12.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 12.0, 18.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(4), bg);
    ui.painter().galley(pos2(rect.min.x + 6.0, rect.center().y - g.size().y / 2.0), g, fg);
    resp
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Primary,
    Default,
    Ghost,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Bars,
    Ring,
    Gauge,
    GaugeOff,
    Eye,
    EyeOff,
    Ellipsis,
    Chevron,
    Plus,
    Shuffle,
    Terminal,
    Folder,
    Trash,
    Play,
    Pause,
    Zap,
    Close,
}

/// Paints a 15 px line icon centred on `c`.
pub fn paint_icon(p: &egui::Painter, c: Pos2, icon: Icon, color: Color32) {
    let s = Stroke::new(1.4, color);
    let l = |a: (f32, f32), b: (f32, f32)| p.line_segment([pos2(c.x + a.0, c.y + a.1), pos2(c.x + b.0, c.y + b.1)], s);
    match icon {
        Icon::Bars => {
            l((-5.5, -4.0), (5.5, -4.0));
            l((-5.5, 0.0), (3.0, 0.0));
            l((-5.5, 4.0), (5.5, 4.0));
        }
        Icon::Ring => {
            // Dashed circle: eight short arcs.
            for i in 0..8 {
                let a0 = i as f32 * std::f32::consts::TAU / 8.0;
                let pts: Vec<Pos2> = (0..=3).map(|k| a0 + k as f32 * 0.16).map(|a| c + vec2(a.cos(), a.sin()) * 5.5).collect();
                p.add(egui::Shape::line(pts, s));
            }
        }
        Icon::Gauge | Icon::GaugeOff => {
            let pts: Vec<Pos2> =
                (0..=16).map(|k| std::f32::consts::PI * (1.0 + k as f32 / 16.0)).map(|a| c + vec2(a.cos(), a.sin()) * 6.0 + vec2(0.0, 2.0)).collect();
            p.add(egui::Shape::line(pts, s));
            l((0.0, 2.0), (3.0, -2.5));
            if icon == Icon::GaugeOff {
                l((-6.0, -5.0), (6.0, 5.0));
            }
        }
        Icon::Eye | Icon::EyeOff => {
            let top: Vec<Pos2> = (0..=12).map(|k| -6.5 + k as f32 * 13.0 / 12.0).map(|x| pos2(c.x + x, c.y - 4.0 * (1.0 - (x / 6.5).powi(2)))).collect();
            let bot: Vec<Pos2> = (0..=12).map(|k| -6.5 + k as f32 * 13.0 / 12.0).map(|x| pos2(c.x + x, c.y + 4.0 * (1.0 - (x / 6.5).powi(2)))).collect();
            p.add(egui::Shape::line(top, s));
            p.add(egui::Shape::line(bot, s));
            p.circle_stroke(c, 1.8, s);
            if icon == Icon::EyeOff {
                l((-6.0, -5.5), (6.0, 5.5));
            }
        }
        Icon::Ellipsis => {
            for dx in [-4.5, 0.0, 4.5] {
                p.circle_filled(pos2(c.x + dx, c.y), 1.2, color);
            }
        }
        Icon::Chevron => {
            l((-3.0, -1.5), (0.0, 1.5));
            l((0.0, 1.5), (3.0, -1.5));
        }
        Icon::Plus => {
            l((-5.0, 0.0), (5.0, 0.0));
            l((0.0, -5.0), (0.0, 5.0));
        }
        Icon::Shuffle => {
            l((-6.0, -3.5), (-2.0, -3.5));
            l((-2.0, -3.5), (2.0, 3.5));
            l((2.0, 3.5), (6.0, 3.5));
            l((-6.0, 3.5), (-2.0, 3.5));
            l((-2.0, 3.5), (2.0, -3.5));
            l((2.0, -3.5), (6.0, -3.5));
            l((4.0, -5.5), (6.0, -3.5));
            l((4.0, 5.5), (6.0, 3.5));
        }
        Icon::Terminal => {
            p.rect_stroke(Rect::from_center_size(c, vec2(12.0, 10.0)), CornerRadius::same(2), s, StrokeKind::Middle);
            l((-3.5, -1.5), (-1.5, 0.0));
            l((-1.5, 0.0), (-3.5, 1.5));
            l((0.0, 2.0), (3.0, 2.0));
        }
        Icon::Folder => {
            let pts = [(-6.0, -4.0), (-2.0, -4.0), (-1.0, -2.5), (6.0, -2.5), (6.0, 4.5), (-6.0, 4.5), (-6.0, -4.0)];
            p.add(egui::Shape::line(pts.iter().map(|(x, y)| pos2(c.x + x, c.y + y)).collect(), s));
        }
        Icon::Trash => {
            l((-5.5, -3.5), (5.5, -3.5));
            l((-2.0, -5.5), (2.0, -5.5));
            let pts = [(-4.0, -3.5), (-3.3, 5.5), (3.3, 5.5), (4.0, -3.5)];
            p.add(egui::Shape::line(pts.iter().map(|(x, y)| pos2(c.x + x, c.y + y)).collect(), s));
        }
        Icon::Play => {
            let pts = vec![pos2(c.x - 3.0, c.y - 5.0), pos2(c.x + 5.0, c.y), pos2(c.x - 3.0, c.y + 5.0)];
            p.add(egui::Shape::convex_polygon(pts, Color32::TRANSPARENT, s));
        }
        Icon::Pause => {
            p.rect_stroke(Rect::from_center_size(pos2(c.x - 2.5, c.y), vec2(2.5, 10.0)), CornerRadius::same(1), s, StrokeKind::Middle);
            p.rect_stroke(Rect::from_center_size(pos2(c.x + 2.5, c.y), vec2(2.5, 10.0)), CornerRadius::same(1), s, StrokeKind::Middle);
        }
        Icon::Zap => {
            let pts = [(1.0, -6.0), (-4.5, 1.0), (0.0, 1.0), (-1.0, 6.0), (4.5, -1.0), (0.0, -1.0), (1.0, -6.0)];
            p.add(egui::Shape::line(pts.iter().map(|(x, y)| pos2(c.x + x, c.y + y)).collect(), s));
        }
        Icon::Close => {
            l((-4.0, -4.0), (4.0, 4.0));
            l((-4.0, 4.0), (4.0, -4.0));
        }
    }
}

/// Lays `f` out in a child at the builder's rect WITHOUT taking space in `ui` (unlike
/// `scope_builder`): the callers position cells absolutely and allocate the whole row
/// themselves, and allocating each cell too made every row twice as tall.
pub fn place<R>(ui: &mut Ui, b: egui::UiBuilder, f: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let mut child = ui.new_child(b);
    let inner = f(&mut child);
    let mut response = child.response();
    response.rect = child.min_rect();
    egui::InnerResponse::new(inner, response)
}

/// Hover fade, 150 ms (Purwa: 300 ms reads as slow). Never changes layout.
fn hover_t(ui: &Ui, resp: &Response) -> f32 {
    ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered() && resp.enabled(), TRANSITION_SECS)
}

pub struct Btn<'a> {
    pub label: &'a str,
    pub variant: Variant,
    pub small: bool,
    pub icon: Option<Icon>,
    pub trailing: Option<Icon>,
    pub enabled: bool,
    pub loading: bool,
    /// Accessible name when `label` is empty (icon-only buttons).
    pub a11y: Option<&'a str>,
    pub min_width: f32,
}

impl<'a> Btn<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            variant: Variant::Default,
            small: false,
            icon: None,
            trailing: None,
            enabled: true,
            loading: false,
            a11y: None,
            min_width: 0.0,
        }
    }
    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
    pub fn icon(mut self, i: Icon) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn trailing(mut self, i: Icon) -> Self {
        self.trailing = Some(i);
        self
    }
    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }
    pub fn loading(mut self, l: bool) -> Self {
        self.loading = l;
        self
    }
    pub fn a11y(mut self, s: &'a str) -> Self {
        self.a11y = Some(s);
        self
    }

    /// Width the button will take.
    pub fn width(&self, ui: &Ui) -> f32 {
        let (h, pad, size) = if self.small { (28.0, 10.0, 12.0) } else { (32.0, 12.0, 13.0) };
        if self.label.is_empty() {
            return h;
        }
        let tw = ui.painter().layout_no_wrap(self.label.to_owned(), font(size), Color32::WHITE).size().x;
        let gap = if self.small { 6.0 } else { 8.0 };
        let icons = (self.icon.is_some() || self.loading) as u8 as f32 * (14.0 + gap) + self.trailing.is_some() as u8 as f32 * (12.0 + gap);
        (pad * 2.0 + tw + icons).max(self.min_width)
    }

    pub fn show(self, ui: &mut Ui, t: &Tokens) -> Response {
        let (h, pad, size) = if self.small { (28.0, 10.0, 12.0) } else { (32.0, 12.0, 13.0) };
        let w = self.width(ui);
        let enabled = self.enabled && !self.loading;
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), if enabled { Sense::click() } else { Sense::hover() });
        let name = self.a11y.unwrap_or(self.label).to_owned();
        resp.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, &name));
        let k = hover_t(ui, &resp);
        let (fill, fill_h, stroke, stroke_h, fg, fg_h) = match self.variant {
            Variant::Primary => (t.accent, t.accent_hover, Color32::TRANSPARENT, Color32::TRANSPARENT, t.accent_fg, t.accent_fg),
            Variant::Default => (t.surface, t.surface_2, t.border, t.border_strong, t.text, t.text),
            Variant::Ghost => (Color32::TRANSPARENT, t.surface_2, Color32::TRANSPARENT, Color32::TRANSPARENT, t.text_2, t.text),
            Variant::Danger => (t.danger, t.danger.gamma_multiply(1.1), Color32::TRANSPARENT, Color32::TRANSPARENT, Color32::WHITE, Color32::WHITE),
        };
        let mix = |a: Color32, b: Color32| a.lerp_to_gamma(b, k);
        let (mut fill, mut stroke, mut fg) = (mix(fill, fill_h), mix(stroke, stroke_h), mix(fg, fg_h));

        if !enabled && !self.loading {
            fill = fill.gamma_multiply(0.5);
            stroke = stroke.gamma_multiply(0.5);
            fg = fg.gamma_multiply(0.5);
        }
        let p = ui.painter();
        p.rect(rect, CornerRadius::same(RADIUS_MD), fill, Stroke::new(1.0, stroke), StrokeKind::Inside);
        if self.label.is_empty() {
            if let Some(i) = self.icon {
                paint_icon(p, rect.center(), i, fg);
            }
            return resp;
        }
        let gap = if self.small { 6.0 } else { 8.0 };
        let mut x = rect.min.x + pad;
        if self.loading {
            let a = ui.input(|i| i.time) as f32 * 6.0;
            let c = pos2(x + 7.0, rect.center().y);
            let pts: Vec<Pos2> = (0..=12).map(|k| a + k as f32 * 0.4).map(|a| c + vec2(a.cos(), a.sin()) * 5.0).collect();
            p.add(egui::Shape::line(pts, Stroke::new(1.5, fg)));
            ui.ctx().request_repaint();
            x += 14.0 + gap;
        } else if let Some(i) = self.icon {
            paint_icon(p, pos2(x + 7.0, rect.center().y), i, fg);
            x += 14.0 + gap;
        }
        let g = p.layout_no_wrap(self.label.to_owned(), font(size), fg);
        let gw = g.size().x;
        p.galley(pos2(x, rect.center().y - g.size().y / 2.0), g, fg);
        if let Some(i) = self.trailing {
            paint_icon(p, pos2(x + gw + gap + 5.0, rect.center().y), i, fg.gamma_multiply(0.7));
        }
        resp
    }
}

/// `Switch`: 30×18 toggle.
pub fn switch(ui: &mut Ui, t: &Tokens, on: &mut bool, label: &str, enabled: bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(30.0, 18.0), if enabled { Sense::click() } else { Sense::hover() });
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let v = *on;
    resp.widget_info(|| WidgetInfo::selected(Role::Switch, enabled, v, label));
    let k = ui.ctx().animate_bool_with_time(resp.id, *on, TRANSITION_SECS);
    let (bg, border) = if *on { (t.accent, t.accent) } else { (t.surface_3, t.border_strong) };
    let a = if enabled { 1.0 } else { 0.5 };
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(9), bg.gamma_multiply(a), Stroke::new(1.0, border.gamma_multiply(a)), StrokeKind::Inside);
    let x = egui::lerp(rect.min.x + 9.0..=rect.max.x - 9.0, k);
    p.circle_filled(pos2(x, rect.center().y), 6.0, Color32::WHITE.gamma_multiply(a));
    resp
}

#[allow(clippy::too_many_arguments)]
/// A single-line input in the 0.1.23 frame (32 px, 7 px radius, accent on focus).
pub fn input(ui: &mut Ui, t: &Tokens, value: &mut String, id: &str, hint: &str, width: f32, mono_font: bool, password: bool) -> Response {
    let f = if mono_font { mono(12.0) } else { font(13.0) };
    let frame = egui::Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(RADIUS_MD))
        .inner_margin(egui::Margin::symmetric(9, 7));
    let edit = egui::TextEdit::singleline(value)
        .id(Id::unique(("kit-input", id)))
        .hint_text(egui::RichText::new(hint).color(t.text_3))
        .font(f)
        .text_color(t.text)
        .password(password)
        .frame(frame)
        .desired_width(width - 18.0);
    let resp = edit.show(ui).response.response;
    if resp.has_focus() {
        ui.painter().rect_stroke(resp.rect, CornerRadius::same(RADIUS_MD), Stroke::new(1.0, t.accent), StrokeKind::Inside);
    }
    resp
}

/// A field label over a control, with a hint below.
pub fn field<R>(ui: &mut Ui, t: &Tokens, label: &str, hint: Option<&str>, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        text(ui, label, 12.0, t.text_2);
        let r = add(ui);
        if let Some(h) = hint {
            text(ui, h, 11.5, t.text_3);
        }
        r
    })
    .inner
}

/// Menu entries; `None` is a separator.
pub struct MenuItem<'a> {
    pub label: &'a str,
    pub hint: Option<&'a str>,
    pub danger: bool,
}

pub fn item(label: &str) -> Option<MenuItem<'_>> {
    Some(MenuItem { label, hint: None, danger: false })
}

pub fn item_hint<'a>(label: &'a str, hint: &'a str) -> Option<MenuItem<'a>> {
    Some(MenuItem { label, hint: Some(hint), danger: false })
}

/// A dropdown under `anchor`; returns the index of the item picked this frame.
pub fn menu(ui: &Ui, t: &Tokens, anchor: &Response, width: f32, items: &[Option<MenuItem<'_>>]) -> Option<usize> {
    let mut picked = None;
    let frame = egui::Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(RADIUS_LG - 2))
        .inner_margin(egui::Margin::same(4))
        .shadow(egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(if t.dark { 90 } else { 30 }) });
    let _ = ui;
    egui::Popup::menu(anchor).width(width).frame(frame).gap(4.0).show(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, it) in items.iter().enumerate() {
            let Some(it) = it else {
                let (r, _) = ui.allocate_exact_size(vec2(width - 8.0, 9.0), Sense::hover());
                ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, t.border));
                continue;
            };
            let (r, resp) = ui.allocate_exact_size(vec2(width - 8.0, 30.0), Sense::click());
            resp.widget_info(|| WidgetInfo::labeled(Role::MenuItem, true, it.label));
            let k = hover_t(ui, &resp);
            let (fg, hover_bg) = if it.danger { (t.danger, t.danger_soft) } else { (t.text, t.surface_2) };
            ui.painter().rect_filled(r, CornerRadius::same(RADIUS_MD - 1), Color32::TRANSPARENT.lerp_to_gamma(hover_bg, k));
            let mut right = r.max.x - 8.0;
            if let Some(h) = it.hint {
                let g = ui.painter().layout_no_wrap(h.to_owned(), font(11.0), t.text_3);
                right -= g.size().x;
                ui.painter().galley(pos2(right, r.center().y - g.size().y / 2.0), g, t.text_3);
                right -= 8.0;
            }
            let g = galley(ui, it.label, font(13.0), fg, right - r.min.x - 8.0);
            ui.painter().galley(pos2(r.min.x + 8.0, r.center().y - g.size().y / 2.0), g, fg);
            if resp.clicked() {
                picked = Some(i);
                ui.close();
            }
        }
    });
    picked
}

/// What a dialog asks of its caller this frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DialogOut {
    pub close: bool,
}

/// `Dialog`: modal card with a title bar, a body and a footer laid right-to-left (put the
/// primary button first).
pub fn dialog(
    ctx: &egui::Context,
    t: &Tokens,
    id: &str,
    title: &str,
    width: f32,
    body: impl FnOnce(&mut Ui),
    footer: impl FnOnce(&mut Ui),
) -> DialogOut {
    let frame = egui::Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(12))
        .shadow(egui::Shadow { offset: [0, 16], blur: 40, spread: 0, color: Color32::from_black_alpha(if t.dark { 120 } else { 45 }) });
    let width = width.min(ctx.content_rect().width() - 32.0);
    let mut close = false;
    let area = egui::Modal::default_area(Id::unique(("kit-dialog", id)))
        .anchor(Align2::CENTER_TOP, vec2(0.0, ctx.content_rect().height() * 0.12));
    let resp = egui::Modal::new(Id::unique(("kit-dialog", id)))
        .area(area)
        .frame(frame)
        .backdrop_color(Color32::from_black_alpha(76))
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            // header
            let (hr, _) = ui.allocate_exact_size(vec2(width, 46.0), Sense::hover());
            let g = galley(ui, title, font(14.0), t.text, width - 60.0);
            ui.painter().galley(pos2(hr.min.x + 16.0, hr.center().y - g.size().y / 2.0), g, t.text);
            ui.painter().hline(hr.x_range(), hr.max.y, Stroke::new(1.0, t.border));
            let close_rect = Rect::from_center_size(pos2(hr.max.x - 26.0, hr.center().y), vec2(28.0, 28.0));
            let cr = ui.interact(close_rect, Id::unique(("kit-dialog-close", id)), Sense::click());
            cr.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Close"));
            let k = hover_t(ui, &cr);
            ui.painter().rect_filled(close_rect, CornerRadius::same(RADIUS_MD), Color32::TRANSPARENT.lerp_to_gamma(t.surface_2, k));
            paint_icon(ui.painter(), close_rect.center(), Icon::Close, t.text_2);
            if cr.clicked() {
                close = true;
            }
            // body
            egui::Frame::new().inner_margin(egui::Margin::same(16)).show(ui, |ui| {
                ui.set_width(width - 32.0);
                ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
                body(ui);
            });
            // footer
            let top = ui.cursor().min.y - 8.0;
            let fr = Rect::from_min_size(pos2(hr.min.x, top), vec2(width, 56.0));
            ui.painter().rect_filled(
                fr,
                CornerRadius { nw: 0, ne: 0, sw: 12, se: 12 },
                t.bg.gamma_multiply(0.6),
            );
            ui.painter().hline(fr.x_range(), fr.min.y, Stroke::new(1.0, t.border));
            let mut fu = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(fr.shrink2(vec2(16.0, 12.0)))
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            footer(&mut fu);
            ui.allocate_space(vec2(width, 56.0 - 8.0));
        });
    if resp.should_close() {
        close = true;
    }
    DialogOut { close }
}

/// Bordered surface card (`rounded-lg border bg-surface shadow-card`).
pub fn card_frame(t: &Tokens) -> egui::Frame {
    egui::Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(RADIUS_LG - 2))
        .shadow(egui::Shadow { offset: [0, 1], blur: 2, spread: 0, color: Color32::from_black_alpha(if t.dark { 40 } else { 10 }) })
}

/// Page header: title + subtitle on the left, actions on the right (wrapping below when
/// there is no room, like `flex-wrap`). `actions` lays out right-to-left.
pub fn page_header(ui: &mut Ui, t: &Tokens, title: &str, subtitle: impl FnOnce(&mut Ui), actions_width: f32, actions: impl FnOnce(&mut Ui)) {
    let full = ui.available_width();
    let side_by_side = full - 48.0 - actions_width >= 220.0;
    egui::Frame::new().inner_margin(egui::Margin { left: 24, right: 24, top: 20, bottom: 12 }).show(ui, |ui| {
        let w = ui.available_width();
        let left_w = if side_by_side { w - actions_width - 12.0 } else { w };
        let top = ui.cursor().min;
        let left = place(ui, egui::UiBuilder::new().max_rect(Rect::from_min_size(top, vec2(left_w, 400.0))), |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.set_width(left_w);
                text(ui, title, 16.0, t.text);
                subtitle(ui);
            })
            .response
            .rect;
        let act_rect = if side_by_side {
            Rect::from_min_size(pos2(top.x + w - actions_width, top.y), vec2(actions_width, 32.0))
        } else {
            Rect::from_min_size(pos2(top.x, left.max.y + 12.0), vec2(actions_width.min(w), 32.0))
        };
        let mut a = ui.new_child(egui::UiBuilder::new().max_rect(act_rect).layout(egui::Layout::right_to_left(egui::Align::Center)));
        a.spacing_mut().item_spacing.x = 8.0;
        actions(&mut a);
        let bottom = left.max.y.max(act_rect.max.y);
        ui.allocate_space(vec2(w, bottom - top.y));
    });
}

/// `Empty`: dashed box with a title, a body and an optional action.
pub fn empty(ui: &mut Ui, t: &Tokens, title: &str, body: &str, action: impl FnOnce(&mut Ui)) {
    let w = ui.available_width();
    let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 56)).show(ui, |ui| {
        ui.set_width(w - 48.0);
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            text(ui, title, 14.0, t.text);
            let g = wrapped(ui, body, font(12.5), t.text_2, (w - 48.0).min(420.0));
            let (rr, _) = ui.allocate_exact_size(g.size(), Sense::hover());
            ui.painter().galley(rr.min, g, t.text_2);
            ui.add_space(10.0);
            action(ui);
        });
    });
    dashed_rect(ui.painter(), r.response.rect, t.border);
}

fn dashed_rect(p: &egui::Painter, r: Rect, color: Color32) {
    let s = Stroke::new(1.0, color);
    let r = r.shrink(0.5);
    for (a, b) in [(r.left_top(), r.right_top()), (r.right_top(), r.right_bottom()), (r.right_bottom(), r.left_bottom()), (r.left_bottom(), r.left_top())] {
        p.extend(egui::Shape::dashed_line(&[a, b], s, 4.0, 3.0));
    }
}

/// Coloured notice strip (errors, stale data).
pub fn notice(ui: &mut Ui, t: &Tokens, msg: &str, tone: ChipTone) {
    let (bg, fg) = tone_colors(t, tone);
    let w = ui.available_width();
    egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, fg.gamma_multiply(0.3)))
        .corner_radius(CornerRadius::same(RADIUS_LG - 2))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(w - 24.0);
            para(ui, msg, 12.5, fg);
        });
}

/// Inline code span look (`mono rounded bg-surface-2`).
pub fn code(ui: &mut Ui, t: &Tokens, s: &str) -> Response {
    let g = ui.painter().layout_no_wrap(s.to_owned(), mono(12.0), t.text);
    let (r, resp) = ui.allocate_exact_size(g.size() + vec2(8.0, 2.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same(RADIUS_SM - 1), t.surface_2);
    ui.painter().galley(r.min + vec2(4.0, 1.0), g, t.text);
    resp
}

/// Size of a chunk of text, for laying out before painting.
pub fn measure(ui: &Ui, s: &str, f: FontId) -> Vec2 {
    ui.painter().layout_no_wrap(s.to_owned(), f, Color32::WHITE).size()
}
