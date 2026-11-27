//! Usage bars, rings and the pace tick. Port of `components/UsageRing.tsx` and the bar parts of `pages/Accounts.tsx`. OWNER: ui-accounts.
//!
//! The pace tick (0.1.13–0.1.15, Purwa picked the tick out of four designs) marks where an
//! evenly spread week would be by now. Only weekly windows carry `expectedPct`, so a
//! 5-hour window never shows one. On a bar it stands proud of the track; on a ring it is
//! a notch OUTSIDE the stroke, because drawn across the track a full arc covers it.

use std::f32::consts::TAU;

use egui::{Color32, CornerRadius, Id, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, pos2, vec2};

use crate::style;
use crate::widgets;
use crate::theme::Tokens;
use cswap_core::format::{Tone, pct};

/// Grow animation of bars and rings (`duration-500 ease-out`).
const GROW_SECS: f32 = 0.5;
const TRACK_H: f32 = 5.0;
/// The pace marker reaches this far above and below the bar's track.
const MARK_OVERHANG: f32 = 4.0;

const RING: f32 = 30.0;
const RING_STROKE: f32 = 3.5;
// Room between the stroke and the box edge, so the pace notch can sit outside the ring.
const RING_R: f32 = (RING - RING_STROKE) / 2.0 - 2.5;

/// Fill colour of a percentage; same thresholds for bars and rings, so one glance means
/// the same thing in both views. `Ok` is the accent, not green.
pub fn fill_color(t: &Tokens, tone: Tone) -> Color32 {
    match tone {
        Tone::Ok => t.accent,
        Tone::Warn => t.warn,
        Tone::Danger => t.danger,
        Tone::Muted => t.border_strong,
    }
}

/// Text colour of a percentage.
pub fn pct_color(t: &Tokens, tone: Tone) -> Color32 {
    match tone {
        Tone::Danger => t.danger,
        Tone::Warn => t.warn,
        _ => t.text,
    }
}

fn clamp_pct(p: Option<f64>) -> f32 {
    p.map(|p| p.clamp(0.0, 100.0) as f32).unwrap_or(0.0)
}

/// x of the pace marker on a track: `pace` percent of the way along it.
pub fn pace_x(track: Rect, pace: f64) -> f32 {
    track.min.x + track.width() * (pace.clamp(0.0, 100.0) as f32 / 100.0)
}

/// The point at `frac` of a turn CLOCKWISE FROM TWELVE O'CLOCK, the way the ring fills.
/// 0.1.14's notch sat a quarter turn early because its rotation started at three o'clock;
/// everything on the ring goes through this one function so that cannot recur.
pub fn clock_point(center: Pos2, radius: f32, frac: f32) -> Pos2 {
    let a = frac * TAU - TAU / 4.0;
    center + vec2(a.cos(), a.sin()) * radius
}

/// A 5 px track filled to `pct`, `width` wide, with the pace marker when `pace` is set.
/// The marker overhangs without taking layout space: a row's height must not change
/// because a window happens to report a pace.
pub fn bar(ui: &mut Ui, t: &Tokens, id: Id, pct: Option<f64>, tone: Tone, pace: Option<f64>, width: f32) -> Response {
    let (track, resp) = ui.allocate_exact_size(vec2(width, TRACK_H), Sense::hover());
    let shown = ui.ctx().animate_value_with_time(id.with("grow"), clamp_pct(pct), GROW_SECS);
    let p = ui.painter();
    p.rect_filled(track, CornerRadius::same(3), t.surface_3);
    if shown > 0.0 {
        let fill = Rect::from_min_size(track.min, vec2(track.width() * shown / 100.0, TRACK_H));
        p.rect_filled(fill, CornerRadius::same(3), fill_color(t, tone));
    }
    if let Some(m) = pace {
        let x = pace_x(track, m);
        let r =
            Rect::from_min_max(pos2(x - 1.0, track.min.y - MARK_OVERHANG), pos2(x + 1.0, track.max.y + MARK_OVERHANG));
        p.rect_filled(r, CornerRadius::same(1), t.text_2);
    }
    resp
}

/// Width a ring cell wants.
pub fn ring_width(ui: &Ui, label: Option<&str>, value: &str, sub: Option<&str>) -> f32 {
    let head = format!("{}{value}", label.map(|l| format!("{l} ")).unwrap_or_default());
    let w1 = widgets::measure(ui, &head, style::body(12.0)).x;
    let w2 = sub.map(|s| widgets::measure(ui, s, style::body(11.0)).x).unwrap_or(0.0);
    (4.0 + RING + 8.0 + w1.max(w2) + 4.0).max(92.0)
}

/// `UsageRing`: a 30 px ring with the percentage (and the reset, `sub`) beside it.
#[allow(clippy::too_many_arguments)]
pub fn ring(
    ui: &mut Ui,
    t: &Tokens,
    id: Id,
    pct_value: Option<f64>,
    pace: Option<f64>,
    label: Option<&str>,
    sub: Option<&str>,
    max_width: f32,
) -> Response {
    let value = pct_value.map(|p| p.clamp(0.0, 100.0)).unwrap_or(0.0);
    let text = if pct_value.is_none() { "—".to_string() } else { pct(pct_value) };
    let want = ring_width(ui, label, &text, sub).min(max_width.max(RING + 8.0));
    let (rect, resp) = ui.allocate_exact_size(vec2(want, 34.0), Sense::hover());
    let hover = ui.ctx().animate_bool_with_time(id.with("hover"), resp.hovered(), crate::theme::TRANSITION_SECS);
    let p = ui.painter();
    if hover > 0.0 {
        p.rect_filled(
            rect,
            CornerRadius::same(crate::theme::RADIUS_MD - 1),
            Color32::TRANSPARENT.lerp_to_gamma(t.surface_2, hover),
        );
    }
    let c = pos2(rect.min.x + 4.0 + RING / 2.0, rect.center().y);
    p.circle_stroke(c, RING_R, Stroke::new(RING_STROKE, t.border_strong.gamma_multiply(0.4)));
    if let Some(m) = pace {
        let frac = m.clamp(0.0, 100.0) as f32 / 100.0;
        let outer = RING / 2.0 - 0.6;
        let inner = RING_R + RING_STROKE / 2.0 + 0.4;
        p.line_segment([clock_point(c, inner, frac), clock_point(c, outer, frac)], Stroke::new(2.0, t.text_2));
    }
    let filled = ui.ctx().animate_value_with_time(id.with("grow"), value as f32, GROW_SECS);
    // A zero-length arc with a round cap still paints a dot, which reads as "a little
    // used" when the answer is none: nothing is drawn at 0.
    if filled > 0.0 {
        let tone = cswap_core::format::tone(Some(value));
        let col = match tone {
            Tone::Danger => t.danger,
            Tone::Warn => t.warn,
            _ => t.accent,
        };
        let frac = filled / 100.0;
        let n = ((frac * 64.0).ceil() as usize).max(2);
        let pts: Vec<Pos2> = (0..=n).map(|k| clock_point(c, RING_R, frac * k as f32 / n as f32)).collect();
        let (first, last) = (pts[0], pts[n]);
        p.add(Shape::line(pts, Stroke::new(RING_STROKE, col)));
        // round caps
        p.circle_filled(first, RING_STROKE / 2.0, col);
        p.circle_filled(last, RING_STROKE / 2.0, col);
    }
    let tx = rect.min.x + 4.0 + RING + 8.0;
    let tw = rect.max.x - tx - 2.0;
    let tone = cswap_core::format::tone(pct_value.map(|_| value));
    let head_h = 15.0;
    let total_h = if sub.is_some() { head_h + 13.0 } else { head_h };
    let mut y = rect.center().y - total_h / 2.0;
    let mut x = tx;
    if let Some(l) = label {
        let g = widgets::galley(ui, format!("{l} "), style::body(12.0), t.text_3, tw);
        let gw = g.size().x;
        p.galley(pos2(x, y), g, t.text_3);
        x += gw;
    }
    let g = widgets::galley(ui, text, style::body(12.0), pct_color(t, tone), (tx + tw - x).max(10.0));
    p.galley(pos2(x, y), g, t.text);
    if let Some(s) = sub {
        y += head_h;
        let g = widgets::galley(ui, s, style::body(11.0), t.text_3, tw);
        p.galley(pos2(tx, y), g, t.text_3);
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    // e2e "the pace marker sits where an even week would be": 50% → the middle.
    #[test]
    fn pace_marker_sits_at_its_percentage() {
        let track = Rect::from_min_size(pos2(100.0, 0.0), vec2(200.0, 5.0));
        assert_eq!(pace_x(track, 50.0), 200.0);
        assert_eq!(pace_x(track, 0.0), 100.0);
        assert_eq!(pace_x(track, 140.0), 300.0, "clamped to the track");
    }

    // e2e "the ring notch points at the pace angle, not a quarter turn early".
    #[test]
    fn ring_angles_start_at_twelve_and_run_clockwise() {
        let c = pos2(0.0, 0.0);
        let near = |a: Pos2, b: Pos2| (a - b).length() < 1e-4;
        assert!(near(clock_point(c, 10.0, 0.0), pos2(0.0, -10.0)), "twelve o'clock");
        assert!(near(clock_point(c, 10.0, 0.25), pos2(10.0, 0.0)), "three o'clock");
        assert!(near(clock_point(c, 10.0, 0.5), pos2(0.0, 10.0)), "half a turn points down");
    }
}
