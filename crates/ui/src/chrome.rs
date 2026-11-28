//! The window's own title bar: the app header IS the title bar, as 0.1.23's
//! `titleBarStyle: 'hidden'` + `titleBarOverlay` (one 38 px bar, no OS title bar above it).
//! OWNER: ui-shell. Structure after craft-launcher's `widgets::{title_bar_drag,
//! window_buttons, window_edges}` (MIT/Apache-2.0).
//!
//! - [`WindowChrome::Custom`] (Windows, Linux): the window is undecorated, so this module moves
//!   it (drag the header's empty space, double-click to maximize), draws minimize / maximize /
//!   close at the right and resizes it from its edges ([`window_edges`]).
//! - [`WindowChrome::MacOs`]: the title bar is transparent over the content; the traffic lights
//!   stay native and the header leaves room for them. Moving is still ours (the content view
//!   takes the clicks there).
//! - [`WindowChrome::Native`]: the OS draws its own bar (tests, the demo).

use egui::{Color32, CornerRadius, Rect, Response, Sense, Stroke, Ui, ViewportCommand, vec2};

use crate::widgets::{self, Side};

/// Who draws the window's title bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowChrome {
    #[default]
    Native,
    Custom,
    MacOs,
}

/// Width of one caption button, as Windows' own (and Electron's overlay, 3 x 46 = 138).
pub const CAPTION_W: f32 = 46.0;
/// Room the macOS traffic lights take at the header's left (0.1.23: `pl-[76px]`).
pub const TRAFFIC_LIGHTS_W: f32 = 76.0;
/// How far in from the window's edges a press resizes it ([`window_edges`]). The header's own
/// controls leave this band alone, or the top edge could never be grabbed.
pub const RESIZE_EDGE: f32 = 5.0;
/// Windows 11's own close-button hover red.
const CLOSE_RED: Color32 = Color32::from_rgb(0xc4, 0x2b, 0x1c);

fn maximized(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false))
}

/// The resize band right now: none while maximized or full screen (the edges are the
/// screen's then), so the close button reaches the corner as Windows' own does.
fn resize_band(ctx: &egui::Context) -> f32 {
    if maximized(ctx) { 0.0 } else { RESIZE_EDGE }
}

/// Dragging `bar`'s empty space moves the window, double-clicking it maximizes or restores.
/// Call BEFORE the header's controls: egui gives a press to the last widget that senses it, so
/// buttons drawn afterwards keep their clicks and text and badges (hover only) stay draggable.
pub fn title_bar_drag(ui: &mut Ui, bar: Rect) {
    let band = resize_band(ui.ctx());
    let rect = Rect::from_min_max(bar.min + vec2(band, band), bar.max - vec2(band, 0.0));
    let resp = ui.interact(rect, ui.scope_id().with("title-bar-drag"), Sense::click_and_drag());
    // Each press moves the window at most once (`sent`); see `held_move` for why that needs
    // remembering at all.
    let pressed = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::PointerButton { pressed: true, .. })));
    let sent = egui::Id::unique("cswap-window-move-sent");
    if pressed || !ui.input(|i| i.pointer.primary_down()) {
        ui.ctx().data_mut(|d| d.remove::<bool>(sent));
    }
    let start = resp.drag_started_by(egui::PointerButton::Primary) || held_move(ui, &resp);
    if start && !ui.ctx().data_mut(|d| d.get_temp::<bool>(sent).unwrap_or(false)) {
        ui.ctx().data_mut(|d| d.insert_temp(sent, true));
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        let_the_button_go(ui.ctx());
    }
    if resp.double_clicked() {
        let m = maximized(ui.ctx());
        ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!m));
    }
}

/// Linux and the BSDs: the window system takes the pointer for a move and never reports the
/// release that ends it (rust-windowing/winit#2999, emilk/egui#7959). egui then thinks the
/// button is still down and the NEXT press on the bar is no press, so every second drag would
/// do nothing. A press that began on the bar, is held, has moved and is decidedly a drag is
/// taken as that next attempt (craft-launcher #1358).
#[cfg(all(unix, not(target_os = "macos")))]
fn held_move(ui: &Ui, resp: &Response) -> bool {
    let began = egui::Id::unique("cswap-window-move-began");
    let press = ui.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, .. } => Some(*pos),
            _ => None,
        })
    });
    let (down, moved, decidedly) = ui.input(|i| {
        (i.pointer.primary_down(), i.pointer.delta() != egui::Vec2::ZERO, i.pointer.is_decidedly_dragging())
    });
    if press.is_some() || !down {
        ui.ctx().data_mut(|d| {
            if press.is_some_and(|p| resp.rect.contains(p)) {
                d.insert_temp(began, true);
            } else {
                d.remove::<bool>(began);
            }
        });
    }
    let began = ui.ctx().data_mut(|d| d.get_temp::<bool>(began).unwrap_or(false));
    began && press.is_none() && down && moved && decidedly && resp.contains_pointer()
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn held_move(_: &Ui, _: &Response) -> bool {
    false
}

/// Linux and the BSDs (see [`held_move`]): let go of the pointer the moment a move or resize
/// starts, as though the button had gone up, and drop the drag owner, or egui keeps the bar as
/// "the dragged widget" and the next press never starts a drag. Elsewhere the release arrives.
#[cfg(all(unix, not(target_os = "macos")))]
fn let_the_button_go(ctx: &egui::Context) {
    ctx.input_mut(|i| i.pointer = egui::PointerState::default());
    ctx.stop_dragging();
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn let_the_button_go(_: &egui::Context) {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Caption {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl Caption {
    fn label(self) -> &'static str {
        match self {
            Caption::Minimize => "Minimize",
            Caption::Maximize => "Maximize",
            Caption::Restore => "Restore",
            Caption::Close => "Close",
        }
    }
}

/// Minimize, maximize (restore) and close across `rect` (3 x [`CAPTION_W`], the header's full
/// height), styled as 0.1.23's overlay: grey symbols, a soft fill on hover, red under close.
/// Close sends the same `Close` as the OS button did, so close-to-tray decides what it does.
pub fn window_buttons(ui: &mut Ui, rect: Rect) {
    let ctx = ui.ctx().clone();
    let max = maximized(&ctx);
    let size = if max { Caption::Restore } else { Caption::Maximize };
    // A click that lands in the resize band (the frame after a restore still hit-tests the
    // maximized layout) is the edge's, not the button's.
    let inside = ctx.content_rect().shrink(resize_band(&ctx));
    let clicked = |r: &Response| r.clicked() && r.interact_pointer_pos().is_none_or(|p| inside.contains(p));
    for (i, b) in [Caption::Minimize, size, Caption::Close].into_iter().enumerate() {
        let r = Rect::from_min_size(rect.min + vec2(i as f32 * CAPTION_W, 0.0), vec2(CAPTION_W, rect.height()));
        let resp = caption_button(ui, b, r);
        if clicked(&resp) {
            ctx.send_viewport_cmd(match b {
                Caption::Minimize => ViewportCommand::Minimized(true),
                Caption::Maximize | Caption::Restore => ViewportCommand::Maximized(!max),
                Caption::Close => ViewportCommand::Close,
            });
        }
    }
}

fn caption_button(ui: &mut Ui, b: Caption, rect: Rect) -> Response {
    let t = widgets::tokens(ui);
    // Clickable below the top resize band (and, for close, left of the right one).
    let band = resize_band(ui.ctx());
    let right = if b == Caption::Close { band } else { 0.0 };
    let hit = Rect::from_min_max(rect.min + vec2(0.0, band), rect.max - vec2(right, 0.0));
    // Restore and Maximize are one button: one id, so hover survives the swap.
    let id = ui.scope_id().with(("caption", if b == Caption::Restore { Caption::Maximize } else { b }));
    let resp = ui.interact(hit, id, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, b.label()));
    let ht = widgets::hover_t(ui, resp.id, resp.hovered());
    let down = resp.is_pointer_button_down_on();
    let (fill, ink) = if b == Caption::Close {
        let red = if down { CLOSE_RED.gamma_multiply(0.85) } else { CLOSE_RED };
        (Color32::TRANSPARENT.lerp_to_gamma(red, ht), crate::style::mix(t.text_3, Color32::WHITE, ht))
    } else {
        let f = if down { t.border } else { t.surface_2 };
        (Color32::TRANSPARENT.lerp_to_gamma(f, ht), crate::style::mix(t.text_3, t.text, ht))
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::ZERO, fill);
    // 10 px glyphs drawn as one-point lines, like Windows' own caption buttons, centred on a
    // pixel so the lines stay sharp.
    let ppp = ui.ctx().pixels_per_point();
    let snap = |v: f32| ((v * ppp).floor() + 0.5) / ppp;
    let c = egui::pos2(snap(rect.center().x), snap(rect.center().y));
    let s = 5.0;
    let stroke = Stroke::new(1.0, ink);
    match b {
        Caption::Minimize => {
            p.hline(c.x - s..=c.x + s, c.y, stroke);
        }
        Caption::Maximize => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(2.0 * s, 2.0 * s)),
                CornerRadius::ZERO,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        Caption::Restore => {
            // a window in front of another
            let front = Rect::from_min_size(c + vec2(-s, -s + 2.0), vec2(2.0 * s - 2.0, 2.0 * s - 2.0));
            p.rect_stroke(front, CornerRadius::ZERO, stroke, egui::StrokeKind::Inside);
            p.hline(c.x - s + 2.0..=c.x + s, c.y - s, stroke);
            p.vline(c.x + s, c.y - s..=c.y + s - 2.0, stroke);
        }
        Caption::Close => {
            p.line_segment([c + vec2(-s, -s), c + vec2(s, s)], stroke);
            p.line_segment([c + vec2(-s, s), c + vec2(s, -s)], stroke);
        }
    }
    widgets::tooltip(ui, &resp, b.label(), Side::Bottom);
    resp
}

/// Which edge or corner a pointer `RESIZE_EDGE` in from the window's border grabs. A corner
/// takes a little of each edge next to it (3 x the band), or it would be a 5 px square.
pub fn edge_at(window: Rect, p: egui::Pos2) -> Option<egui::viewport::ResizeDirection> {
    use egui::viewport::ResizeDirection as D;
    if !window.contains(p) {
        return None;
    }
    let near = |d: f32| d < RESIZE_EDGE;
    let corner = |d: f32| d < 3.0 * RESIZE_EDGE;
    let (l, r, t, b) = (p.x - window.left(), window.right() - p.x, p.y - window.top(), window.bottom() - p.y);
    Some(if (near(t) && corner(l)) || (near(l) && corner(t)) {
        D::NorthWest
    } else if (near(t) && corner(r)) || (near(r) && corner(t)) {
        D::NorthEast
    } else if (near(b) && corner(l)) || (near(l) && corner(b)) {
        D::SouthWest
    } else if (near(b) && corner(r)) || (near(r) && corner(b)) {
        D::SouthEast
    } else if near(t) {
        D::North
    } else if near(b) {
        D::South
    } else if near(l) {
        D::West
    } else if near(r) {
        D::East
    } else {
        return None;
    })
}

/// Undecorated windows (Windows, Linux): resizing from the edges and corners, which the OS no
/// longer offers once its frame is gone (on Windows winit makes the whole window client area),
/// and a hairline round the window so it does not melt into what is behind it. Call last in
/// the frame: the bands sit above every layer, so nothing near the edge takes the press.
pub fn window_edges(ctx: &egui::Context) {
    use egui::CursorIcon as C;
    use egui::viewport::ResizeDirection as D;
    if maximized(ctx) {
        return;
    }
    let t = crate::style::tokens(ctx);
    let r = ctx.content_rect();
    ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("cswap-window-border")))
        .rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, t.border_strong), egui::StrokeKind::Inside);
    let e = RESIZE_EDGE;
    let bands = [
        r.with_max_y(r.top() + e),
        r.with_min_y(r.bottom() - e),
        r.with_max_x(r.left() + e),
        r.with_min_x(r.right() - e),
    ];
    for (i, band) in bands.into_iter().enumerate() {
        egui::Area::new(egui::Id::unique(("cswap-window-edge", i)))
            .order(egui::Order::Tooltip)
            .movable(false)
            .constrain(false)
            .fixed_pos(band.min)
            .show(ctx, |ui| ui.allocate_response(band.size(), Sense::CLICK));
    }
    let Some(dir) = ctx.input(|i| i.pointer.hover_pos()).and_then(|p| edge_at(r, p)) else { return };
    ctx.set_cursor_icon(match dir {
        D::North => C::ResizeNorth,
        D::South => C::ResizeSouth,
        D::West => C::ResizeWest,
        D::East => C::ResizeEast,
        D::NorthWest => C::ResizeNorthWest,
        D::NorthEast => C::ResizeNorthEast,
        D::SouthWest => C::ResizeSouthWest,
        D::SouthEast => C::ResizeSouthEast,
    });
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
        let_the_button_go(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::viewport::ResizeDirection as D;
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, pos2};

    const SCREEN: Rect = Rect { min: Pos2::ZERO, max: Pos2 { x: 800.0, y: 600.0 } };

    fn ctx() -> egui::Context {
        let ctx = egui::Context::default();
        crate::style::setup(&ctx, cswap_core::ThemeSetting::Light);
        ctx.run_ui(RawInput { screen_rect: Some(SCREEN), ..Default::default() }, |_| {}).drop_without_applying_deltas();
        ctx
    }

    fn press(at: Pos2) -> Event {
        Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }
    }

    fn release(at: Pos2) -> Event {
        Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }
    }

    /// One frame of a 38 px title bar (drag + caption buttons) and the edges; the commands sent.
    fn frame(ctx: &egui::Context, events: Vec<Event>) -> Vec<ViewportCommand> {
        let input = RawInput { screen_rect: Some(SCREEN), events, ..Default::default() };
        let out = ctx.run_ui(input, |ui| {
            let bar = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 38.0));
            title_bar_drag(ui, bar);
            window_buttons(ui, Rect::from_min_max(pos2(800.0 - 3.0 * CAPTION_W, 0.0), pos2(800.0, 38.0)));
            window_edges(ui.ctx());
        });
        let cmds = out.viewport_output.values().flat_map(|v| v.commands.iter().cloned()).collect();
        out.drop_without_applying_deltas();
        cmds
    }

    // The header's empty space must move the window, or with the OS title bar gone it
    // cannot be moved at all.
    #[test]
    fn dragging_the_header_moves_the_window_and_double_click_maximizes() {
        let c = ctx();
        // egui hit-tests against the previous frame's widgets: lay the bar out first.
        frame(&c, vec![Event::PointerMoved(pos2(300.0, 20.0))]);
        frame(&c, vec![press(pos2(300.0, 20.0))]);
        let cmds = frame(&c, vec![Event::PointerMoved(pos2(340.0, 24.0))]);
        assert!(cmds.contains(&ViewportCommand::StartDrag), "{cmds:?}");
        frame(&c, vec![release(pos2(340.0, 24.0))]);

        let c = ctx();
        let at = pos2(300.0, 20.0);
        frame(&c, vec![Event::PointerMoved(at)]);
        frame(&c, vec![press(at), release(at)]);
        let cmds = frame(&c, vec![press(at), release(at)]);
        assert!(cmds.contains(&ViewportCommand::Maximized(true)), "{cmds:?}");
        assert!(!cmds.contains(&ViewportCommand::StartDrag), "a click is not a move: {cmds:?}");
    }

    // Close must go through the same Close as the OS button (close-to-tray decides), and the
    // others must reach the window manager.
    #[test]
    fn caption_buttons_send_their_commands() {
        let click = |x: f32| {
            let c = ctx();
            let at = pos2(x, 20.0);
            frame(&c, vec![Event::PointerMoved(at)]);
            frame(&c, vec![press(at)]);
            frame(&c, vec![release(at)])
        };
        let right = 800.0 - 3.0 * CAPTION_W;
        assert!(click(right + 20.0).contains(&ViewportCommand::Minimized(true)));
        assert!(click(right + CAPTION_W + 20.0).contains(&ViewportCommand::Maximized(true)));
        let close = click(right + 2.0 * CAPTION_W + 20.0);
        assert!(close.contains(&ViewportCommand::Close), "{close:?}");
        assert!(!close.contains(&ViewportCommand::StartDrag));
    }

    // The top band belongs to the resize, not to the header under it: without that the top
    // edge (and the top-right corner, under close) could never be grabbed.
    #[test]
    fn edges_and_corners_resize_even_over_the_header() {
        let c = ctx();
        let at = pos2(400.0, 2.0);
        frame(&c, vec![Event::PointerMoved(at)]);
        let cmds = frame(&c, vec![press(at)]);
        assert!(cmds.contains(&ViewportCommand::BeginResize(D::North)), "{cmds:?}");
        assert!(!cmds.contains(&ViewportCommand::StartDrag));

        assert_eq!(edge_at(SCREEN, pos2(798.0, 2.0)), Some(D::NorthEast));
        assert_eq!(edge_at(SCREEN, pos2(790.0, 2.0)), Some(D::NorthEast), "a corner is wider than the band");
        assert_eq!(edge_at(SCREEN, pos2(2.0, 300.0)), Some(D::West));
        assert_eq!(edge_at(SCREEN, pos2(400.0, 598.0)), Some(D::South));
        assert_eq!(edge_at(SCREEN, pos2(2.0, 598.0)), Some(D::SouthWest));
        assert_eq!(edge_at(SCREEN, pos2(400.0, 300.0)), None);
    }
}
