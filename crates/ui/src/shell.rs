//! Title bar and sidebar. Port of `components/{TitleBar,Sidebar}.tsx`. OWNER: ui-shell.
//!
//! The sidebar collapses to icons on its own under [`FORCE_NARROW_BELOW`] px of WINDOW width
//! (the room goes to the content; the icons carry the meaning), and by hand above it with
//! the Collapse button, persisted as the `sidebarCollapsed` setting.

use cswap_core::{AccountsState, AppSettings, AutoStatus, CswapBinaryInfo, Page, UpdaterState, UpdaterStatus};
use egui::{Align, Align2, Color32, CornerRadius, Frame, Layout, Margin, Rect, Sense, Stroke, Ui, pos2, vec2};

use crate::palette::mask_email;
use crate::style::{self, mix};
use crate::theme::{RADIUS_MD, Tokens};
use crate::widgets::{self, Side, icon};

/// Under this window width the sidebar is icons only and the Collapse button goes away.
pub const FORCE_NARROW_BELOW: f32 = 860.0;
pub const SIDEBAR_WIDE: f32 = 204.0;
pub const SIDEBAR_NARROW: f32 = 56.0;
pub const TITLEBAR_H: f32 = 38.0;

/// What the shell shows, borrowed from the app's cached state.
pub struct ShellView<'a> {
    pub page: Page,
    pub settings: &'a AppSettings,
    pub accounts: &'a AccountsState,
    pub auto: &'a AutoStatus,
    /// `None` while the first detection runs.
    pub binary: Option<&'a CswapBinaryInfo>,
    pub updater: &'a UpdaterState,
    pub mac: bool,
    /// Seconds since the Unix epoch, for "updated 3m ago".
    pub now_unix: f64,
}

/// What the user asked of the shell this frame.
#[derive(Debug, Default, PartialEq)]
pub struct ShellAction {
    pub navigate: Option<Page>,
    pub toggle_collapsed: bool,
    pub open_palette: bool,
    pub refresh: bool,
}

/// Whether the sidebar is narrow for this window width and setting.
pub fn sidebar_narrow(window_width: f32, collapsed_setting: bool) -> (bool, bool) {
    let forced = window_width < FORCE_NARROW_BELOW;
    (forced || collapsed_setting, forced)
}

const ITEMS: [(Page, &str, &str); 5] = [
    (Page::Accounts, "Accounts", icon::USERS),
    (Page::Auto, "Auto-switch", icon::ACTIVITY),
    (Page::Mappings, "Mappings", icon::FOLDER_TREE),
    (Page::Log, "Log", icon::SCROLL_TEXT),
    (Page::Settings, "Settings", icon::SETTINGS),
];

// ---- title bar ---------------------------------------------------------------------------

pub fn title_bar(ui: &mut Ui, view: &ShellView, act: &mut ShellAction) {
    let t = widgets::tokens(ui);
    let width = ui.ctx().content_rect().width();
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, mix(t.bg, t.surface, 0.7));
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, t.border));
    let inner = rect.shrink2(vec2(12.0, 0.0));
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(Layout::left_to_right(Align::Center)));
    ui.spacing_mut().item_spacing.x = 8.0;
    let (logo, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
    ui.painter().rect_filled(logo, CornerRadius::same(4), t.accent);
    widgets::text(&mut ui, "Claude Swap", style::body_semibold(12.5), t.text);

    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        // refresh, with the age of the list beside it
        let age = view.accounts.fetched_at.as_deref().and_then(parse_iso).map(|at| age_text(view.now_unix - at));
        let age_g = age.as_ref().map(|a| ui.painter().layout_job(widgets::job(a, style::body(11.0), t.text_3)));
        let w = 6.0 + 13.0 + age_g.as_ref().map_or(0.0, |g| g.size().x + 6.0) + 6.0;
        let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Refresh usage"));
        let ht = widgets::hover_t(ui, resp.id, resp.hovered());
        ui.painter().rect_filled(r, CornerRadius::same(RADIUS_MD), Color32::TRANSPARENT.lerp_to_gamma(t.surface_2, ht));
        let fg = mix(t.text_3, t.text, ht);
        let ic = pos2(r.left() + 6.0 + 6.5, r.center().y);
        if view.accounts.refreshing {
            widgets::paint_spinner(ui, ic, 13.0, fg);
        } else {
            widgets::paint_icon(ui, icon::REFRESH_CW, ic, 13.0, fg);
        }
        if let Some(g) = age_g {
            ui.painter().galley(pos2(r.left() + 25.0, r.center().y - g.size().y / 2.0), g, fg);
        }
        let tip = match &age {
            Some(a) => format!("Refresh usage · updated {a}"),
            None => "Refresh usage".into(),
        };
        widgets::tooltip(ui, &resp, &tip, Side::Top);
        if resp.clicked() {
            act.refresh = true;
        }

        // active account pill (md and up)
        if width >= 768.0
            && let Some(a) = view.accounts.payload.as_ref().and_then(|p| p.accounts.iter().find(|a| a.active))
        {
            let name = a.alias.clone().unwrap_or_else(|| mask_email(&a.email, view.settings.mask_emails));
            let g = ui.painter().layout_job(widgets::job(&name, style::body_medium(12.0), t.accent));
            let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 8.0 + 6.0 + 8.0 + 2.0, 22.0), Sense::hover());
            widgets::name(ui, &resp, &format!("Active: {name}"));
            ui.painter().rect_filled(r, CornerRadius::same(RADIUS_MD), t.accent_soft);
            ui.painter().circle_filled(pos2(r.left() + 11.0, r.center().y), 3.0, t.accent);
            ui.painter().galley(pos2(r.left() + 22.0, r.center().y - g.size().y / 2.0), g, t.accent);
        }

        // palette trigger
        let label = "Switch account…";
        let show_label = width >= 640.0;
        let g = ui.painter().layout_job(widgets::job(label, style::body(12.0), t.text_2));
        let kbd_text = if view.mac { "⌘ K" } else { "Ctrl K" };
        let kg = ui.painter().layout_job(widgets::job(kbd_text, style::mono(10.5), t.text_2));
        let w = 8.0 + 13.0 + 8.0 + if show_label { g.size().x + 8.0 } else { 0.0 } + kg.size().x + 8.0 + 8.0;
        let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        let ht = widgets::hover_t(ui, resp.id, resp.hovered());
        ui.painter().rect(
            r,
            CornerRadius::same(RADIUS_MD),
            t.bg,
            Stroke::new(1.0, mix(t.border, t.border_strong, ht)),
            egui::StrokeKind::Inside,
        );
        let fg = mix(t.text_2, t.text, ht);
        widgets::paint_icon(ui, icon::SEARCH, pos2(r.left() + 8.0 + 6.5, r.center().y), 13.0, fg);
        let mut x = r.left() + 29.0;
        if show_label {
            ui.painter().galley(pos2(x, r.center().y - g.size().y / 2.0), g, fg);
            x += ui.painter().layout_job(widgets::job(label, style::body(12.0), fg)).size().x + 8.0;
        }
        let kr = Rect::from_min_size(pos2(x, r.center().y - 9.0), vec2(kg.size().x + 8.0, 18.0));
        ui.painter().rect(kr, CornerRadius::same(4), t.surface_2, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
        ui.painter().galley(kr.center() - kg.size() / 2.0, kg, t.text_2);
        if resp.clicked() {
            act.open_palette = true;
        }
    });
}

// ---- sidebar -----------------------------------------------------------------------------

/// Draws the sidebar into `ui` (a left panel of the current animated width).
pub fn sidebar(ui: &mut Ui, view: &ShellView, narrow: bool, forced: bool, act: &mut ShellAction) {
    let t = widgets::tokens(ui);
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, mix(t.bg, t.surface, 0.4));
    ui.painter().vline(rect.right() - 0.5, rect.y_range(), Stroke::new(1.0, t.border));
    let pad = if narrow { 8.0 } else { 12.0 };
    let inner = Rect::from_min_max(rect.min + vec2(pad, 12.0), rect.max - vec2(pad + 1.0, 12.0));
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(Layout::top_down(Align::Min)));
    ui.set_clip_rect(rect);
    ui.spacing_mut().item_spacing.y = 2.0;
    let count = view.accounts.payload.as_ref().map(|p| p.accounts.len());
    let ready = view.updater.status == UpdaterStatus::Downloaded;
    for (page, label, glyph) in ITEMS {
        let on = view.page == page;
        let w = if narrow { 40.0 } else { inner.width() };
        let (r, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, label));
        let ht = widgets::hover_t(&ui, resp.id, resp.hovered() && !on);
        let fill = if on { t.surface_2 } else { Color32::TRANSPARENT.lerp_to_gamma(t.surface_2.gamma_multiply(0.7), ht) };
        ui.painter().rect_filled(r, CornerRadius::same(RADIUS_MD), fill);
        let ic = if narrow { r.center() } else { pos2(r.left() + 10.0 + 7.5, r.center().y) };
        widgets::paint_icon(&ui, glyph, ic, 15.0, if on { t.accent } else { t.text_3 });
        let dot = |ui: &Ui, at: egui::Pos2, c: Color32| ui.painter().circle_filled(at, 3.0, c);
        let badge_dot = match page {
            Page::Auto if view.auto.running => Some(t.ok),
            Page::Settings if ready => Some(t.accent),
            _ => None,
        };
        if narrow {
            if let Some(c) = badge_dot {
                dot(&ui, ic + vec2(8.0, -8.0), c);
            }
            widgets::tooltip(&ui, &resp, label, Side::Right);
        } else {
            let font = if on { style::body_medium(13.0) } else { style::body(13.0) };
            let fg = if on { t.text } else { mix(t.text_2, t.text, ht) };
            ui.painter().text(pos2(r.left() + 35.0, r.center().y), Align2::LEFT_CENTER, label, font, fg);
            if page == Page::Accounts
                && let Some(n) = count
            {
                ui.painter().text(pos2(r.right() - 10.0, r.center().y), Align2::RIGHT_CENTER, n.to_string(), style::body(11.0), t.text_3);
            }
            if let Some(c) = badge_dot {
                dot(&ui, pos2(r.right() - 13.0, r.center().y), c);
                let tip = if page == Page::Auto {
                    "Auto-switch running".to_owned()
                } else {
                    format!("Update {} ready", view.updater.version.as_deref().unwrap_or(""))
                };
                let dr = Rect::from_center_size(pos2(r.right() - 13.0, r.center().y), vec2(10.0, 10.0));
                if ui.rect_contains_pointer(dr) {
                    widgets::tooltip(&ui, &resp, &tip, Side::Top);
                }
            }
        }
        if resp.clicked() {
            act.navigate = Some(page);
        }
    }

    // bottom: collapse toggle (unless forced) and the cswap status line
    let bottom_h = if forced { 22.0 } else { 22.0 + 28.0 + 4.0 };
    let space = ui.available_height() - bottom_h;
    if space > 0.0 {
        ui.add_space(space);
    }
    if !forced {
        let label = if narrow { "Expand the sidebar" } else { "Collapse the sidebar" };
        let w = if narrow { 40.0 } else { inner.width() };
        let (r, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        let ht = widgets::hover_t(&ui, resp.id, resp.hovered());
        ui.painter().rect_filled(r, CornerRadius::same(RADIUS_MD), Color32::TRANSPARENT.lerp_to_gamma(t.surface_2.gamma_multiply(0.7), ht));
        let fg = mix(t.text_3, t.text, ht);
        let glyph = if narrow { icon::PANEL_LEFT_OPEN } else { icon::PANEL_LEFT_CLOSE };
        let ic = if narrow { r.center() } else { pos2(r.left() + 10.0 + 7.5, r.center().y) };
        widgets::paint_icon(&ui, glyph, ic, 15.0, fg);
        if !narrow {
            ui.painter().text(pos2(r.left() + 35.0, r.center().y), Align2::LEFT_CENTER, "Collapse", style::body(12.0), fg);
        }
        widgets::tooltip(&ui, &resp, label, Side::Right);
        if resp.clicked() {
            act.toggle_collapsed = true;
        }
        ui.add_space(4.0);
    }
    status_line(&mut ui, view, narrow, &t);
}

fn status_line(ui: &mut Ui, view: &ShellView, narrow: bool, t: &Tokens) {
    let (text, color, dot, tip) = match view.binary {
        None => ("detecting cswap…".to_owned(), t.text_3, t.border_strong, "detecting cswap…".to_owned()),
        Some(b) => match &b.version {
            Some(v) => (format!("cswap {v}"), t.text_3, t.ok, format!("cswap {v}")),
            None => ("cswap not found".to_owned(), t.danger, t.danger, "cswap not found".to_owned()),
        },
    };
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 18.0), Sense::hover());
    widgets::name(ui, &resp, &text);
    let x = if narrow { r.left() + 17.0 } else { r.left() + 10.0 };
    if view.binary.is_some() || narrow {
        ui.painter().circle_filled(pos2(x + 3.0, r.center().y), 3.0, dot);
    }
    if !narrow {
        let tx = if view.binary.is_some() { x + 12.0 } else { x };
        ui.painter().text(pos2(tx, r.center().y), Align2::LEFT_CENTER, &text, style::body(11.0), color);
    } else {
        widgets::tooltip(ui, &resp, &tip, Side::Right);
    }
}

/// Lays out a frame that fills `rect` with no margin, for painting chrome into.
pub fn chrome_frame() -> Frame {
    Frame::new().inner_margin(Margin::ZERO)
}

// ---- time ----------------------------------------------------------------------------------

/// Seconds since the epoch for an RFC 3339 / ISO 8601 UTC-or-offset timestamp, the way cswap
/// and the driver write them (`2026-09-10T08:00:00.000Z`, `...+07:00`).
pub fn parse_iso(s: &str) -> Option<f64> {
    let s = s.trim();
    let (date, rest) = s.split_once(['T', ' '])?;
    let mut d = date.split('-');
    let (y, mo, da): (i64, i64, i64) = (d.next()?.parse().ok()?, d.next()?.parse().ok()?, d.next()?.parse().ok()?);
    let tz_at = rest.find(['Z', 'z', '+', '-']).unwrap_or(rest.len());
    let (time, tz) = rest.split_at(tz_at);
    let mut tp = time.split(':');
    let h: i64 = tp.next()?.parse().ok()?;
    let mi: i64 = tp.next()?.parse().ok()?;
    let sec: f64 = tp.next().unwrap_or("0").parse().ok()?;
    let offset = match tz {
        "" | "Z" | "z" => 0,
        o => {
            let sign = if o.starts_with('-') { -1 } else { 1 };
            let o = &o[1..];
            let (oh, om) = o.split_once(':').unwrap_or((&o[..2.min(o.len())], &o[2.min(o.len())..]));
            sign * (oh.parse::<i64>().ok()? * 3600 + om.parse::<i64>().unwrap_or(0) * 60)
        }
    };
    // days from civil (Howard Hinnant)
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (mo + if mo > 2 { -3 } else { 9 }) + 2) / 5 + da - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some((days * 86400 + h * 3600 + mi * 60 - offset) as f64 + sec)
}

/// `ageSeconds` of format.ts: "just now", "3m ago", "2h ago", "4d ago".
pub fn age_text(sec: f64) -> String {
    let s = sec.max(0.0) as u64;
    if s < 60 {
        "just now".into()
    } else if s < 3600 {
        format!("{}m ago", s / 60)
    } else if s < 86400 {
        format!("{}h ago", s / 3600)
    } else {
        format!("{}d ago", s / 86400)
    }
}

pub fn now_unix() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_timestamps_parse_with_offsets() {
        assert_eq!(parse_iso("1970-01-01T00:00:00Z"), Some(0.0));
        assert_eq!(parse_iso("2026-09-10T08:00:00.500Z"), Some(1_789_027_200.5));
        assert_eq!(parse_iso("2026-09-10T15:00:00+07:00"), Some(1_789_027_200.0));
        assert_eq!(parse_iso("2026-09-10T03:00:00-05:00"), Some(1_789_027_200.0));
        assert_eq!(parse_iso("not a date"), None);
    }

    // The sidebar rule Purwa asked for: forced under 860 px of WINDOW, a choice above it.
    #[test]
    fn sidebar_forced_under_860_and_optional_above() {
        assert_eq!(sidebar_narrow(859.0, false), (true, true));
        assert_eq!(sidebar_narrow(860.0, false), (false, false));
        assert_eq!(sidebar_narrow(1240.0, true), (true, false));
    }
}
