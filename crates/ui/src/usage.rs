//! Usage bars, rings and the pace tick of the Accounts page (1.1.0 redesign). OWNER: ui-accounts.
//!
//! The pace tick (0.1.13–0.1.15, Purwa picked the tick out of four designs) marks where an
//! evenly spread week would be by now. Only weekly windows carry `expectedPct`, so a
//! 5-hour window never shows one. On a bar it stands proud of the track; on a ring it
//! crosses the stroke and is painted LAST, so a full arc cannot cover it.

use std::f32::consts::TAU;

use egui::{Color32, CornerRadius, Id, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, pos2, vec2};

use crate::style;
use crate::theme::Tokens;
use crate::widgets;
use cswap_core::format::{Tone, pct};

/// Grow animation of bars and rings (`duration-500 ease-out`).
const GROW_SECS: f32 = 0.5;
/// The mockup's `.track`: 6 px, radius 3.
const TRACK_H: f32 = 6.0;
/// The pace marker reaches this far above and below the bar's track.
const MARK_OVERHANG: f32 = 3.0;
/// Stroke of every ring, large (hero, 104 px) or small (tiles, 64 px).
const RING_STROKE: f32 = 7.0;

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

/// A 6 px track filled to `pct`, `width` wide, with the pace marker when `pace` is set.
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
        let r = Rect::from_min_max(
            pos2(x - 0.75, track.min.y - MARK_OVERHANG),
            pos2(x + 0.75, track.max.y + MARK_OVERHANG),
        );
        p.rect_filled(r, CornerRadius::ZERO, t.text);
    }
    resp
}

/// A `size` px ring filled to `pct` with the percentage in its middle (the hero's 104 px,
/// a tile's 64 px). Captions under it are the caller's. `None` draws an empty ring and `—`.
pub fn ring(ui: &mut Ui, t: &Tokens, id: Id, pct_value: Option<f64>, pace: Option<f64>, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    let r = size / 2.0 - 6.0;
    p.circle_stroke(c, r, Stroke::new(RING_STROKE, t.surface_3));
    let value = pct_value.map(|v| v.clamp(0.0, 100.0)).unwrap_or(0.0) as f32;
    let filled = ui.ctx().animate_value_with_time(id.with("grow"), value, GROW_SECS);
    let tn = cswap_core::format::tone(pct_value);
    // A zero-length arc with a round cap still paints a dot, which reads as "a little
    // used" when the answer is none: nothing is drawn at 0.
    if filled > 0.0 {
        let col = fill_color(t, tn);
        let frac = filled / 100.0;
        let n = ((frac * 96.0).ceil() as usize).max(2);
        let pts: Vec<Pos2> = (0..=n).map(|k| clock_point(c, r, frac * k as f32 / n as f32)).collect();
        let (first, last) = (pts[0], pts[n]);
        p.add(Shape::line(pts, Stroke::new(RING_STROKE, col)));
        p.circle_filled(first, RING_STROKE / 2.0, col);
        p.circle_filled(last, RING_STROKE / 2.0, col);
    }
    if let Some(m) = pace {
        let frac = m.clamp(0.0, 100.0) as f32 / 100.0;
        p.line_segment([clock_point(c, r - 7.0, frac), clock_point(c, r + 7.0, frac)], Stroke::new(1.5, t.text));
    }
    let text = pct(pct_value);
    let font = if size > 90.0 { style::body_semibold(20.0) } else { style::body_semibold(15.0) };
    let g = widgets::galley(ui, text, font, t.text, size - 14.0);
    ui.painter().galley(c - g.size() / 2.0, g, t.text);
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
