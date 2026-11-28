//! The Accounts page, 1.1.0 redesign (agreed with Purwa 28 Nov 2026, mockup variant A
//! "Hero + list"). OWNER: ui-accounts.
//!
//! - a hero card for the active account (rings, per-model bars, forecast, status), then a
//!   Best next strip naming the account `cswap switch --strategy best` would pick;
//! - the other accounts as rows (mini bars; click a row for its per-model windows) or as
//!   tiles (small rings), sorted by readiness ([`cswap_core::ready`]);
//! - the shape follows the page's CONTENT width ([`view::page_shape`]), never the window's,
//!   and nothing scrolls sideways. Chips wrap only when the name line has no room left.
//!
//! 1.0's table (generated model columns, the 780–956 px horizontal-scroll band) is gone.

use std::collections::HashSet;

use egui::{
    Align, Align2, Color32, CornerRadius, Id, Layout, Margin, Rect, Sense, Shape, Stroke, Ui, UiBuilder, pos2, vec2,
};

use cswap_core::format::{
    age_seconds, iso_from_ms, mask_email, now_ms, org_tag, parse_iso_ms, pct, reset_text, status_label, status_tone,
    tone,
};
use cswap_core::ready::{self, Readiness, ReadyInfo};
use cswap_core::view::{self, ChipTone, PageShape};
use cswap_core::{
    Account, AccountsLayout, AccountsState, AddAccountOptions, AddTokenOptions, AppSettings, ConfigPayload, CsResult,
    Event, ExportOptions, ImportOptions, LaunchedSession, Mapping, Pending, PlainOutput, PlanInfo, RotateStrategy,
    SwitchPayload, Transfer, UsageWindow,
};

use crate::cx::Cx;
use crate::style;
use crate::theme::{TRANSITION_SECS, Tokens};
use crate::toast::ToastKind;
use crate::usage;
use crate::widgets::{self, Button, Variant, icon, item, item_hint};

/// Local UTC offset in seconds, for reset clocks.
pub(crate) fn local_offset() -> i64 {
    chrono::Local::now().offset().local_minus_utc() as i64
}

/// A menu entry with its 0.1.23 icon.
fn with<'a>(it: Option<widgets::MenuItem<'a>>, glyph: &'a str) -> Option<widgets::MenuItem<'a>> {
    it.map(|i| i.with_icon(glyph))
}

/// A toast with 0.1.23's (title, body) shape.
pub(crate) fn notify(cx: &mut Cx, kind: ToastKind, title: &str, body: Option<&str>) {
    match body.filter(|b| !b.is_empty()) {
        Some(b) => cx.toasts.push(kind, format!("{title}\n{b}")),
        None => cx.toasts.push(kind, title),
    }
}

enum OpKind {
    Switch(Pending<CsResult<SwitchPayload>>),
    Plain(Pending<CsResult<PlainOutput>>, String),
}

struct Op {
    key: String,
    kind: OpKind,
}

enum Dlg {
    Add {
        slot: String,
        alias: String,
        overwrite: bool,
        run: Option<Pending<CsResult<PlainOutput>>>,
    },
    Token {
        token: String,
        email: String,
        slot: String,
        show: bool,
        overwrite: bool,
        run: Option<Pending<CsResult<PlainOutput>>>,
    },
    Alias {
        a: Account,
        alias: String,
        run: Option<(Pending<CsResult<PlainOutput>>, String)>,
    },
    Move {
        a: Account,
        slot: String,
        run: Option<(Pending<CsResult<PlainOutput>>, String)>,
    },
    Remove {
        a: Account,
        typed: String,
        run: Option<Pending<CsResult<PlainOutput>>>,
    },
    Export {
        a: Option<Account>,
        full: bool,
        run: Option<Pending<CsResult<Transfer>>>,
    },
    Import {
        force: bool,
        run: Option<Pending<CsResult<Transfer>>>,
    },
    Diag {
        run: Pending<CsResult<PlainOutput>>,
        out: Option<Result<String, String>>,
    },
    Run {
        a: Account,
        dir: String,
        mappings: Vec<Mapping>,
        load: Option<Pending<CsResult<Vec<Mapping>>>>,
        run: Option<Pending<CsResult<LaunchedSession>>>,
    },
}

enum RowAct {
    Switch,
    Menu(usize),
    Toggle,
}

/// How often the page re-reads `autoswitch.model` (one `cswap config --json`): the Best
/// next pick folds those models in, as cswap's own `switch --strategy best` does.
const CONFIG_EVERY_SECS: f64 = 600.0;

pub struct AccountsPage {
    /// Fixed clock (Unix ms) for tests and screenshots; the wall clock otherwise.
    pub now_ms: Option<i64>,
    /// Fixed UTC offset (seconds) for tests; the OS's otherwise.
    pub utc_offset: Option<i64>,
    ops: Vec<Op>,
    dlg: Option<Dlg>,
    /// Rows opened to show their per-model windows.
    expanded: HashSet<u32>,
    /// `autoswitch.model`, parsed as cswap parses it.
    models: Vec<String>,
    config: Option<Pending<CsResult<ConfigPayload>>>,
    config_at: Option<f64>,
    /// What the last frame laid out, for tests: content width, its shape, the list's
    /// order (slots) and the Best next pick.
    pub last_content_width: f32,
    pub last_shape: PageShape,
    pub last_order: Vec<u32>,
    pub last_best: Option<u32>,
}

impl Default for AccountsPage {
    fn default() -> Self {
        Self::new()
    }
}

/// One chip of a name line: text, tone, hover text.
struct ChipSpec {
    label: String,
    tone: ChipTone,
    tip: Option<String>,
}

impl AccountsPage {
    pub fn new() -> Self {
        Self {
            now_ms: None,
            utc_offset: None,
            ops: Vec::new(),
            dlg: None,
            expanded: HashSet::new(),
            models: Vec::new(),
            config: None,
            config_at: None,
            last_content_width: 0.0,
            last_shape: PageShape::Wide,
            last_order: Vec::new(),
            last_best: None,
        }
    }

    pub fn on_event(&mut self, _ev: &Event) {}

    fn now(&self) -> i64 {
        self.now_ms.unwrap_or_else(now_ms)
    }

    fn offset(&self) -> i64 {
        self.utc_offset.unwrap_or_else(local_offset)
    }

    fn busy(&self, key: &str) -> bool {
        self.ops.iter().any(|o| o.key == key)
    }

    fn poll(&mut self, cx: &mut Cx) {
        let mut i = 0;
        while i < self.ops.len() {
            let done = match &mut self.ops[i].kind {
                OpKind::Switch(p) => match p.take() {
                    Some(Ok(v)) => {
                        if !v.warnings.is_empty() {
                            notify(cx, ToastKind::Info, "Switch warnings", Some(&v.warnings.join("\n")));
                        }
                        notify(cx, ToastKind::Success, &v.message, None);
                        true
                    }
                    Some(Err(e)) => {
                        notify(cx, ToastKind::Error, "cswap refused", Some(&e.message));
                        true
                    }
                    None => p.is_lost(),
                },
                OpKind::Plain(p, ok) => match p.take() {
                    Some(Ok(_)) => {
                        let ok = ok.clone();
                        notify(cx, ToastKind::Success, &ok, None);
                        true
                    }
                    Some(Err(e)) => {
                        notify(cx, ToastKind::Error, "cswap refused", Some(&e.message));
                        true
                    }
                    None => p.is_lost(),
                },
            };
            if done {
                self.ops.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Keeps `autoswitch.model` fresh enough for the Best next pick. A failed read leaves
    /// the last known models (none at first: 5-hour and 7-day only, cswap's own default).
    fn poll_config(&mut self, ui: &Ui, cx: &Cx) {
        let now = ui.input(|i| i.time);
        if self.config.is_none() && self.config_at.is_none_or(|t| now - t >= CONFIG_EVERY_SECS) {
            self.config = Some(cx.services.cswap.config());
            self.config_at = Some(now);
        }
        if let Some(p) = &mut self.config {
            match p.take() {
                Some(Ok(c)) => {
                    let v = c.settings.iter().find(|s| s.key == "autoswitch.model").map(|s| &s.value);
                    self.models = v.and_then(|v| v.as_str()).map(ready::parse_models).unwrap_or_default();
                    self.config = None;
                }
                Some(Err(_)) => self.config = None,
                None if p.is_lost() => self.config = None,
                None => {}
            }
        }
    }

    fn switch_to(&mut self, cx: &Cx, n: u32) {
        let p = cx.services.cswap.switch_to(&n.to_string(), false);
        self.ops.push(Op { key: format!("switch-{n}"), kind: OpKind::Switch(p) });
    }

    fn rotate(&mut self, cx: &Cx, s: Option<RotateStrategy>) {
        let p = cx.services.cswap.rotate(s);
        self.ops.push(Op { key: "rotate".into(), kind: OpKind::Switch(p) });
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        self.poll(cx);
        self.poll_config(ui, cx);
        let t = cx.tokens.clone();
        let settings = cx.settings.clone();
        let state = cx.accounts.clone();
        let list: Vec<Account> = state.payload.as_ref().map(|p| p.accounts.clone()).unwrap_or_default();
        if self.now_ms.is_none() {
            // 0.1.23's store ticked `now` every 15 s for the countdowns.
            ui.ctx().request_repaint_after(std::time::Duration::from_secs(15));
        }
        egui::ScrollArea::vertical().id_salt("accounts-page").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
            self.header(ui, cx, &t, &state, &list, &settings);
            egui::Frame::new().inner_margin(Margin { left: 24, right: 24, top: 4, bottom: 32 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.body(ui, cx, &t, &state, &list, &settings);
            });
        });
        self.dialogs(ui.ctx(), cx, &t, &list);
    }

    fn header(
        &mut self,
        ui: &mut Ui,
        cx: &mut Cx,
        t: &Tokens,
        state: &AccountsState,
        list: &[Account],
        s: &AppSettings,
    ) {
        let rotate =
            Button::new("Rotate").icon(icon::SHUFFLE).trailing(icon::CHEVRON_DOWN).loading(self.busy("rotate"));
        let add = Button::new("Add account").variant(Variant::Primary).icon(icon::PLUS).trailing(icon::CHEVRON_DOWN);
        let actions_w = 32.0 * 5.0 + rotate.width(ui) + add.width(ui) + 8.0 * 3.0 + 2.0 * 3.0;
        let mask = s.mask_emails;
        let tiles = s.accounts_layout == AccountsLayout::Tiles;
        let mut picked: (Option<usize>, Option<usize>, Option<usize>) = (None, None, None);
        let mut toggles = [false; 4];
        let sub = if list.is_empty() {
            None
        } else {
            let age = state
                .fetched_at
                .as_deref()
                .and_then(parse_iso_ms)
                .map(|f| age_seconds(Some((self.now() - f) as f64 / 1000.0)));
            Some(match age {
                Some(age) => format!("{} · usage refreshed {age}", view::count_text(list.len())),
                None => view::count_text(list.len()),
            })
        };
        widgets::page_header(
            ui,
            "Accounts",
            |ui| {
                if let Some(sub) = sub {
                    widgets::line(ui, sub, style::body(12.0), t.text_2);
                }
            },
            actions_w,
            |ui| {
                // right-to-left: the last control first
                let more = Button::new("").variant(Variant::Ghost).icon(icon::ELLIPSIS).label("More").show(ui);
                picked.2 = widgets::menu_list(
                    &more,
                    260.0,
                    &[
                        with(item("Export all accounts"), icon::DOWNLOAD),
                        with(item("Import accounts"), icon::UPLOAD),
                        None,
                        with(item_hint("Token diagnostics", "--token-status"), icon::STETHOSCOPE),
                    ],
                );
                let add_r = add.show(ui);
                picked.1 = widgets::menu_list(
                    &add_r,
                    240.0,
                    &[
                        with(item("From current Claude login"), icon::USER_PLUS),
                        with(item("From setup-token or API key"), icon::KEY_ROUND),
                    ],
                );
                let rot = rotate.show(ui);
                picked.0 = widgets::menu_list(
                    &rot,
                    260.0,
                    &[
                        with(item_hint("Next account", "rotation"), icon::ARROW_RIGHT_LEFT),
                        with(item_hint("Best (most quota left)", "best"), icon::SPARKLES),
                        with(item("Next available (skip limited)"), icon::SHUFFLE),
                    ],
                );
                // the four view toggles sit close together, a group apart from the buttons
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.add_space(6.0);
                let toggle = |ui: &mut Ui, glyph: &str, tip: &str| {
                    Button::new("").variant(Variant::Ghost).icon(glyph).label(tip).show(ui).on_hover_text(tip).clicked()
                };
                toggles[3] = if mask {
                    toggle(ui, icon::EYE_OFF, "Show email addresses")
                } else {
                    toggle(ui, icon::EYE, "Hide email addresses (keeps the first three characters)")
                };
                toggles[2] = if s.show_pace {
                    toggle(ui, icon::GAUGE, "Hide the pace marker")
                } else {
                    toggle(ui, icon::CIRCLE_GAUGE, "Show where an evenly-spread week would be")
                };
                toggles[1] = if s.show_hero {
                    toggle(ui, icon::PANEL_TOP, "Hide the active account card")
                } else {
                    toggle(ui, icon::PANEL_TOP, "Show the active account on top")
                };
                toggles[0] = if tiles {
                    toggle(ui, icon::LAYOUT_LIST, "Show accounts as rows")
                } else {
                    toggle(ui, icon::LAYOUT_GRID, "Show accounts as tiles")
                };
            },
        );
        let edit = |f: Box<dyn FnOnce(&mut AppSettings) + Send>| {
            cx.services.cswap.edit_settings(f);
        };
        if toggles[0] {
            let v = if tiles { AccountsLayout::Rows } else { AccountsLayout::Tiles };
            edit(Box::new(move |s| s.accounts_layout = v));
        }
        if toggles[1] {
            let v = !s.show_hero;
            edit(Box::new(move |s| s.show_hero = v));
        }
        if toggles[2] {
            let v = !s.show_pace;
            edit(Box::new(move |s| s.show_pace = v));
        }
        if toggles[3] {
            let v = !mask;
            edit(Box::new(move |s| s.mask_emails = v));
        }
        match picked.0 {
            Some(0) => self.rotate(cx, None),
            Some(1) => self.rotate(cx, Some(RotateStrategy::Best)),
            Some(2) => self.rotate(cx, Some(RotateStrategy::NextAvailable)),
            _ => {}
        }
        match picked.1 {
            Some(0) => self.open_add(),
            Some(1) => self.open_token(),
            _ => {}
        }
        match picked.2 {
            Some(0) => self.dlg = Some(Dlg::Export { a: None, full: false, run: None }),
            Some(1) => self.dlg = Some(Dlg::Import { force: false, run: None }),
            Some(3) => self.dlg = Some(Dlg::Diag { run: cx.services.cswap.token_status(), out: None }),
            _ => {}
        }
    }

    fn open_add(&mut self) {
        self.dlg = Some(Dlg::Add { slot: String::new(), alias: String::new(), overwrite: false, run: None });
    }

    fn open_token(&mut self) {
        self.dlg = Some(Dlg::Token {
            token: String::new(),
            email: String::new(),
            slot: String::new(),
            show: false,
            overwrite: false,
            run: None,
        });
    }

    fn body(&mut self, ui: &mut Ui, cx: &mut Cx, t: &Tokens, state: &AccountsState, list: &[Account], s: &AppSettings) {
        ui.spacing_mut().item_spacing = vec2(8.0, 12.0);
        if let Some(e) = &state.error {
            if state.payload.is_none() {
                widgets::notice(ui, &e.message, ChipTone::Danger);
            } else {
                widgets::notice(
                    ui,
                    &format!("Showing the last successful list — refresh failed: {}", e.message),
                    ChipTone::Warn,
                );
            }
        }
        if state.payload.is_none() && state.error.is_none() {
            // Loading: three placeholder rows.
            for _ in 0..3 {
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 62.0), Sense::hover());
                ui.painter().rect(
                    r,
                    CornerRadius::same(8),
                    t.surface,
                    Stroke::new(1.0, t.border),
                    egui::StrokeKind::Inside,
                );
            }
            return;
        }
        if state.payload.is_some() && list.is_empty() {
            let mut add = false;
            widgets::empty(
                ui,
                None,
                "No managed accounts yet",
                "Log into Claude Code with the account you want to keep, then add it here. cswap backs up its login so you can switch back to it any time.",
                |ui| {
                    add = Button::new("Add current login").variant(Variant::Primary).icon(icon::PLUS).show(ui).clicked()
                },
            );
            if add {
                self.open_add();
            }
            return;
        }
        if list.is_empty() {
            return;
        }
        let width = ui.available_width();
        let shape = view::page_shape(width);
        self.last_content_width = width;
        self.last_shape = shape;
        let (now, off) = (self.now(), self.offset());
        let hero = list.iter().find(|a| a.active).filter(|_| s.show_hero);
        let rows = ready::arrange(list, hero.is_some(), now);
        self.last_order = rows.iter().map(|(a, _)| a.number).collect();
        // The strip belongs to the hero: hidden with it.
        let best = hero.and_then(|_| ready::best_next(list, &self.models));
        self.last_best = best;
        let mut acts: Vec<(Account, RowAct)> = Vec::new();
        let p = Paint { t, s, now, off, mask: s.mask_emails };
        ui.spacing_mut().item_spacing.y = 0.0;
        if let Some(a) = hero {
            hero_card(ui, &p, a, state.plans.get(&a.email), shape);
            ui.add_space(12.0);
            if let Some(b) = best.and_then(|n| list.iter().find(|a| a.number == n)) {
                if self.best_strip(ui, &p, b, state.plans.get(&b.email)) {
                    self.rotate(cx, Some(RotateStrategy::Best));
                }
                ui.add_space(18.0);
            } else {
                ui.add_space(6.0);
            }
            if !rows.is_empty() {
                let hint = match s.accounts_layout {
                    AccountsLayout::Rows => "Ready first · click a row for per-model windows",
                    AccountsLayout::Tiles => "Ready first",
                };
                section_head(ui, t, "Other accounts", hint);
                ui.add_space(8.0);
            }
        }
        if !rows.is_empty() {
            match s.accounts_layout {
                AccountsLayout::Rows => self.rows(ui, &p, state, &rows, shape, &mut acts),
                AccountsLayout::Tiles => self.tiles(ui, &p, state, &rows, &mut acts),
            }
        }
        for (a, act) in acts {
            self.row_action(cx, a, act);
        }
        ui.add_space(10.0);
        let note = "Every write goes through the cswap CLI — nothing here edits the vault directly.";
        widgets::wrap_text(ui, note, style::body(11.5), t.text_3);
    }

    /// The Best next strip. True when its Switch was clicked.
    fn best_strip(&self, ui: &mut Ui, p: &Paint, b: &Account, plan: Option<&PlanInfo>) -> bool {
        let t = p.t;
        let w = ui.available_width();
        let mut clicked = false;
        egui::Frame::new()
            .fill(t.accent_soft)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(14, 10))
            .show(ui, |ui| {
                let iw = w - 28.0;
                ui.set_width(iw);
                let btn = Button::new("Switch")
                    .small()
                    .variant(Variant::Primary)
                    .loading(self.busy("rotate"))
                    .label("Switch to the best next account");
                let bw = btn.width(ui);
                let top = ui.cursor().min;
                let text_w = iw - 14.0 - 12.0 - bw - 12.0;
                let u = view::usage_of(b);
                let left = |w: Option<&UsageWindow>| match w {
                    Some(w) => format!("{}% left", pct(Some((100.0 - w.pct).max(0.0))).trim_end_matches('%')),
                    None => "no data".into(),
                };
                let tail = format!(
                    " — 5-hour {}, 7-day {}",
                    left(u.and_then(|u| u.five_hour.as_ref())),
                    left(u.and_then(|u| u.seven_day.as_ref()))
                );
                let chip = view::plan_badge(b, plan, p.now, p.off).map(|b| b.label);
                // "Best next: name [plan] — 5-hour …": one line while it fits, the numbers
                // under it otherwise (never sideways).
                let head = "Best next: ";
                let name = display_name(b, p.mask);
                let hw = widgets::measure(ui, head, style::body(12.0)).x;
                let nw = widgets::measure(ui, &name, style::body_semibold(12.0)).x;
                let cw = chip.as_deref().map(|c| widgets::chip_width(ui, c) + 6.0).unwrap_or(0.0);
                let tw = widgets::measure(ui, &tail, style::body(12.0)).x;
                let one_line = hw + nw + cw + tw <= text_w;
                let line_h = 18.0;
                let h: f32 = if one_line { line_h } else { line_h * 2.0 + 2.0 };
                let h = h.max(28.0);
                let pnt = ui.painter().clone();
                widgets::paint_icon(ui, icon::CORNER_DOWN_RIGHT, pos2(top.x + 7.0, top.y + h / 2.0), 14.0, t.text_2);
                let x0 = top.x + 14.0 + 12.0;
                let y1 = if one_line { top.y + (h - line_h) / 2.0 } else { top.y + (h - 2.0 * line_h - 2.0) / 2.0 };
                let mut x = x0;
                let g = widgets::galley(ui, head, style::body(12.0), t.text, text_w);
                pnt.galley(pos2(x, y1 + (line_h - g.size().y) / 2.0), g, t.text);
                x += hw;
                let g = widgets::galley(ui, name, style::body_semibold(12.0), t.text, (x0 + text_w - x).max(20.0));
                let gw = g.size().x;
                pnt.galley(pos2(x, y1 + (line_h - g.size().y) / 2.0), g, t.text);
                x += gw + 6.0;
                if let Some(c) = &chip
                    && x + cw <= x0 + text_w + 6.0
                {
                    let r = Rect::from_min_size(pos2(x, y1), vec2(cw - 6.0, 18.0));
                    widgets::place(ui, UiBuilder::new().max_rect(r), |ui| widgets::chip(ui, c, ChipTone::Muted));
                    x += cw;
                }
                let (tx, ty) = if one_line { (x - 6.0, y1) } else { (x0, y1 + line_h + 2.0) };
                let tail = if one_line { tail.as_str() } else { tail.trim_start() };
                let g = widgets::galley(ui, tail, style::body(12.0), t.text, (x0 + text_w - tx).max(20.0));
                pnt.galley(pos2(tx, ty + (line_h - g.size().y) / 2.0), g, t.text);
                let br = Rect::from_min_size(pos2(top.x + iw - bw, top.y + (h - 28.0) / 2.0), vec2(bw, 28.0));
                clicked = widgets::place(ui, UiBuilder::new().max_rect(br), |ui| btn.show(ui).clicked()).inner;
                ui.allocate_space(vec2(iw, h));
            });
        clicked
    }

    fn row_action(&mut self, cx: &mut Cx, a: Account, act: RowAct) {
        let n = a.number;
        match act {
            RowAct::Switch => self.switch_to(cx, n),
            RowAct::Toggle => {
                if !self.expanded.remove(&n) {
                    self.expanded.insert(n);
                }
            }
            RowAct::Menu(0) => self.dlg = Some(Dlg::Alias { alias: a.alias.clone().unwrap_or_default(), a, run: None }),
            RowAct::Menu(1) => self.dlg = Some(Dlg::Move { a, slot: String::new(), run: None }),
            RowAct::Menu(2) => {
                let disabled = a.disabled == Some(true);
                let ok = if disabled {
                    format!("Account-{n} back in rotation")
                } else {
                    format!("Account-{n} held out of rotation")
                };
                let p = cx.services.cswap.set_disabled(&n.to_string(), !disabled);
                self.ops.push(Op { key: format!("dis-{n}"), kind: OpKind::Plain(p, ok) });
            }
            RowAct::Menu(4) => {
                let load = Some(cx.services.cswap.list_mappings());
                self.dlg = Some(Dlg::Run { a, dir: String::new(), mappings: Vec::new(), load, run: None });
            }
            RowAct::Menu(5) => self.dlg = Some(Dlg::Export { a: Some(a), full: false, run: None }),
            RowAct::Menu(6) => self.open_add(),
            RowAct::Menu(8) => self.dlg = Some(Dlg::Remove { a, typed: String::new(), run: None }),
            RowAct::Menu(_) => {}
        }
    }

    /// The ⋯ menu of an account (every per-account action and dialog).
    fn menu(&self, ui: &mut Ui, a: &Account) -> Option<RowAct> {
        let n = a.number;
        let disabled = a.disabled == Some(true);
        let menu_label = format!("Actions for account {n}");
        let m = Button::new("").small().variant(Variant::Ghost).icon(icon::ELLIPSIS).label(&menu_label).show(ui);
        let run_hint = format!("cswap run {n}");
        let slot_hint = format!("slot {n}");
        let items = [
            with(item(if a.alias.is_some() { "Change alias" } else { "Set alias" }), icon::TAG),
            with(item("Move to slot"), icon::MOVE_VERTICAL),
            with(
                item(if disabled { "Enable (back in rotation)" } else { "Disable (hold out of rotation)" }),
                if disabled { icon::EYE } else { icon::EYE_OFF },
            ),
            None,
            with(item_hint("Open terminal as this account", &run_hint), icon::SQUARE_TERMINAL),
            with(item("Export this account"), icon::DOWNLOAD),
            with(item_hint("Re-add from the current login", &slot_hint), icon::PENCIL),
            None,
            Some(widgets::MenuItem {
                label: "Remove account",
                danger: true,
                icon: Some(icon::TRASH_2),
                ..Default::default()
            }),
        ];
        widgets::menu_list(&m, 280.0, &items).map(RowAct::Menu)
    }

    /// Switch, dimmed (still clickable: cswap has the last word) unless the account is
    /// ready. The live account keeps the button's space empty, so columns do not move.
    fn switch_button(&self, ui: &mut Ui, a: &Account, kind: Readiness) -> Option<RowAct> {
        let n = a.number;
        let label = format!("Switch to account {n}");
        let b = Button::new("Switch").small().loading(self.busy(&format!("switch-{n}"))).label(&label);
        if a.active {
            let w = b.width(ui);
            ui.add_space(w);
            return None;
        }
        let dim = kind != Readiness::Ready;
        let r = ui.scope(|ui| {
            if dim {
                ui.multiply_opacity(0.6);
            }
            b.show(ui)
        });
        r.inner.clicked().then_some(RowAct::Switch)
    }

    /// Width of a row's actions: [`Self::switch_button`] + 4 + the ⋯ button.
    fn actions_width(ui: &Ui) -> f32 {
        Button::new("Switch").small().width(ui) + 4.0 + 28.0
    }

    // ---- rows ------------------------------------------------------------------------

    fn rows(
        &self,
        ui: &mut Ui,
        p: &Paint,
        state: &AccountsState,
        rows: &[(&Account, ReadyInfo)],
        shape: PageShape,
        acts: &mut Vec<(Account, RowAct)>,
    ) {
        let t = p.t;
        let width = ui.available_width();
        let narrow = shape != PageShape::Wide;
        let act_w = Self::actions_width(ui);
        // Wide template (the mockup's `.rr`): # | name | readiness 120 | 5-hour | 7-day | actions.
        // The name column takes what its widest name line needs (up to leaving the bars 110
        // each), so its chips do not wrap beside an empty readiness column.
        let inner = width - 2.0 - 2.0 * ROW_PAD;
        let free = inner - CHEV_W - CHEV_GAP - READY_W - act_w - 4.0 * GAP;
        let need = rows
            .iter()
            .map(|(a, _)| {
                num_prefix_width(ui, a.number)
                    + name_line_width(ui, a, &chips_of(a, state.plans.get(&a.email), p, true))
            })
            .fold(0.0, f32::max);
        let share = free * 1.4 / 3.4;
        let name_w = if need > share { need.min(free - 2.0 * BAR_MIN).max(share) } else { share };
        let bar_w = (free - name_w) / 2.0;
        let cols = Cols { name_w, bar_w, act_w };
        widgets::card_frame(t).show(ui, |ui| {
            ui.set_width(width - 2.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            for (i, (a, r)) in rows.iter().enumerate() {
                let last = i + 1 == rows.len();
                let plan = state.plans.get(&a.email);
                let open = self.expanded.contains(&a.number);
                if let Some(act) = self.row(ui, p, a, r, plan, narrow, &cols, open, last) {
                    acts.push(((*a).clone(), act));
                }
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        &self,
        ui: &mut Ui,
        p: &Paint,
        a: &Account,
        r: &ReadyInfo,
        plan: Option<&PlanInfo>,
        narrow: bool,
        c: &Cols,
        open: bool,
        last: bool,
    ) -> Option<RowAct> {
        let t = p.t;
        let n = a.number;
        let top_left = ui.cursor().min;
        let width = ui.available_width();
        let id = Id::unique(("acct-row", n));
        // The click target goes in BEFORE the row's buttons, so they stay on top of it; its
        // height is last frame's (the row's own is known only once it is laid out).
        let prev_h = remembered(ui, id);
        let hit =
            ui.interact(Rect::from_min_size(top_left, vec2(width, prev_h.max(1.0))), id.with("hit"), Sense::click());
        let what = if open { "Hide" } else { "Show" };
        let hit_name = format!("{what} per-model windows for account {n}");
        hit.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Button, true, &hit_name));
        let bg = ui.painter().add(Shape::Noop);
        let mut row =
            ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(top_left, vec2(width, 10_000.0))).id_salt(id));
        if r.kind == Readiness::HeldOut {
            row.multiply_opacity(0.6);
        }
        let mut out = None;
        let x_chev = top_left.x + ROW_PAD;
        let x_name = x_chev + CHEV_W + CHEV_GAP;
        let y0 = top_left.y + 11.0;
        let right = top_left.x + width - ROW_PAD;
        let chips = chips_of(a, plan, p, !narrow);
        let name_w = if narrow { right - c.act_w - GAP - x_name } else { c.name_w };
        let name_h = widgets::place(
            &mut row,
            UiBuilder::new().max_rect(Rect::from_min_size(pos2(x_name, y0), vec2(name_w, 400.0))),
            |ui| name_block(ui, p, a, Some(n), &chips, style::body_medium(13.0), name_w),
        )
        .inner;
        let bars_h = 26.0;
        let (main_h, line1_mid, bars_y) = if narrow {
            let h1 = name_h.max(28.0);
            (h1 + 8.0 + bars_h, y0 + 14.0, y0 + h1 + 8.0)
        } else {
            let h = name_h.max(bars_h).max(34.0);
            (h, y0 + h / 2.0, y0 + (h - bars_h) / 2.0)
        };
        // the expand chevron alone at the left edge, turning 90° as the row opens; not a
        // button of its own: the whole row is the click target. The slot number leads the
        // name line (`name_block`). Narrow: beside the name line, wide: the row's middle.
        let turn = row.ctx().animate_bool_with_time(id.with("open"), open, TRANSITION_SECS);
        let chev_y = if narrow { y0 + 10.0 } else { line1_mid };
        chevron(row.painter(), pos2(x_chev + CHEV_W / 2.0, chev_y), turn, t.text_3);
        // bars
        let u = view::usage_of(a);
        let five = u.and_then(|u| u.five_hour.clone());
        let seven = u.and_then(|u| u.seven_day.clone());
        let (bx1, bw) = if narrow {
            let bw = (right - x_name - GAP) / 2.0;
            (x_name, bw)
        } else {
            (x_name + c.name_w + GAP + READY_W + GAP, c.bar_w)
        };
        for (k, (key, label, w)) in [("5h", "5-hour", &five), ("7d", "7-day", &seven)].into_iter().enumerate() {
            let x = bx1 + k as f32 * (bw + GAP);
            widgets::place(
                &mut row,
                UiBuilder::new().max_rect(Rect::from_min_size(pos2(x, bars_y), vec2(bw, bars_h))),
                |ui| mini_bar(ui, p, n, key, label, w.as_ref(), bw),
            );
        }
        // readiness (wide only: a narrow row says it with the Switch's dimming)
        if !narrow {
            let x = x_name + c.name_w + GAP;
            widgets::place(
                &mut row,
                UiBuilder::new().max_rect(Rect::from_min_size(pos2(x, line1_mid - 15.0), vec2(READY_W, 30.0))),
                |ui| readiness_block(ui, t, n, r),
            );
        }
        // actions
        let act_y = if narrow { y0 } else { line1_mid - 14.0 };
        let act = widgets::place(
            &mut row,
            UiBuilder::new()
                .max_rect(Rect::from_min_size(pos2(right - c.act_w, act_y), vec2(c.act_w, 28.0)))
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let m = self.menu(ui, a);
                let s = self.switch_button(ui, a, r.kind);
                m.or(s)
            },
        )
        .inner;
        out = out.or(act);
        // per-model windows, opened by a click on the row (~150 ms)
        let mut h = 11.0 + main_h + 11.0;
        if turn > 0.0 {
            let models: Vec<(String, UsageWindow)> =
                u.and_then(|u| u.scoped.clone()).unwrap_or_default().into_iter().map(|s| (s.name, s.window)).collect();
            let ex_x = x_name;
            let ex_w = right - ex_x;
            let ex_top = top_left.y + 11.0 + main_h + 10.0;
            let full = expansion_height(&models, ex_w);
            let shown = full * turn;
            let clip = Rect::from_min_size(pos2(top_left.x, ex_top - 2.0), vec2(width, shown + 4.0));
            let mut ex =
                row.new_child(UiBuilder::new().max_rect(Rect::from_min_size(pos2(ex_x, ex_top), vec2(ex_w, full))));
            ex.set_clip_rect(clip.intersect(row.clip_rect()));
            expansion(&mut ex, p, n, &models, ex_w);
            h += shown + 10.0 * turn;
        }
        let rect = Rect::from_min_size(top_left, vec2(width, h));
        remember(ui, id, h);
        let hover = widgets::hover_t(ui, id.with("bg"), ui.rect_contains_pointer(rect));
        let base = if a.active { t.accent_soft.gamma_multiply(0.4) } else { Color32::TRANSPARENT };
        let fill = style::mix(base, t.surface_2, hover * 0.7);
        let radius = if last { CornerRadius { nw: 0, ne: 0, sw: 9, se: 9 } } else { CornerRadius::ZERO };
        ui.painter().set(bg, egui::epaint::RectShape::filled(rect, radius, fill));
        if !last {
            ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, t.border));
        }
        ui.allocate_rect(rect, Sense::hover());
        if out.is_none() && hit.clicked() {
            out = Some(RowAct::Toggle);
        }
        out
    }

    // ---- tiles -----------------------------------------------------------------------

    fn tiles(
        &self,
        ui: &mut Ui,
        p: &Paint,
        state: &AccountsState,
        rows: &[(&Account, ReadyInfo)],
        acts: &mut Vec<(Account, RowAct)>,
    ) {
        let width = ui.available_width();
        let cols = view::tile_columns(width);
        let tile_w = (width - view::TILE_GAP * (cols - 1) as f32) / cols as f32;
        for (k, chunk) in rows.chunks(cols).enumerate() {
            let top = ui.cursor().min;
            let id = Id::unique(("tile-row", k));
            // A grid row is as tall as its tallest tile, footers on one line: the heights
            // are last frame's, so nothing is laid out twice.
            let row_h = remembered(ui, id);
            let mut tallest: f32 = 0.0;
            for (i, (a, r)) in chunk.iter().enumerate() {
                let x = top.x + i as f32 * (tile_w + view::TILE_GAP);
                let plan = state.plans.get(&a.email);
                let (natural, act) = self.tile(ui, p, a, r, plan, pos2(x, top.y), tile_w, row_h);
                tallest = tallest.max(natural);
                if let Some(act) = act {
                    acts.push(((*a).clone(), act));
                }
            }
            remember(ui, id, tallest);
            ui.allocate_rect(Rect::from_min_size(top, vec2(width, tallest.max(row_h))), Sense::hover());
            ui.add_space(view::TILE_GAP);
        }
    }

    /// One tile at `at`, `h` tall (0 = its own height). Returns its natural height.
    #[allow(clippy::too_many_arguments)]
    fn tile(
        &self,
        ui: &mut Ui,
        p: &Paint,
        a: &Account,
        r: &ReadyInfo,
        plan: Option<&PlanInfo>,
        at: egui::Pos2,
        w: f32,
        h: f32,
    ) -> (f32, Option<RowAct>) {
        let t = p.t;
        let n = a.number;
        let bg = ui.painter().add(Shape::Noop);
        let mut tile = ui
            .new_child(UiBuilder::new().max_rect(Rect::from_min_size(at, vec2(w, 10_000.0))).id_salt(("acct-tile", n)));
        if r.kind == Readiness::HeldOut {
            tile.multiply_opacity(0.6);
        }
        let (px, py) = (16.0, 14.0);
        let iw = w - 2.0 * px;
        let x0 = at.x + px;
        let mut y = at.y + py;
        let mut out = None;
        // name / email, ⋯
        let menu_w = 28.0;
        let pnt = tile.painter().clone();
        let name = display_name(a, p.mask);
        let g = widgets::galley(&tile, name, style::body_medium(13.0), t.text, iw - menu_w - 6.0);
        let name_rect = Rect::from_min_size(pos2(x0, y), g.size());
        pnt.galley(name_rect.min, g, t.text);
        name_tooltip(&mut tile, p, a, name_rect);
        let g = widgets::galley(&tile, sub_line(a, p.mask), style::body(12.0), t.text_3, iw - menu_w - 6.0);
        pnt.galley(pos2(x0, y + 19.0), g, t.text_3);
        let m = widgets::place(
            &mut tile,
            UiBuilder::new().max_rect(Rect::from_min_size(pos2(x0 + iw - menu_w, y - 2.0), vec2(menu_w, 28.0))),
            |ui| self.menu(ui, a),
        )
        .inner;
        out = out.or(m);
        y += 19.0 + 17.0 + 6.0;
        // chips, wrapping inside the tile
        let chips = chips_of(a, plan, p, true);
        if !chips.is_empty() {
            let ch = widgets::place(
                &mut tile,
                UiBuilder::new().max_rect(Rect::from_min_size(pos2(x0, y), vec2(iw, 200.0))),
                |ui| flow_chips(ui, n, &chips, iw),
            )
            .inner;
            y += ch;
        }
        y += 12.0;
        // two rings
        let u = view::usage_of(a);
        let mut rx = x0;
        for (key, label, win) in [
            ("5h", "5-hour", u.and_then(|u| u.five_hour.clone())),
            ("7d", "7-day", u.and_then(|u| u.seven_day.clone())),
        ] {
            let rw = widgets::place(
                &mut tile,
                UiBuilder::new().max_rect(Rect::from_min_size(pos2(rx, y), vec2(90.0, 120.0))),
                |ui| ring_with_caption(ui, p, n, key, label, win.as_ref(), 64.0, false),
            )
            .inner;
            rx += rw + 14.0;
        }
        y += 64.0 + 2.0 + 16.0 + 14.0 + 10.0;
        // per model, always shown
        let models: Vec<(String, UsageWindow)> =
            u.and_then(|u| u.scoped.clone()).unwrap_or_default().into_iter().map(|s| (s.name, s.window)).collect();
        let mh = widgets::place(
            &mut tile,
            UiBuilder::new().max_rect(Rect::from_min_size(pos2(x0, y), vec2(iw, 400.0))),
            |ui| model_rows(ui, p, n, &models, iw, true),
        )
        .inner;
        y += mh + 10.0;
        // footer: readiness, Switch; pinned to the bottom of the grid row
        let foot_h = 1.0 + 10.0 + 30.0;
        let natural = y + 4.0 + foot_h + py - at.y;
        let total = natural.max(h);
        let fy = at.y + total - py - foot_h;
        pnt.hline(x0..=x0 + iw, fy + 0.5, Stroke::new(1.0, t.border));
        widgets::place(
            &mut tile,
            UiBuilder::new().max_rect(Rect::from_min_size(pos2(x0, fy + 11.0), vec2(iw - 90.0, 30.0))),
            |ui| readiness_block(ui, t, n, r),
        );
        let s = widgets::place(
            &mut tile,
            UiBuilder::new()
                .max_rect(Rect::from_min_size(pos2(x0 + iw - 90.0, fy + 12.0), vec2(90.0, 28.0)))
                .layout(Layout::right_to_left(Align::Center)),
            |ui| self.switch_button(ui, a, r.kind),
        )
        .inner;
        out = out.or(s);
        let rect = Rect::from_min_size(at, vec2(w, total));
        let fill = if a.active { style::mix(t.surface, t.accent, 0.06) } else { t.surface };
        let shadow = style::card_shadow(t).as_shape(rect, CornerRadius::same(crate::theme::RADIUS_LG));
        ui.painter().set(
            bg,
            Shape::Vec(vec![
                shadow.into(),
                egui::epaint::RectShape::new(
                    rect,
                    CornerRadius::same(crate::theme::RADIUS_LG),
                    fill,
                    Stroke::new(1.0, t.border),
                    egui::StrokeKind::Inside,
                )
                .into(),
            ]),
        );
        (natural, out)
    }

    // ---- dialogs ---------------------------------------------------------------------

    fn dialogs(&mut self, ctx: &egui::Context, cx: &mut Cx, t: &Tokens, list: &[Account]) {
        let Some(mut d) = self.dlg.take() else { return };
        let taken: Vec<u32> = list.iter().map(|a| a.number).collect();
        let keep = match &mut d {
            Dlg::Add { slot, alias, overwrite, run } => add_dialog(ctx, cx, t, &taken, slot, alias, overwrite, run),
            Dlg::Token { token, email, slot, show, overwrite, run } => {
                token_dialog(ctx, cx, t, &taken, token, email, slot, show, overwrite, run)
            }
            Dlg::Alias { a, alias, run } => alias_dialog(ctx, cx, t, a, alias, run),
            Dlg::Move { a, slot, run } => move_dialog(ctx, cx, t, list, a, slot, run),
            Dlg::Remove { a, typed, run } => remove_dialog(ctx, cx, t, a, typed, run),
            Dlg::Export { a, full, run } => export_dialog(ctx, cx, t, a.as_ref(), full, run),
            Dlg::Import { force, run } => import_dialog(ctx, cx, t, force, run),
            Dlg::Diag { run, out } => diag_dialog(ctx, t, run, out),
            Dlg::Run { a, dir, mappings, load, run } => run_dialog(ctx, cx, t, a, dir, mappings, load, run),
        };
        if keep {
            self.dlg = Some(d);
        }
    }
}

// ---- shared pieces ----------------------------------------------------------------------

/// The mockup's row: 11 px top and bottom, 16 px sides, 16 px gaps, `#` 18, readiness 120.
const ROW_PAD: f32 = 16.0;
const GAP: f32 = 16.0;
const READY_W: f32 = 120.0;
/// The expand chevron's column at the left edge of a row (Purwa's variant E, PR #9): the
/// chevron alone there; beside the slot number it read as one glyph ("› 6"), and at the
/// row's right end, after ⋯, it looked out of place. The number leads the name line.
const CHEV_W: f32 = 14.0;
const CHEV_GAP: f32 = 10.0;
/// Room on each side of the `·` between the slot number and the name.
const DOT_PAD: f32 = 6.0;

/// Width of a row's `6 · ` before the name.
fn num_prefix_width(ui: &Ui, n: u32) -> f32 {
    widgets::measure(ui, &n.to_string(), style::body(12.0)).x
        + DOT_PAD
        + widgets::measure(ui, "·", style::body(12.0)).x
        + DOT_PAD
}
/// The bars never get narrower than this to make room for a long name line: `5-hour 100%`
/// and `2d 21h` side by side still fit.
const BAR_MIN: f32 = 120.0;

/// Column widths of the wide row template.
struct Cols {
    name_w: f32,
    bar_w: f32,
    act_w: f32,
}

/// What every painted piece of the page needs.
struct Paint<'a> {
    t: &'a Tokens,
    s: &'a AppSettings,
    now: i64,
    off: i64,
    mask: bool,
}

/// Height a block had last frame (0 before the first), so a row can centre or bottom-align
/// its parts and place its click target without laying itself out twice.
fn remembered(ui: &Ui, id: Id) -> f32 {
    ui.ctx().data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0)
}

fn remember(ui: &Ui, id: Id, h: f32) {
    if (remembered(ui, id) - h).abs() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(id, h));
        ui.ctx().request_repaint();
    }
}

fn display_name(a: &Account, mask: bool) -> String {
    a.alias.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| mask_email(&a.email, mask))
}

/// The line under a name: `(email ·) org`.
fn sub_line(a: &Account, mask: bool) -> String {
    let mut line = String::new();
    if a.alias.as_deref().is_some_and(|s| !s.is_empty()) {
        line.push_str(&mask_email(&a.email, mask));
        line.push_str(" · ");
    }
    line.push_str(&org_tag(a, mask));
    line
}

/// Plan (cswap's `plan`, else plans.json), canceled / lapsed, the login's expiry once it
/// is within a week or past, and `disabled`. Where a readiness text beside the name
/// already says "Held out · disabled" or "Re-login needed · login expired" (a wide row, a
/// tile: `readiness_shown`), those two chips are left out: repeated, they only made the
/// name line wrap. The hero says "active" by being the hero.
fn chips_of(a: &Account, plan: Option<&PlanInfo>, p: &Paint, readiness_shown: bool) -> Vec<ChipSpec> {
    let mut chips = Vec::new();
    if let Some(b) = view::plan_badge(a, plan, p.now, p.off) {
        let standing = b.standing_chip();
        chips.push(ChipSpec { label: b.label, tone: ChipTone::Muted, tip: Some(b.tooltip) });
        if let Some((word, tone, tip)) = standing {
            chips.push(ChipSpec { label: word.into(), tone, tip: Some(tip.into()) });
        }
    }
    if let Some(l) = view::login_expiry(a, p.now, p.off)
        && let Some((text, tone)) = l.chip
        && !(readiness_shown && tone == ChipTone::Danger)
    {
        chips.push(ChipSpec { label: text, tone, tip: Some(l.tooltip) });
    }
    if !readiness_shown && a.disabled == Some(true) {
        chips.push(ChipSpec {
            label: "disabled".into(),
            tone: ChipTone::Muted,
            tip: Some("Held out of rotation".into()),
        });
    }
    chips
}

/// Width of the name and all its chips on one line.
fn name_line_width(ui: &Ui, a: &Account, chips: &[ChipSpec]) -> f32 {
    let name = display_name(a, false);
    widgets::measure(ui, &name, style::body_medium(13.0)).x
        + chips.iter().map(|c| 6.0 + widgets::chip_width(ui, &c.label)).sum::<f32>()
        + 2.0
}

/// The name's hover text: the full address and the login's expiry.
fn name_tooltip(ui: &mut Ui, p: &Paint, a: &Account, rect: Rect) {
    let login = view::login_expiry(a, p.now, p.off);
    let tip = match (p.mask, &login) {
        (false, Some(l)) => Some(format!("{}\n{}", a.email, l.tooltip)),
        (false, None) => Some(a.email.clone()),
        (true, Some(l)) => Some(l.tooltip.clone()),
        (true, None) => None,
    };
    if let Some(tip) = tip {
        let _ = ui
            .interact(rect, Id::unique(("acct-name", a.number, rect.min.y as i32)), Sense::hover())
            .on_hover_text(tip);
    }
}

/// Name, its chips, and the email / org line under them. A chip goes to a new line only
/// when it does not fit on the current one. Returns the height used.
fn name_block(
    ui: &mut Ui,
    p: &Paint,
    a: &Account,
    num: Option<u32>,
    chips: &[ChipSpec],
    font: egui::FontId,
    w: f32,
) -> f32 {
    let t = p.t;
    let top = ui.cursor().min;
    let line_h = font.size.max(18.0) + 2.0;
    // A row's name line reads `6 · name [chips]`, number and dot muted; a wrapped chip
    // and the meta line start flush with the number (variant E).
    let lead = if let Some(n) = num {
        let y = top.y + line_h / 2.0;
        let pnt = ui.painter();
        let r = pnt.text(pos2(top.x, y), Align2::LEFT_CENTER, n.to_string(), style::body(12.0), t.text_3);
        let d = pnt.text(pos2(r.max.x + DOT_PAD, y), Align2::LEFT_CENTER, "·", style::body(12.0), t.text_3);
        d.max.x + DOT_PAD - top.x
    } else {
        0.0
    };
    let left = top.x + lead;
    let g = widgets::galley(ui, display_name(a, p.mask), font, t.text, w - lead);
    let name_rect = Rect::from_min_size(pos2(left, top.y + (line_h - g.size().y) / 2.0), g.size());
    ui.painter().galley(name_rect.min, g, t.text);
    name_tooltip(ui, p, a, name_rect);
    let (mut x, mut y) = (name_rect.max.x + 6.0, top.y);
    for (i, c) in chips.iter().enumerate() {
        let cw = widgets::chip_width(ui, &c.label);
        if x + cw > top.x + w && x > left {
            x = top.x;
            y += 22.0;
        }
        let r = Rect::from_min_size(pos2(x, y + (line_h - 18.0) / 2.0), vec2(cw, 18.0));
        let resp = widgets::place(ui, UiBuilder::new().max_rect(r), |ui| widgets::chip(ui, &c.label, c.tone)).inner;
        if let Some(tip) = &c.tip {
            let _ = ui
                .interact(resp.rect, Id::unique(("acct-chip", a.number, i, top.y as i32)), Sense::hover())
                .on_hover_text(tip);
        }
        x += cw + 6.0;
    }
    let y2 = y + line_h;
    let g2 = widgets::galley(ui, sub_line(a, p.mask), style::body(12.0), t.text_3, w);
    ui.painter().galley(pos2(top.x, y2), g2, t.text_3);
    let h = y2 + 16.0 - top.y;
    ui.allocate_space(vec2(w, h));
    h
}

/// Chips alone, flowing over lines (tiles). Returns the height used.
fn flow_chips(ui: &mut Ui, n: u32, chips: &[ChipSpec], w: f32) -> f32 {
    let top = ui.cursor().min;
    let (mut x, mut y) = (top.x, top.y);
    for (i, c) in chips.iter().enumerate() {
        let cw = widgets::chip_width(ui, &c.label);
        if x + cw > top.x + w && x > top.x {
            x = top.x;
            y += 22.0;
        }
        let resp =
            widgets::place(ui, UiBuilder::new().max_rect(Rect::from_min_size(pos2(x, y), vec2(cw, 18.0))), |ui| {
                widgets::chip(ui, &c.label, c.tone)
            })
            .inner;
        if let Some(tip) = &c.tip {
            let _ = ui.interact(resp.rect, Id::unique(("tile-chip", n, i)), Sense::hover()).on_hover_text(tip);
        }
        x += cw + 6.0;
    }
    y + 18.0 - top.y
}

/// The small uppercase label (`ACTIVE NOW`, `PER MODEL · 7-DAY`, `OTHER ACCOUNTS`).
fn caps_label(ui: &Ui, text: &str, size: f32, color: Color32) -> std::sync::Arc<egui::Galley> {
    let mut job = widgets::job(&text.to_uppercase(), style::body_medium(size), color);
    job.sections[0].format.extra_letter_spacing = size * 0.06;
    ui.painter().layout_job(job)
}

fn section_head(ui: &mut Ui, t: &Tokens, label: &str, hint: &str) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 16.0), Sense::hover());
    let g = caps_label(ui, label, 11.0, t.text_3);
    let lw = g.size().x;
    ui.painter().galley(pos2(r.min.x + 2.0, r.center().y - g.size().y / 2.0), g, t.text_3);
    let hg = widgets::galley(ui, hint, style::body(11.0), t.text_3, (w - lw - 24.0).max(10.0));
    ui.painter().galley(pos2(r.max.x - 2.0 - hg.size().x, r.center().y - hg.size().y / 2.0), hg, t.text_3);
}

/// A `›` that turns to `⌄` as `turn` goes 0 → 1.
fn chevron(p: &egui::Painter, c: egui::Pos2, turn: f32, color: Color32) {
    let a = turn * std::f32::consts::FRAC_PI_2;
    let rot = |v: egui::Vec2| c + vec2(v.x * a.cos() - v.y * a.sin(), v.x * a.sin() + v.y * a.cos());
    let pts = vec![rot(vec2(-1.5, -3.5)), rot(vec2(2.0, 0.0)), rot(vec2(-1.5, 3.5))];
    p.add(Shape::line(pts, Stroke::new(1.3, color)));
}

/// Dot, label, and the small line under it.
fn readiness_block(ui: &mut Ui, t: &Tokens, n: u32, r: &ReadyInfo) {
    let top = ui.cursor().min;
    let dot = match r.kind.tone() {
        ChipTone::Ok => t.ok,
        ChipTone::Warn => t.warn,
        ChipTone::Danger => t.danger,
        _ => t.text_3,
    };
    let w = ui.available_width();
    let g1 = widgets::galley(ui, r.kind.label(), style::body_medium(12.0), t.text, w - 13.0);
    let g2 = widgets::galley(ui, &r.detail, style::body(11.0), t.text_3, w - 13.0);
    let h = g1.size().y + g2.size().y;
    let y = top.y + (30.0 - h) / 2.0;
    ui.painter().circle_filled(pos2(top.x + 3.5, y + h / 2.0), 3.5, dot);
    ui.painter().galley(pos2(top.x + 13.0, y), g1, t.text);
    ui.painter().galley(pos2(top.x + 13.0, y + h - g2.size().y), g2, t.text_3);
    let (_, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
    let name = format!("{n} readiness: {} · {}", r.kind.label(), r.detail);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, &name));
}

/// The pace a window shows, when the setting is on.
fn pace_of(p: &Paint, w: &UsageWindow) -> Option<f64> {
    if p.s.show_pace { w.expected_pct } else { None }
}

/// The mockup's `.mb`: `5-hour 8%` left, the reset right, a 6 px track under them.
fn mini_bar(ui: &mut Ui, p: &Paint, n: u32, key: &str, label: &str, w: Option<&UsageWindow>, width: f32) {
    let t = p.t;
    let id = Id::unique(("window", n, key));
    ui.spacing_mut().item_spacing.y = 0.0;
    let (top, _) = ui.allocate_exact_size(vec2(width, 16.0), Sense::hover());
    let pnt = ui.painter().clone();
    // The percentage first; the reset takes what is left (and is in the tooltip anyway).
    let head = format!("{label} {}", w.map(|w| pct(Some(w.pct))).unwrap_or_else(|| "-".into()));
    let lg = widgets::galley(ui, head, style::body(11.0), t.text_2, width);
    let lw = lg.size().x;
    pnt.galley(pos2(top.min.x, top.center().y - lg.size().y / 2.0), lg, t.text_2);
    let reset = w.and_then(|w| w.resets_at.as_deref()).map(|r| reset_text(Some(r), p.now)).unwrap_or_default();
    if width - lw - 8.0 > 12.0 {
        let rg = widgets::galley(ui, reset, style::body(11.0), t.text_3, width - lw - 8.0);
        pnt.galley(pos2(top.max.x - rg.size().x, top.center().y - rg.size().y / 2.0), rg, t.text_3);
    }
    ui.add_space(4.0);
    usage::bar(ui, t, id, w.map(|w| w.pct), tone(w.map(|w| w.pct)), w.and_then(|w| pace_of(p, w)), width);
    let whole = Rect::from_min_max(top.min, pos2(top.max.x, top.max.y + 10.0));
    let r = ui.interact(whole, id.with("tip"), Sense::hover());
    let a11y = format!("{n} {key}: {}", w.map(|w| pct(Some(w.pct))).unwrap_or_else(|| "-".into()));
    r.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, &a11y));
    if let Some(w) = w {
        let name = if key == "5h" {
            "5-hour window"
        } else if key == "7d" {
            "7-day window"
        } else {
            label
        };
        let _ = r.on_hover_text(view::window_tooltip(w, name, p.now, p.off));
    }
}

/// A ring with `5-hour` and `resets 3h 30m` centred under it. Returns the width used.
#[allow(clippy::too_many_arguments)]
fn ring_with_caption(
    ui: &mut Ui,
    p: &Paint,
    n: u32,
    key: &str,
    label: &str,
    w: Option<&UsageWindow>,
    size: f32,
    large: bool,
) -> f32 {
    let t = p.t;
    let top = ui.cursor().min;
    let reset = w.and_then(|w| w.resets_at.as_deref()).map(|r| format!("resets {}", reset_text(Some(r), p.now)));
    let sub = reset.unwrap_or_else(|| if w.is_none() { "no data".into() } else { String::new() });
    let g1 = widgets::galley(ui, label, style::body(12.0), t.text_2, 200.0);
    let g2 = widgets::galley(ui, sub, style::body(11.0), t.text_3, 200.0);
    let cw = size.max(g1.size().x).max(g2.size().x);
    let id = Id::unique(("window", n, key));
    let rr = widgets::place(
        ui,
        UiBuilder::new().max_rect(Rect::from_min_size(pos2(top.x + (cw - size) / 2.0, top.y), vec2(size, size))),
        |ui| usage::ring(ui, t, id, w.map(|w| w.pct), w.and_then(|w| pace_of(p, w)), size),
    )
    .inner;
    let y1 = top.y + size + if large { 4.0 } else { 2.0 };
    ui.painter().galley(pos2(top.x + (cw - g1.size().x) / 2.0, y1), g1.clone(), t.text_2);
    ui.painter().galley(pos2(top.x + (cw - g2.size().x) / 2.0, y1 + g1.size().y), g2.clone(), t.text_3);
    let h = size + 4.0 + g1.size().y + g2.size().y;
    let a11y = format!("{n} {key}: {}", w.map(|w| pct(Some(w.pct))).unwrap_or_else(|| "-".into()));
    rr.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, &a11y));
    if let Some(w) = w {
        let name = if key == "5h" { "5-hour window" } else { "7-day window" };
        let _ = rr.on_hover_text(view::window_tooltip(w, name, p.now, p.off));
    }
    ui.allocate_space(vec2(cw, h));
    cw
}

/// `PER MODEL · 7-DAY` and one `name | bar | %` row per model (or the "none" line).
/// `small` is a tile's (10/11 px, 56 px names), else the hero's (11/12 px, 70 px names).
fn model_rows(ui: &mut Ui, p: &Paint, n: u32, models: &[(String, UsageWindow)], w: f32, small: bool) -> f32 {
    let t = p.t;
    let top = ui.cursor().min;
    let (lab, font, name_w, pct_w, row_h) =
        if small { (10.0, 11.0, 56.0, 34.0, 17.0) } else { (11.0, 12.0, 70.0, 54.0, 22.0) };
    let g = caps_label(ui, "Per model · 7-day", lab, t.text_3);
    let mut y = top.y;
    ui.painter().galley(pos2(top.x, y), g.clone(), t.text_3);
    y += g.size().y + if small { 2.0 } else { 6.0 };
    if models.is_empty() {
        widgets::place(
            ui,
            UiBuilder::new().max_rect(Rect::from_min_size(pos2(top.x, y + 2.0), vec2(w, row_h))),
            |ui| none_reported(ui, font, t.text_3),
        );
        y += row_h;
    }
    for (name, win) in models {
        let mid = y + row_h / 2.0;
        let gn = widgets::galley(ui, name, style::body(font), if small { t.text_2 } else { t.text }, name_w - 4.0);
        ui.painter().galley(pos2(top.x, mid - gn.size().y / 2.0), gn, t.text);
        let bar_x = top.x + name_w + if small { 8.0 } else { 10.0 };
        let bar_w = (w - name_w - pct_w - if small { 16.0 } else { 20.0 }).max(20.0);
        let key = format!("model-{name}");
        let id = Id::unique(("window", n, &key));
        widgets::place(
            ui,
            UiBuilder::new().max_rect(Rect::from_min_size(pos2(bar_x, mid - 3.0), vec2(bar_w, 6.0))),
            |ui| usage::bar(ui, t, id, Some(win.pct), tone(Some(win.pct)), pace_of(p, win), bar_w),
        );
        let gp = widgets::galley(ui, pct(Some(win.pct)), style::body(font), t.text_3, pct_w);
        ui.painter().galley(pos2(top.x + w - gp.size().x, mid - gp.size().y / 2.0), gp, t.text_3);
        let hit =
            ui.interact(Rect::from_min_max(pos2(top.x, y), pos2(top.x + w, y + row_h)), id.with("tip"), Sense::hover());
        let a11y = format!("{n} {key}: {}", pct(Some(win.pct)));
        hit.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, &a11y));
        let _ = hit.on_hover_text(view::window_tooltip(win, name, p.now, p.off));
        y += row_h;
    }
    let h = y - top.y;
    ui.allocate_space(vec2(w, h));
    h
}

/// "No per-model limits reported", as a node a test (or a screen reader) can find.
fn none_reported(ui: &mut Ui, size: f32, color: Color32) {
    let text = "No per-model limits reported";
    let r = widgets::line(ui, text, style::body(size), color);
    r.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, text));
}

/// The opened row: one mini bar per model, `Fable · 7-day 20%`, in an auto-fill grid of
/// columns at least 200 px wide.
fn expansion(ui: &mut Ui, p: &Paint, n: u32, models: &[(String, UsageWindow)], w: f32) {
    let top = ui.cursor().min;
    if models.is_empty() {
        none_reported(ui, 11.0, p.t.text_3);
        return;
    }
    let cols = (((w + 24.0) / (200.0 + 24.0)).floor() as usize).max(1);
    let cw = (w - 24.0 * (cols - 1) as f32) / cols as f32;
    for (i, (name, win)) in models.iter().enumerate() {
        let (r, c) = (i / cols, i % cols);
        let at = pos2(top.x + c as f32 * (cw + 24.0), top.y + r as f32 * (26.0 + 6.0));
        let key = format!("model-{name}");
        let label = format!("{name} · 7-day");
        widgets::place(ui, UiBuilder::new().max_rect(Rect::from_min_size(at, vec2(cw, 26.0))), |ui| {
            mini_bar(ui, p, n, &key, &label, Some(win), cw)
        });
    }
}

fn expansion_height(models: &[(String, UsageWindow)], w: f32) -> f32 {
    if models.is_empty() {
        return 14.0;
    }
    let cols = (((w + 24.0) / (200.0 + 24.0)).floor() as usize).max(1);
    let rows = models.len().div_ceil(cols);
    rows as f32 * 26.0 + (rows - 1) as f32 * 6.0
}

// ---- the hero ---------------------------------------------------------------------------

const HERO_RING: f32 = 104.0;
const HERO_RING_GAP: f32 = 22.0;

/// The active account's card: who it is (+ the forecast), two large rings, and per model
/// with the status. Narrow: per model + status under a divider; compact: rings centred.
fn hero_card(ui: &mut Ui, p: &Paint, a: &Account, plan: Option<&PlanInfo>, shape: PageShape) {
    let t = p.t;
    let w = ui.available_width();
    widgets::card_frame(t).inner_margin(Margin::symmetric(20, 18)).show(ui, |ui| {
        let iw = w - 42.0;
        ui.set_width(iw);
        let top = ui.cursor().min;
        let id = Id::unique("hero");
        let rings_w = rings_width(ui, p, a);
        let mut y_end;
        match shape {
            PageShape::Wide => {
                let gap = 28.0;
                let rest = iw - rings_w - 2.0 * gap;
                let who_w = rest * 1.25 / 2.25;
                let mod_w = rest - who_w;
                // centred on one line: heights from last frame (see `remembered`)
                let hs = [remembered(ui, id.with(0)), remembered(ui, id.with(1)), remembered(ui, id.with(2))];
                let row_h = hs.iter().cloned().fold(0.0, f32::max);
                let xs = [top.x, top.x + who_w + gap, top.x + who_w + gap + rings_w + gap];
                let ws = [who_w, rings_w, mod_w];
                let mut got = [0.0f32; 3];
                for k in 0..3 {
                    let y = top.y + (row_h - hs[k]).max(0.0) / 2.0;
                    let r = Rect::from_min_size(pos2(xs[k], y), vec2(ws[k], 600.0));
                    got[k] = widgets::place(ui, UiBuilder::new().max_rect(r), |ui| match k {
                        0 => hero_who(ui, p, a, plan, ws[k]),
                        1 => hero_rings(ui, p, a),
                        _ => hero_models(ui, p, a, ws[k]),
                    })
                    .inner;
                }
                for (k, g) in got.iter().enumerate() {
                    remember(ui, id.with(k), *g);
                }
                y_end = top.y + got.iter().cloned().fold(0.0, f32::max).max(row_h);
            }
            PageShape::Narrow => {
                let gap = 20.0;
                let who_w = iw - rings_w - gap;
                let hs = [remembered(ui, id.with(10)), remembered(ui, id.with(11))];
                let row_h = hs[0].max(hs[1]);
                let h0 = widgets::place(
                    ui,
                    UiBuilder::new().max_rect(Rect::from_min_size(
                        pos2(top.x, top.y + (row_h - hs[0]).max(0.0) / 2.0),
                        vec2(who_w, 600.0),
                    )),
                    |ui| hero_who(ui, p, a, plan, who_w),
                )
                .inner;
                let h1 = widgets::place(
                    ui,
                    UiBuilder::new().max_rect(Rect::from_min_size(
                        pos2(top.x + iw - rings_w, top.y + (row_h - hs[1]).max(0.0) / 2.0),
                        vec2(rings_w, 600.0),
                    )),
                    |ui| hero_rings(ui, p, a),
                )
                .inner;
                remember(ui, id.with(10), h0);
                remember(ui, id.with(11), h1);
                y_end = top.y + h0.max(h1).max(row_h);
            }
            PageShape::Compact => {
                let h0 =
                    widgets::place(ui, UiBuilder::new().max_rect(Rect::from_min_size(top, vec2(iw, 600.0))), |ui| {
                        hero_who(ui, p, a, plan, iw)
                    })
                    .inner;
                let y = top.y + h0 + 16.0;
                let h1 = widgets::place(
                    ui,
                    UiBuilder::new()
                        .max_rect(Rect::from_min_size(pos2(top.x + (iw - rings_w) / 2.0, y), vec2(rings_w, 600.0))),
                    |ui| hero_rings(ui, p, a),
                )
                .inner;
                y_end = y + h1;
            }
        }
        if shape != PageShape::Wide {
            y_end += 16.0;
            ui.painter().hline(top.x..=top.x + iw, y_end - 0.5, Stroke::new(1.0, t.border));
            y_end += 10.0;
            let h = widgets::place(
                ui,
                UiBuilder::new().max_rect(Rect::from_min_size(pos2(top.x, y_end), vec2(iw, 600.0))),
                |ui| hero_models(ui, p, a, iw),
            )
            .inner;
            y_end += h;
        }
        ui.allocate_space(vec2(iw, y_end - top.y));
    });
}

fn rings_width(ui: &Ui, p: &Paint, a: &Account) -> f32 {
    let u = view::usage_of(a);
    let cap = |w: Option<&UsageWindow>| {
        let s = w
            .and_then(|w| w.resets_at.as_deref())
            .map(|r| format!("resets {}", reset_text(Some(r), p.now)))
            .unwrap_or_default();
        widgets::measure(ui, &s, style::body(11.0)).x.max(HERO_RING)
    };
    cap(u.and_then(|u| u.five_hour.as_ref())) + HERO_RING_GAP + cap(u.and_then(|u| u.seven_day.as_ref()))
}

fn hero_who(ui: &mut Ui, p: &Paint, a: &Account, plan: Option<&PlanInfo>, w: f32) -> f32 {
    let t = p.t;
    let top = ui.cursor().min;
    let g = caps_label(ui, "Active now", 11.0, t.text_3);
    let lh = g.size().y;
    ui.painter().galley(top, g, t.text_3);
    let chips = chips_of(a, plan, p, false);
    let nh = widgets::place(
        ui,
        UiBuilder::new().max_rect(Rect::from_min_size(pos2(top.x, top.y + lh + 4.0), vec2(w, 300.0))),
        |ui| name_block(ui, p, a, None, &chips, style::body_semibold(18.0), w),
    )
    .inner;
    let mut y = top.y + lh + 4.0 + nh;
    if let Some((text, tone)) = ready::forecast(a, p.now, p.off) {
        y += 12.0;
        let dot = if tone == ChipTone::Ok { t.ok } else { t.warn };
        let g = widgets::wrapped(ui, text, style::body(12.0), t.text_2, w - 13.0);
        let gh = g.size().y;
        ui.painter().circle_filled(pos2(top.x + 3.5, y + 8.0), 3.5, dot);
        ui.painter().galley(pos2(top.x + 13.0, y), g, t.text_2);
        y += gh;
    }
    let h = y - top.y;
    ui.allocate_space(vec2(w, h));
    h
}

fn hero_rings(ui: &mut Ui, p: &Paint, a: &Account) -> f32 {
    let u = view::usage_of(a);
    let top = ui.cursor().min;
    let five = u.and_then(|u| u.five_hour.clone());
    let seven = u.and_then(|u| u.seven_day.clone());
    let n = a.number;
    let mut x = top.x;
    let mut h: f32 = 0.0;
    for (key, label, win) in [("5h", "5-hour", five.as_ref()), ("7d", "7-day", seven.as_ref())] {
        let r = widgets::place(
            ui,
            UiBuilder::new().max_rect(Rect::from_min_size(pos2(x, top.y), vec2(200.0, 200.0))),
            |ui| {
                let w = ring_with_caption(ui, p, n, key, label, win, HERO_RING, true);
                (w, ui.min_rect().height())
            },
        )
        .inner;
        x += r.0 + HERO_RING_GAP;
        h = h.max(r.1);
    }
    ui.allocate_space(vec2(x - HERO_RING_GAP - top.x, h));
    h
}

fn hero_models(ui: &mut Ui, p: &Paint, a: &Account, w: f32) -> f32 {
    let t = p.t;
    let top = ui.cursor().min;
    let models: Vec<(String, UsageWindow)> = view::usage_of(a)
        .and_then(|u| u.scoped.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|s| (s.name, s.window))
        .collect();
    let mh = widgets::place(ui, UiBuilder::new().max_rect(Rect::from_min_size(top, vec2(w, 400.0))), |ui| {
        model_rows(ui, p, a.number, &models, w, false)
    })
    .inner;
    // status chip · login expires in … · measured …
    let y = top.y + mh + 6.0;
    let chip_r =
        widgets::place(ui, UiBuilder::new().max_rect(Rect::from_min_size(pos2(top.x, y), vec2(w, 18.0))), |ui| {
            widgets::chip(ui, &status_label(&a.usage_status), status_tone(&a.usage_status))
        })
        .inner;
    let _ = chip_r.clone().on_hover_text(&a.usage_status);
    let mut bits: Vec<String> = Vec::new();
    if let Some(at) = a.login_expires_at.as_deref().and_then(parse_iso_ms) {
        let iso = iso_from_ms(at);
        bits.push(if at > p.now {
            format!("login expires in {}", reset_text(Some(&iso), p.now))
        } else {
            "login expired".into()
        });
    }
    if view::is_stale(a) {
        bits.push(format!("last good {}", age_seconds(a.last_good_age_seconds)));
    } else if let Some(s) = a.usage_age_seconds.filter(|s| *s >= 60.0) {
        bits.push(format!("measured {}", age_seconds(Some(s))));
    }
    if !bits.is_empty() {
        let x = chip_r.rect.max.x;
        let g = widgets::galley(
            ui,
            format!(" · {}", bits.join(" · ")),
            style::body(11.0),
            t.text_3,
            (top.x + w - x).max(10.0),
        );
        ui.painter().galley(pos2(x, y + 9.0 - g.size().y / 2.0), g, t.text_3);
    }
    let h = y + 18.0 - top.y;
    ui.allocate_space(vec2(w, h));
    h
}

// ---- dialog bodies ------------------------------------------------------------------

/// Shared resolution of a dialog's in-flight CLI call: a success toasts and closes, a
/// refusal toasts and keeps the dialog open so the input is not lost.
fn settle<T>(cx: &mut Cx, run: &mut Option<Pending<CsResult<T>>>, ok: impl FnOnce(&T) -> String) -> Option<bool> {
    let p = run.as_mut()?;
    match p.take() {
        Some(Ok(v)) => {
            let msg = ok(&v);
            notify(cx, ToastKind::Success, &msg, None);
            *run = None;
            Some(true)
        }
        Some(Err(e)) => {
            notify(cx, ToastKind::Error, "cswap refused", Some(&e.message));
            *run = None;
            Some(false)
        }
        None => {
            if p.is_lost() {
                *run = None;
            }
            None
        }
    }
}

fn steps(ui: &mut Ui, t: &Tokens, n: u32, body: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 9.0, t.surface_2);
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, n.to_string(), style::body(11.0), t.text_2);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 2.0);
            body(ui)
        });
    });
}

fn slot_warning(ui: &mut Ui, t: &Tokens, text: &str, overwrite: &mut bool) {
    let w = ui.available_width();
    egui::Frame::new()
        .fill(t.warn_soft)
        .stroke(Stroke::new(1.0, t.warn.gamma_multiply(0.3)))
        .corner_radius(CornerRadius::same(crate::theme::RADIUS_MD))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(w - 26.0);
            ui.horizontal(|ui| {
                widgets::line(ui, text, style::body(12.5), t.warn);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    widgets::toggle(ui, overwrite, "Overwrite slot", true)
                });
            });
        });
}

fn two_fields(ui: &mut Ui, left: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    let w = ui.available_width();
    let half = (w - 12.0) / 2.0;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.allocate_ui_with_layout(vec2(half, 80.0), Layout::top_down(Align::Min), left);
        ui.allocate_ui_with_layout(vec2(half, 80.0), Layout::top_down(Align::Min), right);
    });
}

#[allow(clippy::too_many_arguments)]
fn add_dialog(
    ctx: &egui::Context,
    cx: &mut Cx,
    t: &Tokens,
    taken: &[u32],
    slot: &mut String,
    alias: &mut String,
    overwrite: &mut bool,
    run: &mut Option<Pending<CsResult<PlainOutput>>>,
) -> bool {
    if settle(cx, run, view::out_text) == Some(true) {
        return false;
    }
    let slot_num: Option<u32> = slot.parse().ok();
    let slot_taken = slot_num.is_some_and(|n| taken.contains(&n));
    let mut submit = false;
    let mut cancel = false;
    let busy = run.is_some();
    let can_submit = !slot_taken || *overwrite;
    let out = widgets::dialog(
        ctx,
        "add",
        "Add the current Claude login",
        440.0,
        |ui| {
            steps(ui, t, 1, |ui| {
                ui.horizontal_wrapped(|ui| {
                    widgets::line(ui, "Open a terminal anywhere and start Claude Code:", style::body(12.5), t.text_2);
                    widgets::code(ui, "claude");
                });
            });
            steps(ui, t, 2, |ui| {
                ui.horizontal_wrapped(|ui| {
                    widgets::line(ui, "Run", style::body(12.5), t.text_2);
                    widgets::code(ui, "/login");
                    widgets::line(ui, "and finish the sign-in in the browser.", style::body(12.5), t.text_2);
                });
                widgets::wrap_text(
                    ui,
                    "Do not run /logout on the account you are leaving — Claude Code may revoke the token cswap stored for it.",
                    style::body(12.5),
                    t.text_2,
                );
            });
            steps(ui, t, 3, |ui| {
                widgets::wrap_text(
                    ui,
                    "Come back here and add it. cswap stores that login so you can switch back to it later.",
                    style::body(12.5),
                    t.text_2,
                );
            });
            widgets::wrap_text(
                ui,
                "Adding an account that is already managed refreshes its stored credentials instead of creating a duplicate.",
                style::body(12.5),
                t.text_3,
            );
            two_fields(
                ui,
                |ui| {
                    widgets::field(ui, "Slot", Some("Optional. Empty = next free."), false, |ui| {
                        let w = ui.available_width();
                        if widgets::input_opts(ui, "", slot, "auto", false, w, false).changed() {
                            *slot = view::digits_only(slot);
                        }
                    });
                },
                |ui| {
                    widgets::field(ui, "Alias", Some("Optional short name."), false, |ui| {
                        let w = ui.available_width();
                        widgets::input_opts(ui, "", alias, "e.g. work", false, w, false);
                    });
                },
            );
            if let Some(n) = slot_num.filter(|_| slot_taken) {
                slot_warning(ui, t, &format!("Slot {n} is taken. Overwrite it with the current login?"), overwrite);
            }
        },
        |ui| {
            submit = Button::new("Add account")
                .variant(Variant::Primary)
                .loading(busy)
                .enabled(can_submit)
                .show(ui)
                .clicked();
            cancel = Button::new("Cancel").variant(Variant::Ghost).show(ui).clicked();
        },
    );
    if submit {
        let opts = AddAccountOptions {
            slot: slot_num,
            alias: Some(alias.clone()).filter(|s| !s.is_empty()),
            overwrite: *overwrite,
        };
        *run = Some(cx.services.cswap.add_account(opts));
    }
    !(out.close || cancel)
}

#[allow(clippy::too_many_arguments)]
fn token_dialog(
    ctx: &egui::Context,
    cx: &mut Cx,
    t: &Tokens,
    taken: &[u32],
    token: &mut String,
    email: &mut String,
    slot: &mut String,
    show: &mut bool,
    overwrite: &mut bool,
    run: &mut Option<Pending<CsResult<PlainOutput>>>,
) -> bool {
    if settle(cx, run, view::out_text) == Some(true) {
        token.clear();
        return false;
    }
    let slot_num: Option<u32> = slot.parse().ok();
    let slot_taken = slot_num.is_some_and(|n| taken.contains(&n));
    let (mut submit, mut cancel) = (false, false);
    let busy = run.is_some();
    let can_submit = !token.trim().is_empty() && (!slot_taken || *overwrite);
    let out = widgets::dialog(
        ctx,
        "token",
        "Add from a token",
        440.0,
        |ui| {
            widgets::wrap_text(
                ui,
                "A long-lived setup-token (claude setup-token) or a managed API key. The token is handed to cswap over stdin, never as a command-line argument, and is not kept by this app.",
                style::body(12.5),
                t.text_2,
            );
            let hint = view::token_kind(token)
                .map(|k| format!("Detected: {k}"))
                .unwrap_or_else(|| "sk-ant-oat01-… or sk-ant-api03-…".into());
            widgets::field(ui, "Token", Some(&hint), false, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let w = ui.available_width() - 38.0;
                    widgets::input_opts(ui, "", token, "", true, w, !*show);
                    let tip = if *show { "Hide token" } else { "Show token" };
                    if Button::new("")
                        .variant(Variant::Ghost)
                        .icon(if *show { icon::EYE_OFF } else { icon::EYE })
                        .label(tip)
                        .show(ui)
                        .clicked()
                    {
                        *show = !*show;
                    }
                });
            });
            two_fields(
                ui,
                |ui| {
                    widgets::field(ui, "Label (email)", Some("Optional."), false, |ui| {
                        let w = ui.available_width();
                        widgets::input_opts(ui, "", email, "setup-token-N@token.local", false, w, false);
                    });
                },
                |ui| {
                    widgets::field(ui, "Slot", Some("Optional."), false, |ui| {
                        let w = ui.available_width();
                        if widgets::input_opts(ui, "", slot, "auto", false, w, false).changed() {
                            *slot = view::digits_only(slot);
                        }
                    });
                },
            );
            if let Some(n) = slot_num.filter(|_| slot_taken) {
                slot_warning(ui, t, &format!("Slot {n} is taken. Overwrite it?"), overwrite);
            }
        },
        |ui| {
            submit = Button::new("Register token")
                .variant(Variant::Primary)
                .loading(busy)
                .enabled(can_submit)
                .show(ui)
                .clicked();
            cancel = Button::new("Cancel").variant(Variant::Ghost).show(ui).clicked();
        },
    );
    if submit {
        let opts = AddTokenOptions {
            token: token.clone(),
            email: Some(email.clone()).filter(|s| !s.is_empty()),
            slot: slot_num,
            overwrite: *overwrite,
        };
        *run = Some(cx.services.cswap.add_token(opts));
    }
    !(out.close || cancel)
}

fn settle_msg(cx: &mut Cx, run: &mut Option<(Pending<CsResult<PlainOutput>>, String)>) -> Option<bool> {
    let (p, msg) = run.as_mut()?;
    let msg = msg.clone();
    let mut inner = Some(std::mem::replace(p, Pending::ready(Ok(PlainOutput::default()))));
    let r = settle(cx, &mut inner, |_| msg);
    match inner {
        Some(p) => run.as_mut().unwrap().0 = p,
        None => *run = None,
    }
    r
}

fn alias_dialog(
    ctx: &egui::Context,
    cx: &mut Cx,
    t: &Tokens,
    a: &Account,
    alias: &mut String,
    run: &mut Option<(Pending<CsResult<PlainOutput>>, String)>,
) -> bool {
    if settle_msg(cx, run) == Some(true) {
        return false;
    }
    let n = a.number;
    let valid = view::alias_valid(alias);
    let busy = run.is_some();
    let (mut save, mut unset, mut cancel, mut enter) = (false, false, false, false);
    let can_save = !alias.is_empty() && valid && Some(alias.as_str()) != a.alias.as_deref();
    let preview = format!("cswap switch {}", if alias.is_empty() { "dev" } else { alias.as_str() });
    let out = widgets::dialog(
        ctx,
        "alias",
        &format!("Alias for Account-{n}"),
        440.0,
        |ui| {
            ui.horizontal_wrapped(|ui| {
                widgets::line(
                    ui,
                    format!("{}. An alias works anywhere a slot number or email does:", a.email),
                    style::body(12.5),
                    t.text_2,
                );
                widgets::code(ui, &preview);
            });
            widgets::field(ui, "Alias", Some("Letters, digits, . - _ — not purely numeric."), false, |ui| {
                let w = ui.available_width();
                let r = widgets::input_opts(ui, "", alias, "", false, w, false);
                if r.changed() {
                    *alias = alias.trim().to_string();
                }
                enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            });
            if !valid {
                widgets::line(ui, "Invalid alias.", style::body(12.0), t.danger);
            }
        },
        |ui| {
            save = Button::new("Save").variant(Variant::Primary).loading(busy).enabled(can_save).show(ui).clicked();
            cancel = Button::new("Cancel").variant(Variant::Ghost).show(ui).clicked();
            if a.alias.is_some() {
                unset = Button::new("Remove alias").variant(Variant::Ghost).loading(busy).show(ui).clicked();
            }
        },
    );
    if (save || (enter && !alias.is_empty() && valid)) && !busy {
        let p = cx.services.cswap.set_alias(&n.to_string(), Some(alias));
        *run = Some((p, format!("Account-{n} is now “{alias}”")));
    } else if unset && !busy {
        let p = cx.services.cswap.set_alias(&n.to_string(), None);
        *run = Some((p, format!("Alias removed from Account-{n}")));
    }
    !(out.close || cancel)
}

fn move_dialog(
    ctx: &egui::Context,
    cx: &mut Cx,
    t: &Tokens,
    list: &[Account],
    a: &Account,
    slot: &mut String,
    run: &mut Option<(Pending<CsResult<PlainOutput>>, String)>,
) -> bool {
    if settle_msg(cx, run) == Some(true) {
        return false;
    }
    let n: u32 = slot.parse().unwrap_or(0);
    let occupant = list.iter().find(|x| x.number == n && x.number != a.number);
    let busy = run.is_some();
    let (mut go, mut cancel) = (false, false);
    let can_go = !slot.is_empty() && n >= 1 && n != a.number;
    let out = widgets::dialog(
        ctx,
        "move",
        &format!("Move Account-{} to a slot", a.number),
        440.0,
        |ui| {
            widgets::wrap_text(
                ui,
                "Slot numbers are the shortcuts you type (cswap switch 1) and the list order. Aliases, backups and session history move with the account.",
                style::body(12.5),
                t.text_2,
            );
            widgets::field(ui, "Destination slot", None, false, |ui| {
                let w = ui.available_width();
                if widgets::input_opts(ui, "", slot, "", false, w, false).changed() {
                    *slot = view::digits_only(slot);
                }
            });
            if let Some(o) = occupant {
                let name = o.alias.clone().unwrap_or_else(|| o.email.clone());
                widgets::wrap_text(
                    ui,
                    format!("Slot {n} holds {name} — the two will trade places."),
                    style::body(12.0),
                    t.text_2,
                );
            }
        },
        |ui| {
            go = Button::new(if occupant.is_some() { "Swap" } else { "Move" })
                .variant(Variant::Primary)
                .loading(busy)
                .enabled(can_go)
                .show(ui)
                .clicked();
            cancel = Button::new("Cancel").variant(Variant::Ghost).show(ui).clicked();
        },
    );
    if go {
        let msg = if occupant.is_some() {
            format!("Swapped slots {} and {n}", a.number)
        } else {
            format!("Moved to slot {n}")
        };
        *run = Some((cx.services.cswap.move_account(&a.number.to_string(), n), msg));
    }
    !(out.close || cancel)
}

fn remove_dialog(
    ctx: &egui::Context,
    cx: &mut Cx,
    t: &Tokens,
    a: &Account,
    typed: &mut String,
    run: &mut Option<Pending<CsResult<PlainOutput>>>,
) -> bool {
    let n = a.number;
    if settle(cx, run, |_| format!("Removed Account-{n}")) == Some(true) {
        return false;
    }
    let ok = view::remove_confirmed(typed, a);
    let busy = run.is_some();
    let (mut go, mut cancel) = (false, false);
    let out = widgets::dialog(
        ctx,
        "remove",
        "Remove account",
        440.0,
        |ui| {
            let live = if a.active {
                ", but this is the active account — the live login stays until you switch or /logout"
            } else {
                ""
            };
            widgets::wrap_text(
                ui,
                format!(
                    "This deletes the stored login for {} (slot {n}) from cswap. Claude Code itself is not logged out{live}. Recovery means logging in with that account again and re-adding it.",
                    a.email
                ),
                style::body(12.5),
                t.text_2,
            );
            widgets::field(ui, &format!("Type {n} or the email to confirm"), None, false, |ui| {
                let w = ui.available_width();
                widgets::input_opts(ui, "", typed, "", false, w, false);
            });
        },
        |ui| {
            go =
                Button::new("Remove permanently").variant(Variant::Danger).loading(busy).enabled(ok).show(ui).clicked();
            cancel = Button::new("Cancel").variant(Variant::Ghost).show(ui).clicked();
        },
    );
    if go {
        *run = Some(cx.services.cswap.remove_account(&n.to_string()));
    }
    !(out.close || cancel)
}

fn switch_row(ui: &mut Ui, t: &Tokens, title: &str, sub: &str, on: &mut bool, label: &str) {
    let w = ui.available_width();
    egui::Frame::new()
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(crate::theme::RADIUS_MD))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(w - 26.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.set_width(w - 24.0 - 44.0);
                    widgets::line(ui, title, style::body_medium(13.0), t.text);
                    widgets::wrap_text(ui, sub, style::body(12.0), t.text_3);
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| widgets::toggle(ui, on, label, true));
            });
        });
}

fn transfer_settle(
    cx: &mut Cx,
    run: &mut Option<Pending<CsResult<Transfer>>>,
    title: &str,
    fail: &str,
    import: bool,
) -> bool {
    let Some(p) = run.as_mut() else { return false };
    match p.take() {
        Some(Ok(v)) => {
            let body = if import && !v.output.stdout.trim().is_empty() {
                v.output.stdout.trim().to_string()
            } else {
                v.path.clone()
            };
            notify(cx, ToastKind::Success, title, Some(&body));
            *run = None;
            true
        }
        Some(Err(e)) => {
            notify(cx, ToastKind::Error, fail, Some(&e.message));
            *run = None;
            false
        }
        None => {
            if p.is_lost() {
                *run = None;
            }
            false
        }
    }
}

fn export_dialog(
    ctx: &egui::Context,
    cx: &mut Cx,
    t: &Tokens,
    a: Option<&Account>,
    full: &mut bool,
    run: &mut Option<Pending<CsResult<Transfer>>>,
) -> bool {
    if transfer_settle(cx, run, "Exported", "Export failed", false) {
        return false;
    }
    let busy = run.is_some();
    let (mut go, mut cancel) = (false, false);
    let title = a.map(|a| format!("Export Account-{}", a.number)).unwrap_or_else(|| "Export all accounts".into());
    let out = widgets::dialog(
        ctx,
        "export",
        &title,
        440.0,
        |ui| {
            widgets::wrap_text(
                ui,
                "The export is plaintext JSON containing login credentials. Keep it somewhere private, or encrypt it. By default only each account's own login is included — machine-shared MCP/plugin OAuth tokens stay on this machine.",
                style::body(12.5),
                t.text_2,
            );
            switch_row(
                ui,
                t,
                "Full export (same-PC backup)",
                "Include the whole ~/.claude.json and credential object.",
                full,
                "Full export",
            );
        },
        |ui| {
            go = Button::new("Choose file & export").variant(Variant::Primary).loading(busy).show(ui).clicked();
            cancel = Button::new("Cancel").variant(Variant::Ghost).show(ui).clicked();
        },
    );
    if go {
        let name = format!("claude-swap-{}.cswap", &iso_from_ms(now_ms())[..10]);
        // A cancelled save dialog does nothing, as in 0.1.23.
        if let Some(path) = cx.services.host.save_file("Export accounts", &name) {
            let opts = ExportOptions { account: a.map(|a| a.number.to_string()), full: *full };
            *run = Some(cx.services.cswap.export_accounts(opts, &path));
        }
    }
    !(out.close || cancel)
}

fn import_dialog(
    ctx: &egui::Context,
    cx: &mut Cx,
    t: &Tokens,
    force: &mut bool,
    run: &mut Option<Pending<CsResult<Transfer>>>,
) -> bool {
    if transfer_settle(cx, run, "Import finished", "Import failed", true) {
        return false;
    }
    let busy = run.is_some();
    let (mut go, mut cancel) = (false, false);
    let out = widgets::dialog(
        ctx,
        "import",
        "Import accounts",
        440.0,
        |ui| {
            widgets::wrap_text(
                ui,
                "Accounts that already exist are skipped, except slots whose refresh token has died — those are replaced automatically. If the imported account is the one you are logged in as, activate it afterwards with Switch (use --force from the CLI to bypass the backup of the current login).",
                style::body(12.5),
                t.text_2,
            );
            switch_row(
                ui,
                t,
                "Overwrite existing accounts",
                "Passes --force. A stale export can carry an already-superseded token.",
                force,
                "Overwrite existing",
            );
        },
        |ui| {
            go = Button::new("Choose file & import").variant(Variant::Primary).loading(busy).show(ui).clicked();
            cancel = Button::new("Cancel").variant(Variant::Ghost).show(ui).clicked();
        },
    );
    if go {
        let filters: [(&str, &[&str]); 2] = [("cswap export", &["cswap", "json"]), ("All files", &["*"])];
        if let Some(path) = cx.services.host.pick_file("Import accounts", &filters) {
            *run = Some(cx.services.cswap.import_accounts(ImportOptions { force: *force }, &path));
        }
    }
    !(out.close || cancel)
}

fn diag_dialog(
    ctx: &egui::Context,
    t: &Tokens,
    run: &mut Pending<CsResult<PlainOutput>>,
    out_text: &mut Option<Result<String, String>>,
) -> bool {
    if out_text.is_none() {
        match run.take() {
            Some(Ok(v)) => *out_text = Some(Ok(view::strip_ansi(&v.stdout))),
            Some(Err(e)) => *out_text = Some(Err(e.message)),
            None if run.is_lost() => *out_text = Some(Err("No answer from cswap".into())),
            None => {}
        }
    }
    let mut close = false;
    let out = widgets::dialog(
        ctx,
        "diag",
        "Token diagnostics",
        640.0,
        |ui| {
            ui.horizontal_wrapped(|ui| {
                widgets::line(ui, "Output of", style::body(12.5), t.text_2);
                widgets::code(ui, "cswap list --token-status");
                widgets::line(
                    ui,
                    ": where each account's OAuth token comes from and whether it is still valid.",
                    style::body(12.5),
                    t.text_2,
                );
            });
            match out_text {
                None => {
                    widgets::line(ui, "Running…", style::body(12.0), t.text_3);
                }
                Some(Err(e)) => {
                    widgets::wrap_text(ui, e.clone(), style::body(12.0), t.danger);
                }
                Some(Ok(s)) => {
                    let w = ui.available_width();
                    egui::Frame::new()
                        .fill(t.bg)
                        .stroke(Stroke::new(1.0, t.border))
                        .corner_radius(CornerRadius::same(crate::theme::RADIUS_MD))
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.set_width(w - 26.0);
                            egui::ScrollArea::vertical().max_height(ctx.content_rect().height() * 0.46).show(
                                ui,
                                |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(s.trim()).font(style::mono(12.0)).color(t.text_2),
                                        )
                                        .selectable(true)
                                        .wrap(),
                                    );
                                },
                            );
                        });
                }
            }
        },
        |ui| {
            close = Button::new("Close").show(ui).clicked();
        },
    );
    !(out.close || close)
}

#[allow(clippy::too_many_arguments)]
fn run_dialog(
    ctx: &egui::Context,
    cx: &mut Cx,
    t: &Tokens,
    a: &Account,
    dir: &mut String,
    mappings: &mut Vec<Mapping>,
    load: &mut Option<Pending<CsResult<Vec<Mapping>>>>,
    run: &mut Option<Pending<CsResult<LaunchedSession>>>,
) -> bool {
    if let Some(p) = load.as_mut() {
        match p.take() {
            Some(Ok(m)) => {
                // A directory already mapped to this account is the one you almost always mean.
                if let Some(mine) = m.iter().find(|m| m.account.as_ref().is_some_and(|x| x.number == a.number)) {
                    *dir = mine.path.clone();
                }
                *mappings = m;
                *load = None;
            }
            Some(Err(_)) => *load = None,
            None if p.is_lost() => *load = None,
            None => {}
        }
    }
    if let Some(p) = run.as_mut() {
        match p.take() {
            Some(Ok(v)) => {
                notify(cx, ToastKind::Success, &format!("Opened {}", v.command), Some(&v.cwd));
                return false;
            }
            Some(Err(e)) => {
                notify(cx, ToastKind::Error, "Could not open a terminal", Some(&e.message));
                *run = None;
            }
            None if p.is_lost() => *run = None,
            None => {}
        }
    }
    let busy = run.is_some();
    let (mut go, mut cancel, mut browse) = (false, false, false);
    let out = widgets::dialog(
        ctx,
        "run",
        &format!("Open a terminal as Account-{}", a.number),
        520.0,
        |ui| {
            let who = a.alias.clone().unwrap_or_else(|| a.email.clone());
            widgets::wrap_text(
                ui,
                format!(
                    "Session mode: Claude Code runs as {who} in that terminal only. Your default login and every other terminal are untouched."
                ),
                style::body(12.5),
                t.text_2,
            );
            widgets::field(ui, "Start in", Some("Leave empty for your home directory."), false, |ui| {
                ui.horizontal(|ui| {
                    let b = Button::new("Browse").icon(icon::FOLDER_OPEN);
                    let w = ui.available_width() - b.width(ui) - 8.0;
                    widgets::input_opts(ui, "", dir, "~", true, w, false);
                    browse = b.show(ui).clicked();
                });
            });
            if !mappings.is_empty() {
                widgets::field(
                    ui,
                    "Or pick a mapped directory",
                    Some("Directories you have mapped with cswap map."),
                    false,
                    |ui| {
                        let mut opts: Vec<(String, String)> = vec![(String::new(), "—".to_owned())];
                        opts.extend(mappings.iter().map(|m| {
                            let label = match &m.account {
                                Some(x) => {
                                    format!("{} — {}", m.path, x.alias.clone().unwrap_or_else(|| x.email.clone()))
                                }
                                None => m.path.clone(),
                            };
                            (m.path.clone(), label)
                        }));
                        let mut v = if mappings.iter().any(|m| &m.path == dir) { dir.clone() } else { String::new() };
                        let w = ui.available_width();
                        if widgets::select(ui, "Or pick a mapped directory", &mut v, &opts, w) {
                            *dir = v;
                        }
                    },
                );
            }
            let w = ui.available_width();
            egui::Frame::new()
                .fill(t.bg)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(crate::theme::RADIUS_MD))
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(w - 26.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let shown = if dir.trim().is_empty() { "~".to_string() } else { dir.trim().to_string() };
                        widgets::line(ui, format!("{shown}$ "), style::mono(12.0), t.text_3);
                        widgets::line(ui, format!("cswap run {}", a.number), style::mono(12.0), t.text_2);
                    });
                });
        },
        |ui| {
            go = Button::new("Open terminal")
                .variant(Variant::Primary)
                .icon(icon::SQUARE_TERMINAL)
                .loading(busy)
                .show(ui)
                .clicked();
            cancel = Button::new("Cancel").variant(Variant::Ghost).show(ui).clicked();
        },
    );
    if browse && let Some(p) = cx.services.host.pick_directory("Start in") {
        *dir = p;
    }
    if go {
        let d = dir.trim().to_string();
        *run = Some(cx.services.cswap.launch_session(&a.number.to_string(), (!d.is_empty()).then_some(d.as_str())));
    }
    !(out.close || cancel)
}
