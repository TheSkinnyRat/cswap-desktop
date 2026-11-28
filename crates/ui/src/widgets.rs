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
use cswap_core::view::ChipTone;

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
    pub const LAYOUT_LIST: &str = "\u{E1D9}";
    pub const LAYOUT_GRID: &str = "\u{E0FF}";
    pub const PANEL_TOP: &str = "\u{E435}";
    pub const CORNER_DOWN_RIGHT: &str = "\u{E0A2}";
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

/// `Button` of ui.tsx. The label is also the accessible name; an icon-only button (empty
/// text) is square and gets its name from [`Button::label`].
#[must_use]
pub struct Button<'a> {
    text: &'a str,
    name: Option<&'a str>,
    variant: Variant,
    size: Size,
    icon: Option<&'a str>,
    trailing: Option<&'a str>,
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
            trailing: None,
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
    /// A small glyph after the text (the chevron of a menu button).
    pub fn trailing(mut self, glyph: &'a str) -> Self {
        self.trailing = Some(glyph);
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
    /// The accessible name, when it differs from the visible text (icon-only buttons).
    pub fn label(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }

    fn is_square(&self) -> bool {
        self.square || (self.text.is_empty() && self.trailing.is_none())
    }

    fn icon_size(&self) -> f32 {
        if self.size == Size::Sm { 13.0 } else { 14.0 }
    }

    /// Content width (icon, text, trailing glyph, gaps), without padding.
    fn content_width(&self, ui: &Ui) -> f32 {
        let gap = self.size.gap();
        let mut parts: Vec<f32> = Vec::new();
        if self.icon.is_some() || self.loading {
            parts.push(self.icon_size());
        }
        if !self.text.is_empty() {
            let font = style::body_medium(self.size.font());
            parts.push(ui.painter().layout_job(job(self.text, font, Color32::PLACEHOLDER)).size().x);
        }
        if self.trailing.is_some() {
            parts.push(12.0);
        }
        parts.iter().sum::<f32>() + gap * parts.len().saturating_sub(1) as f32
    }

    /// The width [`Button::show`] will take, for laying out a row before drawing it.
    pub fn width(&self, ui: &Ui) -> f32 {
        if self.is_square() {
            self.size.height()
        } else {
            (self.size.pad() * 2.0 + self.content_width(ui)).max(self.min_width)
        }
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let t = tokens(ui);
        let h = self.size.height();
        let width = self.width(ui);
        let content_w = self.content_width(ui);
        let icon_sz = self.icon_size();
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
            Variant::Ghost => {
                (mix(Color32::TRANSPARENT, t.surface_2, ht), mix(t.text_2, t.text, ht), Color32::TRANSPARENT)
            }
            Variant::Danger => {
                (mix(t.danger, mix(t.danger, Color32::WHITE, 0.12), ht), Color32::WHITE, Color32::TRANSPARENT)
            }
        };
        let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.5) };
        let painter = ui.painter();
        painter.rect(
            rect,
            CornerRadius::same(RADIUS_MD),
            fade(fill),
            Stroke::new(1.0, fade(border)),
            StrokeKind::Inside,
        );
        if self.variant == Variant::Primary {
            // `shadow-[inset_0_1px_0_rgba(255,255,255,0.12)]`: the faint top highlight.
            let top = Rect::from_min_size(rect.min + vec2(3.0, 0.5), vec2(rect.width() - 6.0, 1.0));
            painter.rect_filled(top, 0.0, Color32::from_white_alpha(fade(Color32::from_white_alpha(28)).a()));
        }
        if resp.has_focus() {
            focus_ring(ui, rect, &t);
        }
        let gap = self.size.gap();
        let cy = rect.center().y;
        let mut x = rect.center().x - content_w / 2.0;
        if self.loading {
            paint_spinner(ui, pos2(x + icon_sz / 2.0, cy), icon_sz, fade(fg));
            x += icon_sz + gap;
        } else if let Some(g) = self.icon {
            paint_icon(ui, g, pos2(x + icon_sz / 2.0, cy), icon_sz, fade(fg));
            x += icon_sz + gap;
        }
        if !self.text.is_empty() {
            let g = ui.painter().layout_job(job(self.text, style::body_medium(self.size.font()), fade(fg)));
            let gw = g.size().x;
            ui.painter().galley(pos2(x, cy - g.size().y / 2.0), g, fade(fg));
            x += gw + gap;
        }
        if let Some(g) = self.trailing {
            paint_icon(ui, g, pos2(x + 6.0, cy), 12.0, fade(fg).gamma_multiply(0.7));
        }
        resp
    }
}

/// `*:focus-visible { box-shadow: 0 0 0 2px var(--bg), 0 0 0 4px var(--accent) }`.
pub fn focus_ring(ui: &Ui, rect: Rect, t: &Tokens) {
    ui.painter().rect_stroke(
        rect.expand(2.0),
        CornerRadius::same(RADIUS_MD + 2),
        Stroke::new(2.0, t.accent),
        StrokeKind::Outside,
    );
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

impl From<ChipTone> for Tone {
    fn from(c: ChipTone) -> Self {
        match c {
            ChipTone::Ok => Tone::Ok,
            ChipTone::Warn => Tone::Warn,
            ChipTone::Danger => Tone::Danger,
            ChipTone::Muted => Tone::Muted,
            ChipTone::Accent => Tone::Accent,
        }
    }
}

impl From<cswap_core::format::Tone> for Tone {
    fn from(c: cswap_core::format::Tone) -> Self {
        use cswap_core::format::Tone as F;
        match c {
            F::Ok => Tone::Ok,
            F::Warn => Tone::Warn,
            F::Danger => Tone::Danger,
            F::Muted => Tone::Muted,
        }
    }
}

/// Width [`chip`] takes for `label`.
pub fn chip_width(ui: &Ui, label: &str) -> f32 {
    measure(ui, label, style::body_medium(11.0)).x + 12.0
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
pub fn chip(ui: &mut Ui, label: &str, tone: impl Into<Tone>) -> Response {
    let t = tokens(ui);
    let (bg, fg) = tone.into().colors(&t);
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
    input_opts(ui, label, value, placeholder, mono, width, false)
}

/// Where [`field`] leaves its label for the control it wraps, so an input inside it is
/// named by the label the user reads (`<label>` wrapping `<input>` in 0.1.23).
fn field_label_key() -> Id {
    Id::unique("cswap-field-label")
}

/// [`input`] with every option. An empty `label` takes the enclosing [`field`]'s label as
/// the accessible name; `password` hides the text (tokens).
pub fn input_opts(
    ui: &mut Ui,
    label: &str,
    value: &mut String,
    placeholder: &str,
    mono: bool,
    width: f32,
    password: bool,
) -> Response {
    let t = tokens(ui);
    let name = if label.is_empty() {
        ui.data(|d| d.get_temp::<String>(field_label_key())).unwrap_or_default()
    } else {
        label.to_owned()
    };
    let id = ui.make_persistent_id(("input", &name, placeholder));
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
        .password(password)
        .hint_text(RichText::new(placeholder).font(font).color(t.text_3))
        .frame(frame)
        .min_size(vec2(width, 32.0))
        .desired_width(width - 20.0);
    let resp = ui.add(edit);
    ui.ctx().accesskit_node_builder(resp.id, |n| n.set_label(name));
    resp
}

/// `Select`: a framed button with a chevron that opens a list. Returns true when the value
/// changed. `label` is the accessible name (0.1.23's `aria-label`).
pub fn select<T: PartialEq + Clone>(
    ui: &mut Ui,
    label: &str,
    value: &mut T,
    options: &[(T, String)],
    width: f32,
) -> bool {
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
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
            for (v, l) in options {
                let on = v == value;
                if menu_item(ui, l, None, on, false, true, None).clicked() && !on {
                    *value = v.clone();
                    changed = true;
                }
            }
        });
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

/// [`switch`] that flips `on` itself when clicked (and marks the response changed).
pub fn toggle(ui: &mut Ui, on: &mut bool, label: &str, enabled: bool) -> Response {
    let mut r = switch(ui, label, *on, enabled);
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    r
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
    ui.painter().rect(
        outer,
        CornerRadius::same(RADIUS_MD),
        t.surface_2,
        Stroke::new(1.0, t.border),
        StrokeKind::Inside,
    );
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
        // `items-center`: the label block and the control are centred on each other. Both
        // heights are known only after drawing, so they come from the previous frame (one
        // frame to settle, then stable).
        let id = ui.make_persistent_id(("field", label));
        let (row_h, labels_h): (f32, f32) = ui.data(|d| d.get_temp(id)).unwrap_or((0.0, 0.0));
        let full = ui.available_width();
        let mut out = None;
        let mut measured = (0.0, 0.0);
        ui.allocate_ui_with_layout(vec2(full, row_h.max(1.0)), Layout::right_to_left(Align::Center), |ui| {
            let ctrl = ui.scope(|ui| with_field_label(ui, label, control));
            let ctrl_h = ctrl.response.rect.height();
            let rest = (full - ctrl.response.rect.width() - 16.0).max(80.0);
            out = Some(ctrl.inner);
            let top = ui.max_rect().top() + ((row_h - labels_h) / 2.0).max(0.0);
            let rect = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(rest, labels_h.max(1.0)));
            let r =
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)), |ui| {
                    ui.set_width(rest);
                    labels(ui)
                });
            let lh = r.response.rect.height();
            measured = (ctrl_h.max(lh), lh);
        });
        if measured != (row_h, labels_h) {
            ui.data_mut(|d| d.insert_temp(id, measured));
            ui.ctx().request_repaint();
        }
        out.expect("control ran")
    } else {
        ui.vertical(|ui| {
            labels(ui);
            ui.add_space(4.0);
            with_field_label(ui, label, control)
        })
        .inner
    }
}

fn with_field_label<R>(ui: &mut Ui, label: &str, control: impl FnOnce(&mut Ui) -> R) -> R {
    let prev = ui.data_mut(|d| d.remove_temp::<String>(field_label_key()));
    ui.data_mut(|d| d.insert_temp(field_label_key(), label.to_owned()));
    let r = control(ui);
    ui.data_mut(|d| match prev {
        Some(p) => {
            d.insert_temp(field_label_key(), p);
        }
        None => {
            d.remove::<String>(field_label_key());
        }
    });
    r
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
                    egui::UiBuilder::new()
                        .max_rect(rect.shrink2(vec2(16.0, 0.0)))
                        .layout(Layout::left_to_right(Align::Center)),
                );
                text(&mut head, title, style::body_semibold(13.0), t.text);
                head.with_layout(Layout::right_to_left(Align::Center), |ui| actions(ui));
                ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, t.border));
            }
            ui.spacing_mut().item_spacing.y = 6.0;
            let inner = body(ui);
            let natural = ui.min_rect().bottom() - top;
            // `set_min_height` would add the height after the cursor, not from the top.
            ui.expand_to_include_y(top + min_height - 2.0);
            (inner, natural + 2.0)
        });
    r.inner
}

/// `Empty`: a dashed box with an icon, a title and a line of explanation.
pub fn empty(ui: &mut Ui, glyph: Option<&str>, title: &str, body: &str, action: impl FnOnce(&mut Ui)) {
    let t = tokens(ui);
    let w = ui.available_width();
    let r = Frame::new().inner_margin(Margin::symmetric(24, 56)).show(ui, |ui| {
        ui.set_width(w - 48.0);
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if let Some(g) = glyph {
                icon(ui, g, 28.0, t.text_3);
                ui.add_space(12.0);
            }
            let r = line(ui, title, style::body_medium(14.0), t.text);
            r.widget_info(|| WidgetInfo::labeled(Role::Label, true, title));
            ui.add_space(4.0);
            let g = wrapped(ui, body, style::body(12.5), t.text_2, (w - 48.0).min(420.0));
            let (rr, _) = ui.allocate_exact_size(g.size(), Sense::hover());
            ui.painter().galley(rr.min, g, t.text_2);
            ui.add_space(16.0);
            action(ui);
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
pub fn page_header(
    ui: &mut Ui,
    title: &str,
    subtitle: impl FnOnce(&mut Ui),
    actions_width: f32,
    actions: impl FnOnce(&mut Ui),
) {
    // `flex-wrap`: the actions sit beside the title while there is room, under it otherwise.
    let t = tokens(ui);
    let full = ui.available_width();
    let side_by_side = full - 48.0 - actions_width >= 220.0;
    Frame::new().inner_margin(Margin { left: 24, right: 24, top: 20, bottom: 12 }).show(ui, |ui| {
        let w = ui.available_width();
        let left_w = if side_by_side { w - actions_width - 12.0 } else { w };
        let top = ui.cursor().min;
        let left = place(ui, egui::UiBuilder::new().max_rect(Rect::from_min_size(top, vec2(left_w, 400.0))), |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.set_width(left_w);
            let r = line(ui, title, style::body_semibold(16.0), t.text);
            let owned = title.to_owned();
            ui.ctx().accesskit_node_builder(r.id, |n| {
                n.set_role(Role::Heading);
                n.set_label(owned);
            });
            subtitle(ui);
        })
        .response
        .rect;
        // no actions: no row for them either (Settings)
        let act_h = if actions_width > 0.0 { 32.0 } else { 0.0 };
        let act_rect = if side_by_side {
            Rect::from_min_size(pos2(top.x + w - actions_width, top.y), vec2(actions_width, act_h))
        } else {
            Rect::from_min_size(pos2(top.x, left.max.y + 12.0), vec2(actions_width.min(w), act_h))
        };
        let mut a =
            ui.new_child(egui::UiBuilder::new().max_rect(act_rect).layout(Layout::right_to_left(Align::Center)));
        a.spacing_mut().item_spacing.x = 8.0;
        actions(&mut a);
        let bottom = left.max.y.max(act_rect.max.y);
        ui.allocate_space(vec2(w, bottom - top.y));
    });
}
// ---- Tooltip -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Right,
    Left,
}

impl Side {
    /// The side to try next, in order: the asked-for side, the one opposite, then the two
    /// across from them.
    fn order(self) -> [Side; 4] {
        match self {
            Side::Top => [Side::Top, Side::Bottom, Side::Right, Side::Left],
            Side::Bottom => [Side::Bottom, Side::Top, Side::Right, Side::Left],
            Side::Right => [Side::Right, Side::Left, Side::Bottom, Side::Top],
            Side::Left => [Side::Left, Side::Right, Side::Bottom, Side::Top],
        }
    }
}

/// How far a tooltip keeps from the window's edges.
const TIP_MARGIN: f32 = 4.0;

/// Where a `size` tooltip goes for `anchor` inside `screen`: on `side`, `gap` away, centred on
/// the anchor and slid along that edge to stay inside the window. A side the card does not fit
/// on is skipped for the next in [`Side::order`]; with no side that fits, the one with the most
/// room takes it and the card runs off the window there. The card is never moved over the
/// anchor: 1.0.0 clamped it into the window instead, so the title bar's refresh tooltip (no room
/// above) landed on the button it describes.
pub fn place_tip(anchor: Rect, size: Vec2, screen: Rect, side: Side, gap: f32) -> Rect {
    let inner = screen.shrink(TIP_MARGIN);
    // Centred on the anchor along the edge it sits on, then slid inside the window; a card
    // wider than the window starts at its left (top) edge.
    let along = |centre: f32, len: f32, lo: f32, hi: f32| (centre - len / 2.0).min(hi - len).max(lo);
    let at = |s: Side| -> Rect {
        let min = match s {
            Side::Top => {
                pos2(along(anchor.center().x, size.x, inner.left(), inner.right()), anchor.top() - gap - size.y)
            }
            Side::Bottom => pos2(along(anchor.center().x, size.x, inner.left(), inner.right()), anchor.bottom() + gap),
            Side::Right => pos2(anchor.right() + gap, along(anchor.center().y, size.y, inner.top(), inner.bottom())),
            Side::Left => {
                pos2(anchor.left() - gap - size.x, along(anchor.center().y, size.y, inner.top(), inner.bottom()))
            }
        };
        Rect::from_min_size(min, size)
    };
    let room = |s: Side| match s {
        Side::Top => anchor.top() - gap - inner.top(),
        Side::Bottom => inner.bottom() - anchor.bottom() - gap,
        Side::Right => inner.right() - anchor.right() - gap,
        Side::Left => anchor.left() - gap - inner.left(),
    };
    let need = |s: Side| if matches!(s, Side::Top | Side::Bottom) { size.y } else { size.x };
    let order = side.order();
    let s = order
        .into_iter()
        .find(|s| room(*s) >= need(*s))
        .unwrap_or_else(|| order.into_iter().fold(side, |best, s| if room(s) > room(best) { s } else { best }));
    at(s)
}

/// The app's own tooltip (Tooltip.tsx): after 120 ms of hover (or on keyboard focus), a small
/// card on `side` of the trigger, or on another side where that one has no room
/// ([`place_tip`]): it never covers what it describes. Beside is what a collapsed sidebar
/// needs. Multi-line labels keep their line breaks (`whitespace-pre`); a line wider than the
/// window wraps.
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
    let screen = ctx.content_rect();
    // Laid out here, not by the Area, because where the card goes depends on its size.
    let pad = vec2(8.0, 4.0);
    let mut j = job(label, style::body(11.5), t.text);
    j.wrap.max_width = (screen.width() - 2.0 * (TIP_MARGIN + pad.x + 1.0)).max(40.0);
    let galley = ui.painter().layout_job(j);
    // galley + inner margin + the 1 px stroke on each side
    let size = galley.size() + 2.0 * pad + vec2(2.0, 2.0);
    let gap = if side == Side::Right || side == Side::Left { 14.0 } else { 6.0 };
    let at = place_tip(resp.rect, size, screen, side, gap);
    egui::Area::new(resp.id.with("tip"))
        .order(egui::Order::Tooltip)
        .fixed_pos(at.min)
        .pivot(Align2::LEFT_TOP)
        .constrain(false)
        .interactable(false)
        .show(ctx, |ui| {
            Frame::new()
                .fill(t.surface)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(RADIUS_MD))
                .shadow(style::tip_shadow(&t))
                .inner_margin(Margin::symmetric(pad.x as i8, pad.y as i8))
                .show(ui, |ui| {
                    let r = ui.add(egui::Label::new(galley).selectable(false));
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
    let w = 8.0
        + if glyph.is_some() { 26.0 } else { 0.0 }
        + g.size().x
        + hint_g.as_ref().map_or(0.0, |h| h.size().x + 12.0)
        + if checked { 24.0 } else { 0.0 }
        + 8.0;
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    // In the menu's justified layout this takes the menu's width; the content width above is
    // what sizes the menu in its sizing pass, so the longest label decides.
    let (rect, resp) = ui.allocate_at_least(vec2(w, 30.0), sense);
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
        .width(min_width)
        .frame(popup_frame(&t))
        .align(egui::RectAlign::BOTTOM_END)
        .gap(4.0)
        .show(|ui| {
            ui.set_min_width(min_width - 8.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.with_layout(Layout::top_down_justified(Align::Min), add).inner
        })
        .map(|r| r.inner)
}

// ---- Dialog ------------------------------------------------------------------------------

/// What a [`dialog`] asks of its caller this frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DialogOut {
    /// Escape, the close button or a click on the backdrop: the caller closes it.
    pub close: bool,
}

/// `Dialog`: a modal card 12 % from the top over a dimmed backdrop. Escape, the close button
/// or a click on the backdrop set [`DialogOut::close`]. `footer` sits in a tinted strip at
/// the bottom, laid out right to left (put the primary button first).
pub fn dialog(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    width: f32,
    body: impl FnOnce(&mut Ui),
    footer: impl FnOnce(&mut Ui),
) -> DialogOut {
    let t = style::tokens(ctx);
    let screen = ctx.content_rect();
    let top = screen.top() + screen.height() * 0.12;
    let width = width.min(screen.width() - 32.0);
    let area =
        egui::Modal::default_area(Id::unique(("dialog", id))).anchor(Align2::CENTER_TOP, vec2(0.0, top - screen.top()));
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
                    ui.spacing_mut().item_spacing.y = 10.0;
                    ui.set_width(width - 32.0);
                    body(ui);
                });
            });
            {
                let f = footer;
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
    DialogOut { close: close || resp.should_close() }
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
    let footer = |ui: &mut Ui| {
        let b = Button::new(yes).variant(if danger { Variant::Danger } else { Variant::Primary });
        if b.show(ui).clicked() {
            out = Confirm::Yes;
        }
        if Button::new("Cancel").show(ui).clicked() {
            out = Confirm::No;
        }
    };
    let closed = dialog(
        ctx,
        id,
        title,
        420.0,
        |ui| {
            para(ui, body, style::body(13.0), t.text_2);
        },
        footer,
    )
    .close;
    if closed && out == Confirm::Pending {
        out = Confirm::No;
    }
    out
}

// ---- Painted text and layout helpers --------------------------------------------------------
//
// Text the pages position themselves (table cells, cards): painted galleys that take exactly
// their size and add no accesskit node, so a page's own `widget_info` names stay unique.

/// One line cut to `width` with an ellipsis.
pub fn galley(
    ui: &Ui,
    text: impl Into<String>,
    font: FontId,
    color: Color32,
    width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::simple_singleline(text.into(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(1.0));
    ui.painter().layout_job(job)
}

/// Text wrapped at `width`.
pub fn wrapped(
    ui: &Ui,
    text: impl Into<String>,
    font: FontId,
    color: Color32,
    width: f32,
) -> std::sync::Arc<egui::Galley> {
    ui.painter().layout(text.into(), font, color, width.max(1.0))
}

/// Size of a run of text, for laying a row out before painting it.
pub fn measure(ui: &Ui, s: &str, font: FontId) -> Vec2 {
    ui.painter().layout_no_wrap(s.to_owned(), font, Color32::WHITE).size()
}

/// One painted line, truncated to the available width.
pub fn line(ui: &mut Ui, s: impl Into<String>, font: FontId, color: Color32) -> Response {
    let g = galley(ui, s, font, color, ui.available_width());
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(rect.min, g, color);
    resp
}

/// A painted paragraph wrapped at the available width.
pub fn wrap_text(ui: &mut Ui, s: impl Into<String>, font: FontId, color: Color32) -> Response {
    let g = wrapped(ui, s, font, color, ui.available_width());
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(rect.min, g, color);
    resp
}

/// Lays `f` out in a child at the builder's rect WITHOUT taking space in `ui` (unlike
/// `scope_builder`): callers that position cells absolutely allocate the whole row
/// themselves, and allocating each cell too made every row twice as tall.
pub fn place<R>(ui: &mut Ui, b: egui::UiBuilder, f: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let mut child = ui.new_child(b);
    let inner = f(&mut child);
    let mut response = child.response();
    response.rect = child.min_rect();
    egui::InnerResponse::new(inner, response)
}

/// A tinted notice strip (errors, stale data).
pub fn notice(ui: &mut Ui, msg: &str, tone: impl Into<Tone>) {
    let t = tokens(ui);
    let (bg, fg) = tone.into().colors(&t);
    let w = ui.available_width();
    Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, fg.gamma_multiply(0.3)))
        .corner_radius(CornerRadius::same(RADIUS_LG - 2))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            // inner width + margins + the 1 px stroke on both sides = the row's width
            ui.set_width(w - 26.0);
            wrap_text(ui, msg, style::body(12.5), fg);
        });
}

/// An inline code span (`mono rounded bg-surface-2`).
pub fn code(ui: &mut Ui, s: &str) -> Response {
    let t = tokens(ui);
    let g = ui.painter().layout_no_wrap(s.to_owned(), style::mono(12.0), t.text);
    let (r, resp) = ui.allocate_exact_size(g.size() + vec2(8.0, 2.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same(RADIUS_SM - 1), t.surface_2);
    ui.painter().galley(r.min + vec2(4.0, 1.0), g, t.text);
    resp
}

/// The frame of [`card`] without its header, for lists that lay out their own rows.
pub fn card_frame(t: &Tokens) -> Frame {
    Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(RADIUS_LG))
        .shadow(style::card_shadow(t))
}

// ---- Menu from a list ------------------------------------------------------------------------

/// One entry of [`menu_list`]; `None` in the list is a separator.
#[derive(Debug, Clone, Copy, Default)]
pub struct MenuItem<'a> {
    pub label: &'a str,
    pub hint: Option<&'a str>,
    pub danger: bool,
    pub icon: Option<&'a str>,
}

pub fn item(label: &str) -> Option<MenuItem<'_>> {
    Some(MenuItem { label, ..Default::default() })
}

pub fn item_hint<'a>(label: &'a str, hint: &'a str) -> Option<MenuItem<'a>> {
    Some(MenuItem { label, hint: Some(hint), ..Default::default() })
}

impl<'a> MenuItem<'a> {
    pub fn with_icon(mut self, glyph: &'a str) -> Self {
        self.icon = Some(glyph);
        self
    }
}

/// A dropdown under `anchor` built from a list; returns the index picked this frame.
pub fn menu_list(anchor: &Response, width: f32, items: &[Option<MenuItem<'_>>]) -> Option<usize> {
    let mut picked = None;
    menu(anchor, width, |ui| {
        for (i, it) in items.iter().enumerate() {
            match it {
                None => menu_separator(ui),
                Some(it) => {
                    if menu_item(ui, it.label, it.icon, false, it.danger, true, it.hint).clicked() {
                        picked = Some(i);
                    }
                }
            }
        }
    });
    picked
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

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect { min: Pos2::ZERO, max: Pos2 { x: 800.0, y: 600.0 } };

    fn every_side() -> [Side; 4] {
        [Side::Top, Side::Bottom, Side::Right, Side::Left]
    }

    // 1.0.0 clamped the card into the window, so with no room on its side it slid onto the
    // anchor: the title bar's refresh tooltip covered the refresh button.
    #[test]
    fn a_tooltip_with_no_room_on_its_side_flips_instead_of_covering_the_anchor() {
        let refresh = Rect::from_min_size(pos2(600.0, 6.0), vec2(60.0, 26.0));
        let tip = place_tip(refresh, vec2(180.0, 24.0), SCREEN, Side::Top, 6.0);
        assert!(!tip.intersects(refresh), "{tip:?}");
        assert!(tip.top() >= refresh.bottom(), "flipped below: {tip:?}");
        assert!(SCREEN.contains_rect(tip));

        let edge = Rect::from_min_size(pos2(760.0, 300.0), vec2(40.0, 32.0));
        let tip = place_tip(edge, vec2(120.0, 24.0), SCREEN, Side::Right, 14.0);
        assert!(tip.right() <= edge.left(), "flipped to the left: {tip:?}");
    }

    // Near a corner the card slides along its edge to stay inside, but never over the anchor.
    #[test]
    fn a_tooltip_slides_along_its_edge_to_stay_in_the_window() {
        let corner = Rect::from_min_size(pos2(770.0, 560.0), vec2(30.0, 30.0));
        for side in every_side() {
            let tip = place_tip(corner, vec2(200.0, 40.0), SCREEN, side, 6.0);
            assert!(!tip.intersects(corner), "{side:?}: {tip:?}");
            assert!(SCREEN.contains_rect(tip), "{side:?}: {tip:?}");
        }
    }

    // Whatever the sizes, the card never lands on what it describes; when it cannot fit
    // anywhere it takes the roomiest side and runs off the window there.
    #[test]
    fn a_tooltip_never_overlaps_its_anchor() {
        let anchors = [
            Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 40.0)),
            Rect::from_min_size(pos2(380.0, 280.0), vec2(40.0, 40.0)),
            Rect::from_min_size(pos2(10.0, 10.0), vec2(780.0, 580.0)),
            Rect::from_min_size(pos2(0.0, 500.0), vec2(800.0, 100.0)),
        ];
        let sizes = [vec2(60.0, 20.0), vec2(500.0, 120.0), vec2(1000.0, 30.0), vec2(30.0, 900.0)];
        for a in anchors {
            for s in sizes {
                for side in every_side() {
                    let tip = place_tip(a, s, SCREEN, side, 6.0);
                    assert!(!tip.intersects(a), "{a:?} {s:?} {side:?}: {tip:?}");
                    assert_eq!(tip.size(), s);
                }
            }
        }
        // A full-width strip at the bottom: above is the only room.
        let strip = Rect::from_min_size(pos2(0.0, 500.0), vec2(800.0, 100.0));
        assert!(place_tip(strip, vec2(60.0, 20.0), SCREEN, Side::Bottom, 6.0).bottom() <= strip.top());
    }
}
