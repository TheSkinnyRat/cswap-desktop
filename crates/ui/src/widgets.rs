//! Shared controls in the 0.1.23 look. Port of `components/{ui,Tooltip,PageHeader}.tsx`.
//! OWNER: ui-shell.
//!
//! Every interactive control reports an accesskit role and a name that IS its visible label
//! (or its `aria-label` in 0.1.23), so tests and screen readers find it by name. Hover only
//! blends colour over [`theme::TRANSITION_SECS`]; nothing moves or resizes on hover.

use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Frame, Id, Layout, Margin, Pos2, Rect, Response, RichText, Sense,
    Shape, Stroke, StrokeKind, Ui, Vec2, WidgetInfo, WidgetText, accesskit::Role, pos2, text::LayoutJob, vec2,
};

use crate::style::{self, mix};
use crate::theme::{self, RADIUS_LG, RADIUS_MD, RADIUS_SM, Tokens};

/// Lucide glyphs in the embedded icon font (codepoints of lucide-static 1.82). Draw them
/// with [`icon`] / [`style::icon`]; only these exist in the subset.
pub mod icon {
    pub const ACTIVITY: &str = "\u{E038}";
    pub const TEXT_ALIGN_START: &str = "\u{E185}";
    pub const ARROW_RIGHT_LEFT: &str = "\u{E417}";
    pub const CIRCLE_ARROW_UP: &str = "\u{E07B}";
    pub const CHECK: &str = "\u{E06C}";
    pub const CIRCLE_CHECK: &str = "\u{E226}";
    pub const CHEVRON_DOWN: &str = "\u{E06D}";
    pub const CHEVRON_RIGHT: &str = "\u{E06F}";
    pub const CHEVRON_UP: &str = "\u{E070}";
    pub const CIRCLE_ALERT: &str = "\u{E077}";
    pub const CIRCLE_DASHED: &str = "\u{E4B0}";
    pub const CORNER_DOWN_LEFT: &str = "\u{E0A1}";
    pub const DOWNLOAD: &str = "\u{E0B2}";
    pub const ELLIPSIS: &str = "\u{E0B6}";
    pub const EXTERNAL_LINK: &str = "\u{E0B9}";
    pub const EYE: &str = "\u{E0BA}";
    pub const EYE_OFF: &str = "\u{E0BB}";
    pub const FOLDER_OPEN: &str = "\u{E247}";
    pub const FOLDER_TREE: &str = "\u{E33C}";
    pub const GAUGE: &str = "\u{E1BF}";
    pub const CIRCLE_GAUGE: &str = "\u{E4E1}";
    pub const INFO: &str = "\u{E0F9}";
    pub const KEY_ROUND: &str = "\u{E4A3}";
    pub const LOADER_CIRCLE: &str = "\u{E10A}";
    pub const MONITOR: &str = "\u{E11D}";
    pub const MOON: &str = "\u{E11E}";
    pub const MOVE_VERTICAL: &str = "\u{E1C7}";
    pub const PANEL_LEFT_CLOSE: &str = "\u{E21C}";
    pub const PANEL_LEFT_OPEN: &str = "\u{E21D}";
    pub const PAUSE: &str = "\u{E12E}";
    pub const PENCIL: &str = "\u{E1F9}";
    pub const PLAY: &str = "\u{E13C}";
    pub const PLUS: &str = "\u{E13D}";
    pub const REFRESH_CW: &str = "\u{E145}";
    pub const SCROLL_TEXT: &str = "\u{E45F}";
    pub const SEARCH: &str = "\u{E151}";
    pub const SETTINGS: &str = "\u{E154}";
    pub const SHUFFLE: &str = "\u{E15E}";
    pub const SPARKLES: &str = "\u{E412}";
    pub const STETHOSCOPE: &str = "\u{E2F1}";
    pub const SUN: &str = "\u{E178}";
    pub const TAG: &str = "\u{E17F}";
    pub const TERMINAL: &str = "\u{E181}";
    pub const SQUARE_TERMINAL: &str = "\u{E20A}";
    pub const TRASH_2: &str = "\u{E18E}";
    pub const UPLOAD: &str = "\u{E19E}";
    pub const USER_PLUS: &str = "\u{E1A2}";
    pub const USERS: &str = "\u{E1A4}";
    pub const X: &str = "\u{E1B2}";
    pub const ZAP: &str = "\u{E1B4}";
    pub const ARROW_UP_DOWN: &str = "\u{E37D}";
    pub const ARROW_LEFT_RIGHT: &str = "\u{E24A}";
    pub const COPY: &str = "\u{E09E}";
    pub const CIRCLE_X: &str = "\u{E084}";
    pub const TRIANGLE_ALERT: &str = "\u{E193}";
}

/// The palette this frame is drawn in.
pub fn tokens(ui: &Ui) -> Tokens {
    style::tokens(ui.ctx())
}

/// 0..1 hover progress over the transition time.
pub fn hover_t(ui: &Ui, id: Id, hovered: bool) -> f32 {
    ui.ctx().animate_bool_with_time(id.with("hover"), hovered, theme::TRANSITION_SECS)
}

/// One run of text in one font.
pub fn job(text: &str, font: FontId, color: Color32) -> LayoutJob {
    LayoutJob::simple_singleline(text.to_owned(), font, color)
}

/// Paints an icon glyph centred on `center`.
pub fn paint_icon(ui: &Ui, glyph: &str, center: Pos2, size: f32, color: Color32) {
    ui.painter().text(center, Align2::CENTER_CENTER, glyph, style::icon(size), color);
}

/// An icon as a non-interactive label.
pub fn icon(ui: &mut Ui, glyph: &str, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_icon(ui, glyph, rect.center(), size, color);
    resp
}

/// Text in a given font and colour, not selectable (the window is `user-select: none`).
pub fn text(ui: &mut Ui, s: impl Into<String>, font: FontId, color: Color32) -> Response {
    ui.add(egui::Label::new(RichText::new(s).font(font).color(color)).selectable(false))
}

/// Text that wraps at the available width.
pub fn para(ui: &mut Ui, s: impl Into<String>, font: FontId, color: Color32) -> Response {
    ui.add(egui::Label::new(RichText::new(s).font(font).color(color)).selectable(false).wrap())
}

/// Text the user may copy (paths, versions, log output: `.selectable` in 0.1.23).
pub fn selectable(ui: &mut Ui, s: impl Into<String>, font: FontId, color: Color32) -> Response {
    ui.add(egui::Label::new(RichText::new(s).font(font).color(color)).selectable(true).wrap())
}

/// The `Loader2` spinner: a 3/4 arc turning once every 900 ms.
pub fn paint_spinner(ui: &Ui, center: Pos2, size: f32, color: Color32) {
    let t = ui.input(|i| i.time);
    ui.ctx().request_repaint();
    let start = (t / 0.9 * std::f64::consts::TAU) as f32;
    let r = size * 0.38;
    let n = 24;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = start + (i as f32 / n as f32) * std::f32::consts::TAU * 0.75;
            center + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    ui.painter().add(Shape::line(pts, Stroke::new(size * 0.11, color)));
}

pub fn spinner(ui: &mut Ui, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_spinner(ui, rect.center(), size, color);
    resp
}

fn mark_disabled(ui: &Ui, resp: &Response) {
    ui.ctx().accesskit_node_builder(resp.id, |n| n.set_disabled());
}

// ---- Button ------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    Primary,
    #[default]
    Default,
    Subtle,
    Ghost,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Size {
    Sm,
    #[default]
    Md,
}

impl Size {
    pub fn height(self) -> f32 {
        match self {
            Size::Sm => 28.0,
            Size::Md => 32.0,
        }
    }
    fn pad(self) -> f32 {
        match self {
            Size::Sm => 10.0,
            Size::Md => 12.0,
        }
    }
    fn font(self) -> f32 {
        match self {
            Size::Sm => 12.0,
            Size::Md => 13.0,
        }
    }
    fn gap(self) -> f32 {
        match self {
            Size::Sm => 6.0,
            Size::Md => 8.0,
        }
    }
}

/// `Button` of ui.tsx. The label is also the accessible name; an icon-only button gets its
/// name from [`Button::label`] while drawing nothing but the icon.
#[must_use]
pub struct Button<'a> {
    text: &'a str,
    name: Option<&'a str>,
    variant: Variant,
    size: Size,
    icon: Option<&'a str>,
    loading: bool,
    enabled: bool,
    square: bool,
    min_width: f32,
}

impl<'a> Button<'a> {
    pub fn new(text: &'a str) -> Self {
        Button {
            text,
            name: None,
            variant: Variant::Default,
            size: Size::Md,
            icon: None,
            loading: false,
            enabled: true,
            square: false,
            min_width: 0.0,
        }
    }
    /// Icon-only ghost button (`IconButton`): square, named by `label`.
    pub fn icon_only(glyph: &'a str, label: &'a str) -> Self {
        Button { square: true, name: Some(label), icon: Some(glyph), variant: Variant::Ghost, ..Button::new("") }
    }
    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }
    pub fn primary(self) -> Self {
        self.variant(Variant::Primary)
    }
    pub fn ghost(self) -> Self {
        self.variant(Variant::Ghost)
    }
    pub fn danger(self) -> Self {
        self.variant(Variant::Danger)
    }
    pub fn subtle(self) -> Self {
        self.variant(Variant::Subtle)
    }
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }
    pub fn small(self) -> Self {
        self.size(Size::Sm)
    }
    pub fn icon(mut self, glyph: &'a str) -> Self {
        self.icon = Some(glyph);
        self
    }
    /// Spinner instead of the icon, and not clickable (`disabled || loading`).
    pub fn loading(mut self, on: bool) -> Self {
        self.loading = on;
        self
    }
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
    pub fn label(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let t = tokens(ui);
        let h = self.size.height();
        let font = style::body_medium(self.size.font());
        let galley = (!self.text.is_empty()).then(|| ui.painter().layout_job(job(self.text, font, Color32::PLACEHOLDER)));
        let icon_sz = if self.size == Size::Sm { 13.0 } else { 14.0 };
        let has_icon = self.icon.is_some() || self.loading;
        let width = if self.square {
            h
        } else {
            let text_w = galley.as_ref().map_or(0.0, |g| g.size().x);
            let gap = if has_icon && galley.is_some() { self.size.gap() } else { 0.0 };
            let icon_w = if has_icon { icon_sz } else { 0.0 };
            (self.size.pad() * 2.0 + icon_w + gap + text_w).max(self.min_width)
        };
        let enabled = self.enabled && !self.loading;
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let (rect, resp) = ui.allocate_exact_size(vec2(width, h), sense);
        let name = self.name.unwrap_or(self.text).to_owned();
        resp.widget_info(|| WidgetInfo::labeled(egui::Role::Button, enabled, &name));
        if !enabled {
            mark_disabled(ui, &resp);
        }
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        let ht = hover_t(ui, resp.id, enabled && resp.hovered());
        let (fill, fg, border) = match self.variant {
            Variant::Primary => (mix(t.accent, t.accent_hover, ht), t.accent_fg, Color32::TRANSPARENT),
            Variant::Default => (mix(t.surface, t.surface_2, ht), t.text, mix(t.border, t.border_strong, ht)),
            Variant::Subtle => (mix(t.surface_2, t.surface_3, ht), t.text, Color32::TRANSPARENT),
            Variant::Ghost => (mix(Color32::TRANSPARENT, t.surface_2, ht), mix(t.text_2, t.text, ht), Color32::TRANSPARENT),
            Variant::Danger => {
                (mix(t.danger, mix(t.danger, Color32::WHITE, 0.12), ht), Color32::WHITE, Color32::TRANSPARENT)
            }
        };
        let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.5) };
        let painter = ui.painter();
        painter.rect(rect, CornerRadius::same(RADIUS_MD), fade(fill), Stroke::new(1.0, fade(border)), StrokeKind::Inside);
        if self.variant == Variant::Primary {
            // `shadow-[inset_0_1px_0_rgba(255,255,255,0.12)]`: the faint top highlight.
            let top = Rect::from_min_size(rect.min + vec2(3.0, 0.5), vec2(rect.width() - 6.0, 1.0));
            painter.rect_filled(top, 0.0, Color32::from_white_alpha(fade(Color32::from_white_alpha(28)).a()));
        }
        if resp.has_focus() {
            focus_ring(ui, rect, &t);
        }
        let mut x = if self.square { rect.center().x - icon_sz / 2.0 } else { rect.center().x - (width.min(rect.width()) - self.size.pad() * 2.0) / 2.0 };
        if self.min_width > 0.0 && !self.square {
            let text_w = galley.as_ref().map_or(0.0, |g| g.size().x);
            let content = if has_icon { icon_sz + if galley.is_some() { self.size.gap() } else { 0.0 } } else { 0.0 } + text_w;
            x = rect.center().x - content / 2.0;
        }
        if self.loading {
            paint_spinner(ui, pos2(x + icon_sz / 2.0, rect.center().y), icon_sz, fade(fg));
            x += icon_sz + self.size.gap();
        } else if let Some(g) = self.icon {
            paint_icon(ui, g, pos2(x + icon_sz / 2.0, rect.center().y), icon_sz, fade(fg));
            x += icon_sz + self.size.gap();
        }
        if let Some(g) = galley {
            let y = rect.center().y - g.size().y / 2.0;
            ui.painter().galley(pos2(x, y), g, fade(fg));
        }
        resp
    }
}

/// `*:focus-visible { box-shadow: 0 0 0 2px var(--bg), 0 0 0 4px var(--accent) }`.
pub fn focus_ring(ui: &Ui, rect: Rect, t: &Tokens) {
    ui.painter().rect_stroke(rect.expand(2.0), CornerRadius::same(RADIUS_MD + 2), Stroke::new(2.0, t.accent), StrokeKind::Outside);
}

// ---- Chip / Kbd --------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Warn,
    Danger,
    Muted,
    Accent,
}

impl Tone {
    pub fn colors(self, t: &Tokens) -> (Color32, Color32) {
        match self {
            Tone::Ok => (t.ok_soft, t.ok),
            Tone::Warn => (t.warn_soft, t.warn),
            Tone::Danger => (t.danger_soft, t.danger),
            Tone::Muted => (t.surface_2, t.text_2),
            Tone::Accent => (t.accent_soft, t.accent),
        }
    }
}

/// `Chip`: an 18 px pill, 11 px medium text on a soft fill.
pub fn chip(ui: &mut Ui, label: &str, tone: Tone) -> Response {
    let t = tokens(ui);
    let (bg, fg) = tone.colors(&t);
    let galley = ui.painter().layout_job(job(label, style::body_medium(11.0), fg));
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 12.0, 18.0), Sense::hover());
    let owned = label.to_owned();
    resp.widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, &owned));
    ui.painter().rect_filled(rect, CornerRadius::same(4), bg);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp
}

/// `Kbd`: a key cap.
pub fn kbd(ui: &mut Ui, label: &str) -> Response {
    let t = tokens(ui);
    let is_icon = label.chars().all(|c| ('\u{E000}'..='\u{F8FF}').contains(&c));
    let font = if is_icon { style::icon(10.0) } else { style::mono(10.5) };
    let galley = ui.painter().layout_job(job(label, font, t.text_2));
    let (rect, resp) = ui.allocate_exact_size(vec2((galley.size().x + 8.0).max(18.0), 18.0), Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(4), t.surface_2, Stroke::new(1.0, t.border), StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, t.text_2);
    resp
}

// ---- Inputs ------------------------------------------------------------------------------

/// `Input`: a 32 px framed single-line field, accent border on focus. `label` is its
/// accessible name (0.1.23's `aria-label`, or the Field label it sits under).
pub fn input(ui: &mut Ui, label: &str, value: &mut String, placeholder: &str, mono: bool, width: f32) -> Response {
    let t = tokens(ui);
    let id = ui.make_persistent_id(("input", label));
    let focused = ui.memory(|m| m.has_focus(id));
    let hovered = ui.rect_contains_pointer(Rect::from_min_size(ui.cursor().min, vec2(width, 32.0)));
    let ht = hover_t(ui, id, hovered);
    let ft = ui.ctx().animate_bool_with_time(id.with("focus"), focused, theme::TRANSITION_SECS);
    let border = mix(mix(t.border, t.border_strong, ht), t.accent, ft);
    let font = if mono { style::mono(theme::FONT_MONO) } else { style::body(theme::FONT_BODY) };
    let frame = Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(CornerRadius::same(RADIUS_MD))
        .inner_margin(Margin::symmetric(10, 7));
    let edit = egui::TextEdit::singleline(value)
        .id(id)
        .font(font.clone())
        .text_color(t.text)
        .hint_text(RichText::new(placeholder).font(font).color(t.text_3))
        .frame(frame)
        .min_size(vec2(width, 32.0))
        .desired_width(width - 20.0);
    let resp = ui.add(edit);
    let owned = label.to_owned();
    ui.ctx().accesskit_node_builder(resp.id, |n| n.set_label(owned));
    resp
}

/// `Select`: a framed button with a chevron that opens a list. Returns true when the value
/// changed. `label` is the accessible name (0.1.23's `aria-label`).
pub fn select<T: PartialEq + Clone>(ui: &mut Ui, label: &str, value: &mut T, options: &[(T, String)], width: f32) -> bool {
    let t = tokens(ui);
    let current = options.iter().find(|(v, _)| v == value).map(|(_, l)| l.clone()).unwrap_or_default();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click());
    let owned = label.to_owned();
    let cur = current.clone();
    resp.widget_info(|| {
        let mut w = WidgetInfo::labeled(egui::Role::ComboBox, true, &owned);
        w.current_text_value = Some(cur.clone());
        w
    });
    let ht = hover_t(ui, resp.id, resp.hovered());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(RADIUS_MD),
        t.surface,
        Stroke::new(1.0, mix(t.border, t.border_strong, ht)),
        StrokeKind::Inside,
    );
    let g = painter.layout_job(job(&current, style::body(theme::FONT_BODY), t.text));
    painter.galley(pos2(rect.left() + 10.0, rect.center().y - g.size().y / 2.0), g, t.text);
    paint_icon(ui, icon::CHEVRON_DOWN, pos2(rect.right() - 15.0, rect.center().y), 14.0, t.text_3);
    let mut changed = false;
    egui::Popup::menu(&resp).width(width).frame(popup_frame(&t)).show(|ui| {
        ui.set_min_width(width - 8.0);
        for (v, l) in options {
            let on = v == value;
            if menu_item(ui, l, None, on, false, true, None).clicked() && !on {
                *value = v.clone();
                changed = true;
            }
        }
    });
    changed
}

/// `Switch`: 30 x 18, accent when on, the knob sliding over the transition time. Toggles on
/// click; returns the response so callers can act on `clicked()`.
pub fn switch(ui: &mut Ui, label: &str, on: bool, enabled: bool) -> Response {
    let t = tokens(ui);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 18.0), sense);
    let owned = label.to_owned();
    resp.widget_info(|| WidgetInfo::selected(egui::Role::CheckBox, enabled, on, &owned));
    if !enabled {
        mark_disabled(ui, &resp);
    }
    let k = ui.ctx().animate_bool_with_time(resp.id.with("knob"), on, theme::TRANSITION_SECS);
    let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.5) };
    let fill = mix(t.surface_3, t.accent, k);
    let border = mix(t.border_strong, t.accent, k);
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(9), fade(fill), Stroke::new(1.0, fade(border)), StrokeKind::Inside);
    let x = rect.left() + 2.0 + 6.0 + 12.0 * k;
    painter.circle_filled(pos2(x + 1.0, rect.center().y), 6.0, fade(Color32::WHITE));
    resp
}

/// `Segmented`: a few options in a sunken track; the picked one is a raised chip. Each option
/// is a radio button named by its text. Returns the clicked option.
pub fn segmented<T: Copy + PartialEq>(ui: &mut Ui, value: T, options: &[(T, Option<&str>, &str)]) -> Option<T> {
    let t = tokens(ui);
    let font = style::body_medium(12.0);
    let widths: Vec<f32> = options
        .iter()
        .map(|(_, ic, l)| {
            let g = ui.painter().layout_job(job(l, font.clone(), Color32::PLACEHOLDER));
            g.size().x + 20.0 + if ic.is_some() { 19.0 } else { 0.0 }
        })
        .collect();
    let total = widths.iter().sum::<f32>() + 4.0;
    let (outer, _) = ui.allocate_exact_size(vec2(total, 32.0), Sense::hover());
    ui.painter().rect(outer, CornerRadius::same(RADIUS_MD), t.surface_2, Stroke::new(1.0, t.border), StrokeKind::Inside);
    let mut x = outer.left() + 2.0;
    let mut picked = None;
    for ((v, ic, l), w) in options.iter().zip(widths) {
        let r = Rect::from_min_size(pos2(x, outer.top() + 2.0), vec2(w, outer.height() - 4.0));
        x += w;
        let id = ui.scope_id().with(("seg", *l));
        let resp = ui.interact(r, id, Sense::click());
        let on = *v == value;
        let owned = (*l).to_owned();
        resp.widget_info(|| WidgetInfo::selected(egui::Role::RadioButton, true, on, &owned));
        let ht = hover_t(ui, id, resp.hovered() && !on);
        let painter = ui.painter();
        if on {
            painter.add(style::card_shadow(&t).as_shape(r, CornerRadius::same(RADIUS_SM)));
            painter.rect_filled(r, CornerRadius::same(RADIUS_SM), t.surface);
        }
        let fg = if on { t.text } else { mix(t.text_2, t.text, ht) };
        let g = painter.layout_job(job(l, font.clone(), fg));
        let content = g.size().x + if ic.is_some() { 19.0 } else { 0.0 };
        let mut cx = r.center().x - content / 2.0;
        if let Some(ic) = ic {
            paint_icon(ui, ic, pos2(cx + 6.5, r.center().y), 13.0, fg);
            cx += 19.0;
        }
        ui.painter().galley(pos2(cx, r.center().y - g.size().y / 2.0), g, fg);
        if resp.clicked() && !on {
            picked = Some(*v);
        }
    }
    picked
}

/// `Field`: label (13 px medium) and an optional hint (12 px, `--text-3`) beside or above
/// the control. `inline` puts the control on the right, as every Settings row does.
pub fn field<R>(ui: &mut Ui, label: &str, hint: Option<&str>, inline: bool, control: impl FnOnce(&mut Ui) -> R) -> R {
    let t = tokens(ui);
    let labels = |ui: &mut Ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        text(ui, label, style::body_medium(13.0), t.text);
        if let Some(h) = hint {
            para(ui, h, style::body(12.0), t.text_3);
        }
    };
    if inline {
        // The control is laid out first, right-aligned, so the labels know what is left.
        let mut out = None;
        ui.horizontal(|ui| {
            let full = ui.available_width();
            let r = ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ctrl = ui.scope(|ui| control(ui));
                let ctrl_w = ctrl.response.rect.width();
                out = Some(ctrl.inner);
                let rest = (full - ctrl_w - 16.0).max(80.0);
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.set_max_width(rest);
                    ui.vertical(|ui| labels(ui));
                });
            });
            let _ = r;
        });
        out.expect("control ran")
    } else {
        ui.vertical(|ui| {
            labels(ui);
            ui.add_space(4.0);
            control(ui)
        })
        .inner
    }
}

// ---- Card / Empty / PageHeader -----------------------------------------------------------

/// `Card`: surface, 1 px border, `--radius-lg`, soft shadow; an optional header row with a
/// title on the left and actions on the right, separated by a border. Returns the body's
/// value and the card's natural height (without `min_height`), so a grid row can give two
/// cards the same height without the measurement feeding on itself.
pub fn card<R>(
    ui: &mut Ui,
    title: Option<&str>,
    actions: impl FnOnce(&mut Ui),
    min_height: f32,
    body: impl FnOnce(&mut Ui) -> R,
) -> (R, f32) {
    let t = tokens(ui);
    let r = Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(RADIUS_LG))
        .shadow(style::card_shadow(&t))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let top = ui.cursor().top();
            ui.spacing_mut().item_spacing.y = 0.0;
            if let Some(title) = title {
                let w = ui.available_width();
                let (rect, _) = ui.allocate_exact_size(vec2(w, 42.0), Sense::hover());
                let mut head = ui.new_child(
                    egui::UiBuilder::new().max_rect(rect.shrink2(vec2(16.0, 0.0))).layout(Layout::left_to_right(Align::Center)),
                );
                text(&mut head, title, style::body_semibold(13.0), t.text);
                head.with_layout(Layout::right_to_left(Align::Center), |ui| actions(ui));
                ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, t.border));
            }
            ui.spacing_mut().item_spacing.y = 6.0;
            let inner = body(ui);
            let natural = ui.min_rect().bottom() - top;
            ui.set_min_height(min_height - 2.0);
            (inner, natural + 2.0)
        });
    r.inner
}

/// `Empty`: a dashed box with an icon, a title and a line of explanation.
pub fn empty(ui: &mut Ui, glyph: &str, title: &str, body: &str) {
    let t = tokens(ui);
    let w = ui.available_width();
    let r = Frame::new().inner_margin(Margin::symmetric(24, 56)).show(ui, |ui| {
        ui.set_width(w - 48.0);
        ui.vertical_centered(|ui| {
            icon(ui, glyph, 28.0, t.text_3);
            ui.add_space(10.0);
            text(ui, title, style::body_medium(14.0), t.text);
            ui.add_space(2.0);
            ui.set_max_width(420.0);
            ui.add(egui::Label::new(RichText::new(body).font(style::body(12.5)).color(t.text_2)).selectable(false));
        });
    });
    dashed_rect(ui, r.response.rect, &t);
}

fn dashed_rect(ui: &Ui, rect: Rect, t: &Tokens) {
    let rect = rect.shrink(0.5);
    let stroke = Stroke::new(1.0, t.border);
    let c = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
    for w in c.windows(2) {
        ui.painter().extend(Shape::dashed_line(&[w[0], w[1]], stroke, 4.0, 3.0));
    }
}

/// `PageHeader`: 16 px semibold title, optional subtitle, actions on the right. Padding
/// 24 px sides, 20 top, 12 bottom.
pub fn page_header(ui: &mut Ui, title: &str, subtitle: Option<&dyn Fn(&mut Ui)>, actions: impl FnOnce(&mut Ui)) {
    let t = tokens(ui);
    Frame::new().inner_margin(Margin { left: 24, right: 24, top: 20, bottom: 12 }).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            let full = ui.available_width();
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                let a = ui.horizontal(|ui| actions(ui)).response.rect.width();
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.set_max_width((full - a - 12.0).max(120.0));
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let r = text(ui, title, style::body_semibold(16.0), t.text);
                    ui.ctx().accesskit_node_builder(r.id, |n| n.set_role(Role::Heading));
                    if let Some(s) = subtitle {
                        s(ui);
                    }
                });
            });
        });
    });
}

// ---- Tooltip -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Top,
    Right,
}

/// The app's own tooltip (Tooltip.tsx): after 120 ms of hover (or on keyboard focus), a small
/// card above the trigger or beside it. Beside is what a collapsed sidebar needs: above, the
/// topmost item's card would be cut off by the top of the window. Multi-line labels keep
/// their line breaks (`whitespace-pre`).
pub fn tooltip(ui: &Ui, resp: &Response, label: &str, side: Side) {
    if label.is_empty() {
        return;
    }
    let ctx = ui.ctx();
    let key = resp.id.with("tip-since");
    let active = resp.hovered() || resp.has_focus();
    let now = ui.input(|i| i.time);
    let since: Option<f64> = ctx.data(|d| d.get_temp(key));
    if !active {
        if since.is_some() {
            ctx.data_mut(|d| d.remove::<f64>(key));
        }
        return;
    }
    let since = since.unwrap_or_else(|| {
        ctx.data_mut(|d| d.insert_temp(key, now));
        now
    });
    let wait = 0.12 - (now - since);
    if wait > 0.0 {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
        return;
    }
    let t = tokens(ui);
    let r = resp.rect;
    let (pivot, pos) = match side {
        Side::Top => (Align2::CENTER_BOTTOM, pos2(r.center().x, r.top() - 6.0)),
        Side::Right => (Align2::LEFT_CENTER, pos2(r.right() + 14.0, r.center().y)),
    };
    let screen = ctx.content_rect();
    egui::Area::new(resp.id.with("tip"))
        .order(egui::Order::Tooltip)
        .fixed_pos(pos)
        .pivot(pivot)
        .constrain_to(screen)
        .interactable(false)
        .show(ctx, |ui| {
            Frame::new()
                .fill(t.surface)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(RADIUS_MD))
                .shadow(style::pop_shadow(&t))
                .inner_margin(Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    let mut j = job(label, style::body(11.5), t.text);
                    j.wrap.max_width = f32::INFINITY;
                    let r = ui.add(egui::Label::new(j).selectable(false).extend());
                    ui.ctx().accesskit_node_builder(r.id, |n| n.set_role(Role::Tooltip));
                });
        });
}

// ---- Menu --------------------------------------------------------------------------------

pub fn popup_frame(t: &Tokens) -> Frame {
    Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(RADIUS_LG))
        .shadow(style::pop_shadow(t))
        .inner_margin(Margin::same(4))
}

/// One `role="menuitem"` row of a dropdown menu. `hint` is the grey text on the right.
pub fn menu_item(
    ui: &mut Ui,
    label: &str,
    glyph: Option<&str>,
    checked: bool,
    danger: bool,
    enabled: bool,
    hint: Option<&str>,
) -> Response {
    let t = tokens(ui);
    let font = style::body(13.0);
    let g = ui.painter().layout_job(job(label, font, Color32::PLACEHOLDER));
    let hint_g = hint.map(|h| ui.painter().layout_job(job(h, style::body(11.0), t.text_3)));
    let w = (8.0 + if glyph.is_some() { 26.0 } else { 0.0 } + g.size().x + hint_g.as_ref().map_or(0.0, |h| h.size().x + 12.0) + if checked { 24.0 } else { 0.0 } + 8.0)
        .max(ui.available_width());
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), sense);
    let owned = label.to_owned();
    resp.widget_info(|| WidgetInfo::labeled(egui::Role::Button, enabled, &owned));
    ui.ctx().accesskit_node_builder(resp.id, |n| n.set_role(Role::MenuItem));
    let ht = hover_t(ui, resp.id, enabled && resp.hovered());
    let fill = if danger { t.danger_soft.gamma_multiply(ht) } else { mix(Color32::TRANSPARENT, t.surface_2, ht) };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(RADIUS_MD), fill);
    let fg = if danger { t.danger } else { t.text };
    let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.4) };
    let mut x = rect.left() + 8.0;
    if let Some(gl) = glyph {
        paint_icon(ui, gl, pos2(x + 7.0, rect.center().y), 14.0, fade(t.text_3));
        x += 26.0;
    }
    let gy = rect.center().y - g.size().y / 2.0;
    ui.painter().galley(pos2(x, gy), g, fade(fg));
    let mut right = rect.right() - 8.0;
    if checked {
        paint_icon(ui, icon::CHECK, pos2(right - 7.0, rect.center().y), 14.0, t.accent);
        right -= 24.0;
    }
    if let Some(h) = hint_g {
        ui.painter().galley(pos2(right - h.size().x, rect.center().y - h.size().y / 2.0), h, fade(t.text_3));
    }
    if resp.clicked() {
        ui.close();
    }
    resp
}

pub fn menu_separator(ui: &mut Ui) {
    let t = tokens(ui);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, t.border));
}

/// A dropdown under `trigger` (Menu of ui.tsx): opens on click, closes on a pick, a click
/// outside or Escape. `min_width` is a minimum; the menu grows to its longest label.
pub fn menu<R>(trigger: &Response, min_width: f32, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let t = style::tokens(&trigger.ctx);
    egui::Popup::menu(trigger)
        .frame(popup_frame(&t))
        .align(egui::RectAlign::BOTTOM_END)
        .gap(4.0)
        .show(|ui| {
            ui.set_min_width(min_width - 8.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            add(ui)
        })
        .map(|r| r.inner)
}

// ---- Dialog ------------------------------------------------------------------------------

/// `Dialog`: a modal card 12 % from the top over a dimmed backdrop. Escape, the close button
/// or a click on the backdrop call `on_close` (by returning `true`). `footer` sits in a
/// tinted strip at the bottom with its buttons on the right.
pub fn dialog(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    width: f32,
    body: impl FnOnce(&mut Ui),
    footer: Option<&mut dyn FnMut(&mut Ui)>,
) -> bool {
    let t = style::tokens(ctx);
    let screen = ctx.content_rect();
    let top = screen.top() + screen.height() * 0.12;
    let width = width.min(screen.width() - 32.0);
    let area = egui::Modal::default_area(Id::unique(("dialog", id))).anchor(Align2::CENTER_TOP, vec2(0.0, top - screen.top()));
    let mut close = false;
    let resp = egui::Modal::new(Id::unique(("dialog", id)))
        .area(area)
        .backdrop_color(Color32::from_black_alpha(77))
        .frame(
            Frame::new()
                .fill(t.surface)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(12))
                .shadow(style::pop_shadow(&t)),
        )
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 0.0;
            Frame::new().inner_margin(Margin { left: 16, right: 12, top: 10, bottom: 10 }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let r = text(ui, title, style::body_semibold(14.0), t.text);
                    ui.ctx().accesskit_node_builder(r.id, |n| n.set_role(Role::Heading));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if Button::icon_only(icon::X, "Close").small().show(ui).clicked() {
                            close = true;
                        }
                    });
                });
            });
            let r = ui.available_rect_before_wrap();
            ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
            let max_body = (screen.height() * 0.76 - 110.0).max(120.0);
            egui::ScrollArea::vertical().max_height(max_body).auto_shrink([false, true]).show(ui, |ui| {
                Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    ui.set_width(width - 32.0);
                    body(ui);
                });
            });
            if let Some(f) = footer {
                let r = ui.available_rect_before_wrap();
                let strip = Rect::from_min_size(r.min, vec2(width, 54.0));
                ui.painter().rect_filled(
                    strip,
                    CornerRadius { nw: 0, ne: 0, sw: 12, se: 12 },
                    t.bg.gamma_multiply(0.6),
                );
                ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
                Frame::new().inner_margin(Margin::symmetric(16, 11)).show(ui, |ui| {
                    ui.set_width(width - 32.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        f(ui)
                    });
                });
            }
        });
    close || resp.should_close()
}

/// What [`confirm`] returned this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    Pending,
    Yes,
    No,
}

/// A yes/no dialog. `danger` paints the confirming button red (remove, purge).
pub fn confirm(ctx: &egui::Context, id: &str, title: &str, body: &str, yes: &str, danger: bool) -> Confirm {
    let mut out = Confirm::Pending;
    let t = style::tokens(ctx);
    let mut footer = |ui: &mut Ui| {
        let b = Button::new(yes).variant(if danger { Variant::Danger } else { Variant::Primary });
        if b.show(ui).clicked() {
            out = Confirm::Yes;
        }
        if Button::new("Cancel").show(ui).clicked() {
            out = Confirm::No;
        }
    };
    let closed = dialog(ctx, id, title, 420.0, |ui| {
        para(ui, body, style::body(13.0), t.text_2);
    }, Some(&mut footer));
    if closed && out == Confirm::Pending {
        out = Confirm::No;
    }
    out
}

/// Accessible text for screen readers and tests that a painted region otherwise lacks.
pub fn name(ui: &Ui, resp: &Response, label: &str) {
    let owned = label.to_owned();
    ui.ctx().accesskit_node_builder(resp.id, |n| n.set_label(owned));
}

/// `WidgetText` in a font/colour, for egui widgets that take one.
pub fn rich(s: &str, font: FontId, color: Color32) -> WidgetText {
    RichText::new(s).font(font).color(color).into()
}
