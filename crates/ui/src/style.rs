//! Fonts and `egui::Style` from [`Tokens`]. OWNER: ui-shell. Every window (the app's, the
//! kittest harnesses, the demo) calls [`setup`] once and [`apply_theme`] when the setting
//! changes; widgets then read the current palette with [`tokens`].

use std::sync::Arc;

use cswap_core::ThemeSetting;
use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Theme,
    ThemePreference, Visuals,
    epaint::{
        Shadow,
        text::{FontTweak, VariationCoords},
    },
};

use crate::theme::{self, Tokens};

/// Inter 4.1, variable (SIL OFL 1.1, `assets/fonts/Inter-LICENSE.txt`). One file serves
/// every weight through the `wght` axis, which egui 0.37 applies per registered face.
const INTER: &[u8] = include_bytes!("../assets/fonts/InterVariable.ttf");
/// The lucide icons 0.1.23 used (lucide-static 1.82, ISC), subset to what the app draws:
/// 20 KB instead of 1 MB. To add one, re-subset from lucide-static and add a const in
/// [`crate::widgets::icon`].
const LUCIDE: &[u8] = include_bytes!("../assets/fonts/lucide.ttf");

/// Inter at weight 500 (`font-medium`).
pub fn medium() -> FontFamily {
    FontFamily::Name("inter-500".into())
}
/// Inter at weight 600 (`font-semibold`).
pub fn semibold() -> FontFamily {
    FontFamily::Name("inter-600".into())
}
/// The lucide icon font. Glyphs fill the em box, so an icon of size N is an N x N square.
pub fn icons() -> FontFamily {
    FontFamily::Name("icons".into())
}

pub fn body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn body_medium(size: f32) -> FontId {
    FontId::new(size, medium())
}
pub fn body_semibold(size: f32) -> FontId {
    FontId::new(size, semibold())
}
pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}
pub fn icon(size: f32) -> FontId {
    FontId::new(size, icons())
}

fn inter_at(weight: f32) -> FontData {
    // Without the coordinate every Inter face renders at the file's default instance (400),
    // so medium and semibold text would silently look regular.
    FontData::from_static(INTER).tweak(FontTweak { coords: VariationCoords::new([("wght", weight)]), ..Default::default() })
}

pub fn font_definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    defs.font_data.insert("inter-400".into(), Arc::new(inter_at(400.0)));
    defs.font_data.insert("inter-500".into(), Arc::new(inter_at(500.0)));
    defs.font_data.insert("inter-600".into(), Arc::new(inter_at(600.0)));
    defs.font_data.insert("lucide".into(), Arc::new(FontData::from_static(LUCIDE)));
    // What egui ships for Proportional (Ubuntu-Light + emoji) stays as the fallback chain.
    let fallback = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let chain = |first: &str| {
        let mut v = vec![first.to_owned()];
        v.extend(fallback.iter().cloned());
        v.push("lucide".into());
        v
    };
    defs.families.insert(FontFamily::Proportional, chain("inter-400"));
    defs.families.insert(medium(), chain("inter-500"));
    defs.families.insert(semibold(), chain("inter-600"));
    defs.families.insert(icons(), vec!["lucide".into()]);
    if let Some(m) = defs.families.get_mut(&FontFamily::Monospace) {
        m.push("inter-400".into());
    }
    defs
}

/// Fonts, both palettes and the spacing. Once per window.
pub fn setup(ctx: &Context, theme: ThemeSetting) {
    ctx.set_fonts(font_definitions());
    ctx.set_visuals_of(Theme::Light, visuals(&Tokens::light()));
    ctx.set_visuals_of(Theme::Dark, visuals(&Tokens::dark()));
    ctx.all_styles_mut(|style| {
        style.animation_time = theme::TRANSITION_SECS;
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.interact_size.y = 18.0;
        // Floating scroll bars overlay the content instead of taking width, so a page that
        // grows past the window does not shift sideways (no hover effect moves layout).
        style.spacing.scroll.floating = true;
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.scroll.foreground_color = false;
        style.interaction.tooltip_delay = 0.12;
        style.text_styles = [
            (TextStyle::Heading, body_semibold(16.0)),
            (TextStyle::Body, body(theme::FONT_BODY)),
            (TextStyle::Button, body_medium(theme::FONT_BODY)),
            (TextStyle::Small, body(11.5)),
            (TextStyle::Monospace, mono(theme::FONT_MONO)),
        ]
        .into();
    });
    apply_theme(ctx, theme);
}

/// Light / dark / follow the OS. eframe reports the OS theme, so `System` needs nothing else.
pub fn apply_theme(ctx: &Context, theme: ThemeSetting) {
    ctx.set_theme(match theme {
        ThemeSetting::Light => ThemePreference::Light,
        ThemeSetting::Dark => ThemePreference::Dark,
        ThemeSetting::System => ThemePreference::System,
    });
}

/// The palette of the theme this frame is drawn in.
pub fn tokens(ctx: &Context) -> Tokens {
    let dark = ctx.theme() == Theme::Dark;
    let id = egui::Id::unique(("cswap-tokens", dark));
    if let Some(t) = ctx.data(|d| d.get_temp::<Arc<Tokens>>(id)) {
        return (*t).clone();
    }
    let t = if dark { Tokens::dark() } else { Tokens::light() };
    ctx.data_mut(|d| d.insert_temp(id, Arc::new(t.clone())));
    t
}

fn visuals(t: &Tokens) -> Visuals {
    let mut v = if t.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = t.bg;
    v.window_fill = t.surface;
    v.extreme_bg_color = t.surface;
    v.text_edit_bg_color = Some(t.surface);
    v.faint_bg_color = t.surface_2;
    v.code_bg_color = t.surface_2;
    v.window_stroke = Stroke::new(1.0, t.border);
    v.window_corner_radius = CornerRadius::same(theme::RADIUS_LG + 2);
    v.menu_corner_radius = CornerRadius::same(theme::RADIUS_LG);
    v.window_shadow = pop_shadow(t);
    v.popup_shadow = pop_shadow(t);
    v.override_text_color = Some(t.text);
    v.weak_text_color = Some(t.text_3);
    v.hyperlink_color = t.accent;
    v.selection.bg_fill = t.accent_soft;
    v.selection.stroke = Stroke::new(1.0, t.accent);
    v.warn_fg_color = t.warn;
    v.error_fg_color = t.danger;
    v.text_cursor.stroke = Stroke::new(1.5, t.accent);
    // The painted widgets ignore these; what still reads them is egui's own ComboBox
    // popup rows, the scroll bars, text edits and collapsing headers.
    for (w, fill, stroke) in [
        (&mut v.widgets.noninteractive, t.surface, t.border),
        (&mut v.widgets.inactive, t.surface_2, t.border),
        (&mut v.widgets.hovered, t.surface_2, t.border_strong),
        (&mut v.widgets.active, t.surface_3, t.accent),
        (&mut v.widgets.open, t.surface_2, t.border_strong),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0, stroke);
        w.corner_radius = CornerRadius::same(theme::RADIUS_MD);
        w.fg_stroke = Stroke::new(1.0, t.text);
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.text_2);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, t.text_2);
    // Scroll bar thumb: `--border-strong`, like the old thin scrollbars.
    v.widgets.inactive.bg_fill = t.border_strong;
    v
}

/// `--shadow-pop`, approximated: egui has one blurred shadow per shape.
pub fn pop_shadow(t: &Tokens) -> Shadow {
    let a = if t.dark { 150 } else { 46 };
    Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(a) }
}

/// A small card's pop shadow (tooltips): the full one dwarfs a one-line label.
pub fn tip_shadow(t: &Tokens) -> Shadow {
    let a = if t.dark { 120 } else { 30 };
    Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(a) }
}

/// `--shadow` (cards).
pub fn card_shadow(t: &Tokens) -> Shadow {
    let a = if t.dark { 70 } else { 12 };
    Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(a) }
}

/// Linear mix of two colours, `t` = share of `b`. The ~150 ms hover blends.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}
