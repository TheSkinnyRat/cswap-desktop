//! Design tokens: 0.1.23's CSS variables (`src/renderer/src/styles/app.css`) as egui
//! colors. Light is the default and it is TINTED, never plain white; dark is opt-in.
//! Applying them to `egui::Style` (fonts, spacing, widget visuals) is ui-shell's job.

use egui::Color32;

#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub surface_2: Color32,
    pub surface_3: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_2: Color32,
    pub text_3: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub accent_fg: Color32,
    pub ok: Color32,
    pub ok_soft: Color32,
    pub warn: Color32,
    pub warn_soft: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
}

/// Radii, px (`--radius-sm/md/lg`).
pub const RADIUS_SM: u8 = 5;
pub const RADIUS_MD: u8 = 7;
pub const RADIUS_LG: u8 = 10;
/// Body text 13 px, mono 12 px, line height 1.45.
pub const FONT_BODY: f32 = 13.0;
pub const FONT_MONO: f32 = 12.0;
/// Hover and panel transitions: Purwa prefers ~150 ms; 300 ms reads as slow.
pub const TRANSITION_SECS: f32 = 0.15;

impl Tokens {
    pub fn light() -> Self {
        Self {
            dark: false,
            bg: hsl(228.0, 14.0, 94.5, 1.0),
            surface: hsl(228.0, 18.0, 98.0, 1.0),
            surface_2: hsl(228.0, 14.0, 92.0, 1.0),
            surface_3: hsl(228.0, 12.0, 89.0, 1.0),
            border: hsl(228.0, 12.0, 86.0, 1.0),
            border_strong: hsl(228.0, 10.0, 76.0, 1.0),
            text: hsl(232.0, 22.0, 13.0, 1.0),
            text_2: hsl(232.0, 10.0, 40.0, 1.0),
            text_3: hsl(232.0, 8.0, 58.0, 1.0),
            accent: hsl(235.0, 58.0, 57.0, 1.0),
            accent_hover: hsl(235.0, 58.0, 52.0, 1.0),
            accent_soft: hsl(235.0, 58.0, 57.0, 0.12),
            accent_fg: Color32::WHITE,
            ok: hsl(152.0, 52.0, 40.0, 1.0),
            ok_soft: hsl(152.0, 52.0, 40.0, 0.14),
            warn: hsl(36.0, 92.0, 46.0, 1.0),
            warn_soft: hsl(36.0, 92.0, 50.0, 0.16),
            danger: hsl(2.0, 68.0, 54.0, 1.0),
            danger_soft: hsl(2.0, 68.0, 54.0, 0.12),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            bg: hsl(240.0, 6.0, 8.0, 1.0),
            surface: hsl(240.0, 6.0, 11.0, 1.0),
            surface_2: hsl(240.0, 6.0, 14.0, 1.0),
            surface_3: hsl(240.0, 6.0, 17.0, 1.0),
            border: hsl(240.0, 6.0, 18.0, 1.0),
            border_strong: hsl(240.0, 6.0, 27.0, 1.0),
            text: hsl(240.0, 12.0, 93.0, 1.0),
            text_2: hsl(240.0, 7.0, 66.0, 1.0),
            text_3: hsl(240.0, 6.0, 48.0, 1.0),
            accent: hsl(235.0, 58.0, 57.0, 1.0),
            accent_hover: hsl(235.0, 62.0, 64.0, 1.0),
            accent_soft: hsl(235.0, 70.0, 70.0, 0.18),
            accent_fg: Color32::WHITE,
            ok: hsl(152.0, 48.0, 52.0, 1.0),
            ok_soft: hsl(152.0, 48.0, 52.0, 0.16),
            warn: hsl(38.0, 90.0, 58.0, 1.0),
            warn_soft: hsl(38.0, 90.0, 58.0, 0.16),
            danger: hsl(2.0, 72.0, 64.0, 1.0),
            danger_soft: hsl(2.0, 72.0, 64.0, 0.14),
        }
    }
}

/// CSS `hsl(h s% l% / a)` → premultiplied egui color.
pub fn hsl(h: f32, s: f32, l: f32, a: f32) -> Color32 {
    let (s, l) = (s / 100.0, l / 100.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h % 360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(to(r), to(g), to(b), (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The light theme must never be plain white: a regression here silently changes the
    // whole look the README promises.
    #[test]
    fn hsl_matches_css_and_light_is_tinted() {
        assert_eq!(hsl(0.0, 100.0, 50.0, 1.0), Color32::from_rgb(255, 0, 0));
        assert_eq!(hsl(235.0, 58.0, 57.0, 1.0), Color32::from_rgb(82, 92, 209));
        let t = Tokens::light();
        assert_ne!(t.surface, Color32::WHITE);
        assert_ne!(t.bg, Color32::WHITE);
    }
}
