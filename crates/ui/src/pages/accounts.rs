//! Port of `src/renderer/src/pages/Accounts.tsx`. OWNER: ui-accounts.
//!
//! Rules born from Purwa using 0.1.x (project notes, "Halaman Accounts"):
//! - the page's shape follows the width of the LIST, not of the window: cards below a
//!   FIXED 780 px ([`view::CARD_BELOW`]), a table above;
//! - window columns are generated: 5-hour, 7-day, then one per model ANY account reports;
//!   they belong to the table, so a row without that model gets an empty `-` cell
//!   (plain hyphen, vertically centred), and the header names the column instead of a
//!   name inside each bar. Cards drop the cell instead;
//! - between 780 px and the template's minimum (≈956 px) only the list scrolls sideways.

use egui::{Align, Color32, CornerRadius, Id, Layout, Margin, Rect, Sense, Shape, Stroke, Ui, UiBuilder, pos2, vec2};

use cswap_core::format::{
    age_seconds, iso_from_ms, mask_email, now_ms, org_tag, parse_iso_ms, pct, reset_text,
    status_label, status_tone, tone,
};
use cswap_core::view::{self, ChipTone, WindowCell};
use cswap_core::{
    Account, AccountsState, AddAccountOptions, AddTokenOptions, AppSettings, CsResult, Event, ExportOptions,
    ImportOptions, LaunchedSession, Mapping, Pending, PlainOutput, PlanInfo, RotateStrategy, SwitchPayload,
    Transfer, UsageView,
};

use super::kit::{self, Btn, Icon, Variant, item, item_hint};
use crate::cx::Cx;
use crate::theme::Tokens;
use crate::toast::ToastKind;
use crate::usage;

/// Local UTC offset in seconds, for reset clocks.
pub(crate) fn local_offset() -> i64 {
    chrono::Local::now().offset().local_minus_utc() as i64
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
    Add { slot: String, alias: String, overwrite: bool, run: Option<Pending<CsResult<PlainOutput>>> },
    Token { token: String, email: String, slot: String, show: bool, overwrite: bool, run: Option<Pending<CsResult<PlainOutput>>> },
    Alias { a: Account, alias: String, run: Option<(Pending<CsResult<PlainOutput>>, String)> },
    Move { a: Account, slot: String, run: Option<(Pending<CsResult<PlainOutput>>, String)> },
    Remove { a: Account, typed: String, run: Option<Pending<CsResult<PlainOutput>>> },
    Export { a: Option<Account>, full: bool, run: Option<Pending<CsResult<Transfer>>> },
    Import { force: bool, run: Option<Pending<CsResult<Transfer>>> },
    Diag { run: Pending<CsResult<PlainOutput>>, out: Option<Result<String, String>> },
    Run { a: Account, dir: String, mappings: Vec<Mapping>, load: Option<Pending<CsResult<Vec<Mapping>>>>, run: Option<Pending<CsResult<LaunchedSession>>> },
}

enum RowAct {
    Switch,
    Menu(usize),
}

pub struct AccountsPage {
    /// Fixed clock (Unix ms) for tests and screenshots; the wall clock otherwise.
    pub now_ms: Option<i64>,
    /// Fixed UTC offset (seconds) for tests; the OS's otherwise.
    pub utc_offset: Option<i64>,
    ops: Vec<Op>,
    dlg: Option<Dlg>,
    /// What the last frame laid out, for tests: list width and whether it was cards.
    pub last_list_width: f32,
    pub last_cards: bool,
}

impl Default for AccountsPage {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountsPage {
    pub fn new() -> Self {
        Self { now_ms: None, utc_offset: None, ops: Vec::new(), dlg: None, last_list_width: 0.0, last_cards: false }
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
            self.header(ui, cx, &t, &list, &settings);
            egui::Frame::new().inner_margin(Margin { left: 24, right: 24, top: 0, bottom: 32 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.body(ui, cx, &t, &state, &list, &settings);
            });
        });
        self.dialogs(ui.ctx(), cx, &t, &list);
    }

    fn header(&mut self, ui: &mut Ui, cx: &mut Cx, t: &Tokens, list: &[Account], s: &AppSettings) {
        let rings = s.usage_view == UsageView::Rings;
        let active = list.iter().find(|a| a.active);
        let rotate = Btn::new("Rotate").icon(Icon::Shuffle).trailing(Icon::Chevron).loading(self.busy("rotate"));
        let add = Btn::new("Add account").variant(Variant::Primary).icon(Icon::Plus).trailing(Icon::Chevron);
        let actions_w = 32.0 * 4.0 + rotate.width(ui) + add.width(ui) + 8.0 * 5.0;
        let mask = s.mask_emails;
        let mut picked: (Option<usize>, Option<usize>, Option<usize>) = (None, None, None);
        let mut toggles = (false, false, false);
        kit::page_header(
            ui,
            t,
            "Accounts",
            |ui| {
                if let Some(a) = active {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        kit::text(ui, "Active: ", 12.5, t.text_2);
                        let name = a.alias.clone().unwrap_or_else(|| mask_email(&a.email, mask));
                        kit::text(ui, name, 12.5, t.text);
                        if a.is_organization {
                            kit::text(ui, format!(" · {}", org_tag(a, mask)), 12.5, t.text_3);
                        }
                    });
                } else if !list.is_empty() {
                    kit::text(ui, "No active managed account", 12.5, t.text_2);
                }
            },
            actions_w,
            |ui| {
                // right-to-left: the last control first
                let more = Btn::new("").variant(Variant::Ghost).icon(Icon::Ellipsis).a11y("More").show(ui, t);
                picked.2 = kit::menu(
                    ui,
                    t,
                    &more,
                    260.0,
                    &[item("Export all accounts"), item("Import accounts"), None, item_hint("Token diagnostics", "--token-status")],
                );
                let add_r = add.show(ui, t);
                picked.1 = kit::menu(ui, t, &add_r, 240.0, &[item("From current Claude login"), item("From setup-token or API key")]);
                let rot = rotate.show(ui, t);
                picked.0 = kit::menu(
                    ui,
                    t,
                    &rot,
                    260.0,
                    &[item_hint("Next account", "rotation"), item_hint("Best (most quota left)", "best"), item("Next available (skip limited)")],
                );
                let (m_icon, m_tip) = if mask {
                    (Icon::EyeOff, "Show email addresses")
                } else {
                    (Icon::Eye, "Hide email addresses (keeps the first three characters)")
                };
                toggles.2 = Btn::new("").variant(Variant::Ghost).icon(m_icon).a11y(m_tip).show(ui, t).on_hover_text(m_tip).clicked();
                let (p_icon, p_tip) = if s.show_pace {
                    (Icon::Gauge, "Hide the pace marker")
                } else {
                    (Icon::GaugeOff, "Show where an evenly-spread week would be")
                };
                toggles.1 = Btn::new("").variant(Variant::Ghost).icon(p_icon).a11y(p_tip).show(ui, t).on_hover_text(p_tip).clicked();
                let (v_icon, v_tip) = if rings { (Icon::Bars, "Show usage as bars") } else { (Icon::Ring, "Show usage as rings") };
                toggles.0 = Btn::new("").variant(Variant::Ghost).icon(v_icon).a11y(v_tip).show(ui, t).on_hover_text(v_tip).clicked();
            },
        );
        if toggles.0 {
            let v = if rings { UsageView::Bars } else { UsageView::Rings };
            cx.services.cswap.edit_settings(Box::new(move |s| s.usage_view = v));
        }
        if toggles.1 {
            let v = !s.show_pace;
            cx.services.cswap.edit_settings(Box::new(move |s| s.show_pace = v));
        }
        if toggles.2 {
            let v = !mask;
            cx.services.cswap.edit_settings(Box::new(move |s| s.mask_emails = v));
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
        self.dlg = Some(Dlg::Token { token: String::new(), email: String::new(), slot: String::new(), show: false, overwrite: false, run: None });
    }

    fn body(&mut self, ui: &mut Ui, cx: &mut Cx, t: &Tokens, state: &AccountsState, list: &[Account], s: &AppSettings) {
        ui.spacing_mut().item_spacing = vec2(8.0, 12.0);
        if let Some(e) = &state.error {
            if state.payload.is_none() {
                kit::notice(ui, t, &e.message, ChipTone::Danger);
            } else {
                kit::notice(ui, t, &format!("Showing the last successful list — refresh failed: {}", e.message), ChipTone::Warn);
            }
        }
        if state.payload.is_none() && state.error.is_none() {
            // Loading: three placeholder rows.
            for _ in 0..3 {
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 62.0), Sense::hover());
                ui.painter().rect(r, CornerRadius::same(8), t.surface, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
            }
            return;
        }
        if state.payload.is_some() && list.is_empty() {
            let mut add = false;
            kit::empty(
                ui,
                t,
                "No managed accounts yet",
                "Log into Claude Code with the account you want to keep, then add it here. cswap backs up its login so you can switch back to it any time.",
                |ui| add = Btn::new("Add current login").variant(Variant::Primary).icon(Icon::Plus).show(ui, t).clicked(),
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
        let cards = view::is_cards(width);
        self.last_list_width = width;
        self.last_cards = cards;
        let models = view::models(list);
        let rings = s.usage_view == UsageView::Rings;
        let min_w = view::table_min_width(models.len(), rings);
        let mut acts: Vec<(Account, RowAct)> = Vec::new();
        if !cards && width < min_w {
            // Only the list scrolls sideways; the page and the window never do.
            egui::ScrollArea::horizontal().id_salt("accounts-hscroll").auto_shrink([false, true]).show(ui, |ui| {
                self.list(ui, cx, t, state, list, s, &models, min_w, false, &mut acts);
            });
        } else {
            self.list(ui, cx, t, state, list, s, &models, width, cards, &mut acts);
        }
        for (a, act) in acts {
            self.row_action(cx, a, act);
        }
        // footer line
        if let Some(f) = state.fetched_at.as_deref().and_then(parse_iso_ms) {
            let age = age_seconds(Some((self.now() - f) as f64 / 1000.0));
            let left = format!("{} · usage refreshed {age}", view::count_text(list.len()));
            let right = "Every write goes through the cswap CLI — nothing here edits the vault directly.";
            let w = ui.available_width();
            let lw = kit::measure(ui, &left, kit::font(11.5)).x;
            let rw = kit::measure(ui, right, kit::font(11.5)).x;
            ui.add_space(-4.0);
            if lw + rw + 16.0 <= w {
                let (r, _) = ui.allocate_exact_size(vec2(w, 16.0), Sense::hover());
                let gl = kit::galley(ui, left, kit::font(11.5), t.text_3, w);
                ui.painter().galley(r.min, gl, t.text_3);
                let gr = kit::galley(ui, right, kit::font(11.5), t.text_3, w);
                ui.painter().galley(pos2(r.max.x - rw, r.min.y), gr, t.text_3);
            } else {
                ui.spacing_mut().item_spacing.y = 2.0;
                kit::text(ui, left, 11.5, t.text_3);
                kit::para(ui, right, 11.5, t.text_3);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn list(
        &mut self,
        ui: &mut Ui,
        cx: &Cx,
        t: &Tokens,
        state: &AccountsState,
        list: &[Account],
        s: &AppSettings,
        models: &[String],
        width: f32,
        cards: bool,
        acts: &mut Vec<(Account, RowAct)>,
    ) {
        let rings = s.usage_view == UsageView::Rings;
        let cols = view::grid_columns(width, models.len(), rings);
        kit::card_frame(t).show(ui, |ui| {
            ui.set_width(width - 2.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            if !cards {
                header_row(ui, t, &cols, models);
            }
            for (i, a) in list.iter().enumerate() {
                let last = i + 1 == list.len();
                let plan = state.plans.get(&a.email);
                let act = if cards {
                    self.card_row(ui, cx, t, a, plan, models, s, last)
                } else {
                    self.table_row(ui, cx, t, a, plan, models, s, &cols, last)
                };
                if let Some(act) = act {
                    acts.push((a.clone(), act));
                }
            }
        });
    }

    fn row_action(&mut self, cx: &mut Cx, a: Account, act: RowAct) {
        let n = a.number;
        match act {
            RowAct::Switch => self.switch_to(cx, n),
            RowAct::Menu(0) => self.dlg = Some(Dlg::Alias { alias: a.alias.clone().unwrap_or_default(), a, run: None }),
            RowAct::Menu(1) => self.dlg = Some(Dlg::Move { a, slot: String::new(), run: None }),
            RowAct::Menu(2) => {
                let disabled = a.disabled == Some(true);
                let ok = if disabled { format!("Account-{n} back in rotation") } else { format!("Account-{n} held out of rotation") };
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

    /// Switch + row menu. Returns the action picked.
    fn actions(&self, ui: &mut Ui, t: &Tokens, a: &Account) -> Option<RowAct> {
        let mut out = None;
        let n = a.number;
        let disabled = a.disabled == Some(true);
        let menu_label = format!("Actions for account {n}");
        let m = Btn::new("").small().variant(Variant::Ghost).icon(Icon::Ellipsis).a11y(&menu_label).show(ui, t);
        let run_hint = format!("cswap run {n}");
        let slot_hint = format!("slot {n}");
        let items = [
            item(if a.alias.is_some() { "Change alias" } else { "Set alias" }),
            item("Move to slot"),
            item(if disabled { "Enable (back in rotation)" } else { "Disable (hold out of rotation)" }),
            None,
            item_hint("Open terminal as this account", &run_hint),
            item("Export this account"),
            item_hint("Re-add from the current login", &slot_hint),
            None,
            Some(kit::MenuItem { label: "Remove account", hint: None, danger: true }),
        ];
        if let Some(i) = kit::menu(ui, t, &m, 280.0, &items) {
            out = Some(RowAct::Menu(i));
        }
        // The active row keeps the button's space (columns must not move) but hides it.
        let label = format!("Switch to account {n}");
        let b = Btn::new("Switch").small().variant(if a.active { Variant::Ghost } else { Variant::Default }).loading(self.busy(&format!("switch-{n}"))).a11y(&label);
        if a.active {
            let w = b.width(ui);
            ui.add_space(w);
        } else if b.show(ui, t).clicked() {
            out = Some(RowAct::Switch);
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn table_row(
        &mut self,
        ui: &mut Ui,
        cx: &Cx,
        t: &Tokens,
        a: &Account,
        plan: Option<&PlanInfo>,
        models: &[String],
        s: &AppSettings,
        cols: &[f32],
        last: bool,
    ) -> Option<RowAct> {
        let _ = cx;
        let (now, off) = (self.now(), self.offset());
        let rings = s.usage_view == UsageView::Rings;
        let top_left = ui.cursor().min;
        let width = ui.available_width();
        let bg = ui.painter().add(Shape::Noop);
        let mut xs = Vec::with_capacity(cols.len());
        let mut x = top_left.x + view::ROW_PAD_X;
        for w in cols {
            xs.push(x);
            x += w + view::COL_GAP;
        }
        let y0 = top_left.y + 10.0;
        let mut row = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(top_left, vec2(width, 10_000.0))).id_salt(("acct-row", a.number)));
        if a.disabled == Some(true) {
            row.multiply_opacity(0.7);
        }
        let cell = |row: &mut Ui, i: usize, f: &mut dyn FnMut(&mut Ui)| -> f32 {
            kit::place(&mut row, UiBuilder::new().max_rect(Rect::from_min_size(pos2(xs[i], y0), vec2(cols[i], 10_000.0))), |ui| {
                ui.set_width(cols[i]);
                f(ui)
            })
            .response
            .rect
            .height()
        };
        // top-aligned cells first (the row is `items-start`), to learn the row's height
        let mut h = 19.0f32;
        h = h.max(cell(&mut row, 1, &mut |ui| {
            name_block(ui, t, a, plan, s.mask_emails, now, off);
        }));
        let cells = view::window_cells(a, models, false);
        let mut empty_cols = Vec::new();
        for (k, c) in cells.iter().enumerate() {
            let col = 2 + k;
            match &c.window {
                Some(_) => {
                    let ch = cell(&mut row, col, &mut |ui| window_cell(ui, t, a.number, c, false, rings, s.show_pace, now, off));
                    h = h.max(ch);
                }
                None => empty_cols.push(col),
            }
        }
        // stretched + vertically centred cells
        let mid = y0 + h / 2.0;
        let p = row.painter().clone();
        let dot = if a.active { t.accent } else { Color32::TRANSPARENT };
        p.circle_filled(pos2(xs[0] + 3.0, mid), 3.0, dot);
        p.text(pos2(xs[0] + 12.0, mid), egui::Align2::LEFT_CENTER, a.number.to_string(), kit::mono(12.0), t.text_3);
        for col in empty_cols {
            // A plain hyphen, not an em dash (Purwa, 10 Sep), centred in the stretched cell.
            p.text(pos2(xs[col], mid), egui::Align2::LEFT_CENTER, "-", kit::font(12.0), t.text_3);
            // The empty cell is a node of its own, named like a filled one, so a test can
            // point at the cell that holds the column.
            let key = &cells[col - 2].key;
            let r = row.interact(Rect::from_min_size(pos2(xs[col], y0), vec2(cols[col], h)), Id::unique(("window", a.number, key)), Sense::hover());
            let name = format!("{} {key}: -", a.number);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, &name));
        }
        let si = cols.len() - 2;
        let status_h = status_height(a);
        kit::place(&mut row, UiBuilder::new().max_rect(Rect::from_min_size(pos2(xs[si], mid - status_h / 2.0), vec2(cols[si], status_h))), |ui| {
            status_block(ui, t, a)
        });
        let ai = cols.len() - 1;
        let act = kit::place(&mut row, 
                UiBuilder::new()
                    .max_rect(Rect::from_min_size(pos2(xs[ai], mid - 14.0), vec2(cols[ai], 28.0)))
                    .layout(Layout::right_to_left(Align::Center)),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    self.actions(ui, t, a)
                },
            )
            .inner;
        let rect = Rect::from_min_size(top_left, vec2(width, h + 20.0));
        let hovered = ui.rect_contains_pointer(rect);
        let fill = match (a.active, hovered) {
            (true, false) => t.accent_soft.gamma_multiply(0.4),
            (true, true) => t.accent_soft.gamma_multiply(0.5),
            (false, true) => t.surface_2.gamma_multiply(0.5),
            (false, false) => Color32::TRANSPARENT,
        };
        ui.painter().set(bg, egui::epaint::RectShape::filled(rect, CornerRadius::ZERO, fill));
        if !last {
            ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, t.border));
        }
        ui.allocate_rect(rect, Sense::hover());
        act
    }

    #[allow(clippy::too_many_arguments)]
    fn card_row(
        &mut self,
        ui: &mut Ui,
        cx: &Cx,
        t: &Tokens,
        a: &Account,
        plan: Option<&PlanInfo>,
        models: &[String],
        s: &AppSettings,
        last: bool,
    ) -> Option<RowAct> {
        let _ = cx;
        let (now, off) = (self.now(), self.offset());
        let rings = s.usage_view == UsageView::Rings;
        let top_left = ui.cursor().min;
        let width = ui.available_width();
        let bg = ui.painter().add(Shape::Noop);
        let mut act = None;
        let inner = kit::place(ui, UiBuilder::new().max_rect(Rect::from_min_size(top_left, vec2(width, 10_000.0))).id_salt(("acct-card", a.number)), |ui| {
                if a.disabled == Some(true) {
                    ui.multiply_opacity(0.7);
                }
                egui::Frame::new().inner_margin(Margin::same(12)).show(ui, |ui| {
                    let w = width - 24.0;
                    ui.set_width(w);
                    ui.spacing_mut().item_spacing = vec2(10.0, 12.0);
                    // top line: number, name, actions
                    let top = ui.cursor().min;
                    let num_w = 6.0 + 6.0 + kit::measure(ui, &a.number.to_string(), kit::mono(12.0)).x + 10.0;
                    let act_w = 104.0;
                    let p = ui.painter().clone();
                    p.circle_filled(pos2(top.x + 3.0, top.y + 9.0), 3.0, if a.active { t.accent } else { Color32::TRANSPARENT });
                    p.text(pos2(top.x + 12.0, top.y + 9.0), egui::Align2::LEFT_CENTER, a.number.to_string(), kit::mono(12.0), t.text_3);
                    let name_w = (w - num_w - act_w).max(60.0);
                    let nh = kit::place(ui, UiBuilder::new().max_rect(Rect::from_min_size(pos2(top.x + num_w, top.y), vec2(name_w, 400.0))), |ui| {
                            ui.set_width(name_w);
                            name_block(ui, t, a, plan, s.mask_emails, now, off)
                        })
                        .inner;
                    act = kit::place(ui, 
                            UiBuilder::new()
                                .max_rect(Rect::from_min_size(pos2(top.x + w - act_w, top.y - 4.0), vec2(act_w, 28.0)))
                                .layout(Layout::right_to_left(Align::Center)),
                            |ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                self.actions(ui, t, a)
                            },
                        )
                        .inner;
                    ui.allocate_space(vec2(w, nh.max(20.0)));
                    // windows; models this account lacks are absent, not dashes
                    let cells = view::window_cells(a, models, true);
                    if rings {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(16.0, 8.0);
                            for c in &cells {
                                window_cell(ui, t, a.number, c, true, true, s.show_pace, now, off);
                            }
                        });
                    } else {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 10.0;
                            for c in &cells {
                                window_cell(ui, t, a.number, c, true, false, s.show_pace, now, off);
                            }
                        });
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(8.0, 4.0);
                        status_inline(ui, t, a);
                    });
                });
            })
            .response;
        let rect = Rect::from_min_size(top_left, vec2(width, inner.rect.height()));
        let fill = if a.active { t.accent_soft.gamma_multiply(0.4) } else { Color32::TRANSPARENT };
        ui.painter().set(bg, egui::epaint::RectShape::filled(rect, CornerRadius::ZERO, fill));
        if !last {
            ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, t.border));
        }
        ui.allocate_rect(rect, Sense::hover());
        act
    }

    // ---- dialogs ---------------------------------------------------------------------

    fn dialogs(&mut self, ctx: &egui::Context, cx: &mut Cx, t: &Tokens, list: &[Account]) {
        let Some(mut d) = self.dlg.take() else { return };
        let taken: Vec<u32> = list.iter().map(|a| a.number).collect();
        let keep = match &mut d {
            Dlg::Add { slot, alias, overwrite, run } => add_dialog(ctx, cx, t, &taken, slot, alias, overwrite, run),
            Dlg::Token { token, email, slot, show, overwrite, run } => token_dialog(ctx, cx, t, &taken, token, email, slot, show, overwrite, run),
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

fn header_row(ui: &mut Ui, t: &Tokens, cols: &[f32], models: &[String]) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, t.bg.gamma_multiply(0.5));
    p.hline(r.x_range(), r.max.y - 0.5, Stroke::new(1.0, t.border));
    let mut labels: Vec<String> = vec!["#".into(), "ACCOUNT".into(), "5-HOUR WINDOW".into(), "7-DAY WINDOW".into()];
    labels.extend(models.iter().map(|m| m.to_uppercase()));
    labels.extend(["STATUS".into(), String::new()]);
    let mut x = r.min.x + view::ROW_PAD_X;
    for (i, (l, cw)) in labels.iter().zip(cols).enumerate() {
        let lx = if i == 0 { x + 13.0 } else { x };
        let g = kit::galley(ui, l.clone(), kit::font(11.0), t.text_3, *cw);
        let resp = ui.interact(Rect::from_min_size(pos2(lx, r.min.y), vec2(g.size().x, 30.0)), Id::unique(("col-head", i)), Sense::hover());
        if i >= 4 && i < 4 + models.len() {
            let _ = resp.on_hover_text(format!("{} window", models[i - 4]));
        }
        ui.painter().galley(pos2(lx, r.center().y - g.size().y / 2.0), g, t.text_3);
        x += cw + view::COL_GAP;
    }
}

/// Name, badges and the line under it. Returns the height used.
fn name_block(ui: &mut Ui, t: &Tokens, a: &Account, plan: Option<&PlanInfo>, mask: bool, now: i64, off: i64) -> f32 {
    let w = ui.available_width();
    let top = ui.cursor().min;
    let name = a.alias.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| mask_email(&a.email, mask));
    let mut chips: Vec<(String, ChipTone, Option<String>)> = Vec::new();
    if let Some(p) = plan {
        chips.push((p.label.clone(), ChipTone::Muted, Some(view::plan_tooltip(p, a.active, now, off))));
    }
    if a.active {
        chips.push(("active".into(), ChipTone::Accent, None));
    }
    if a.disabled == Some(true) {
        chips.push(("disabled".into(), ChipTone::Muted, Some("Held out of rotation".into())));
    }
    // flex-wrap: the name keeps its width and the badges wrap under it; only a name wider
    // than the cell is truncated.
    let g = kit::galley(ui, name, kit::font(13.0), t.text, w);
    let name_rect = Rect::from_min_size(top, vec2(g.size().x, 18.0));
    ui.painter().galley(pos2(top.x, top.y + 9.0 - g.size().y / 2.0), g, t.text);
    if !mask {
        let _ = ui.interact(name_rect, Id::unique(("acct-name", a.number)), Sense::hover()).on_hover_text(&a.email);
    }
    let (mut x, mut y) = (name_rect.max.x + 6.0, top.y);
    for (i, (label, tone, tip)) in chips.iter().enumerate() {
        let cw = kit::chip_width(ui, label);
        if x + cw > top.x + w {
            x = top.x;
            y += 24.0;
        }
        let r = kit::place(ui, UiBuilder::new().max_rect(Rect::from_min_size(pos2(x, y), vec2(cw, 18.0))), |ui| kit::chip(ui, t, label, *tone))
            .inner;
        if let Some(tip) = tip {
            let _ = ui.interact(r.rect, Id::unique(("acct-chip", a.number, i)), Sense::hover()).on_hover_text(tip);
        }
        x += cw + 6.0;
    }
    // second line: (email ·) org
    let y2 = y + 20.0;
    let mut line = String::new();
    if a.alias.as_deref().is_some_and(|s| !s.is_empty()) {
        line.push_str(&mask_email(&a.email, mask));
        line.push_str("  ·  ");
    }
    line.push_str(&org_tag(a, mask));
    let g2 = kit::galley(ui, line, kit::font(11.5), t.text_3, w);
    ui.painter().galley(pos2(top.x, y2), g2, t.text_3);
    let h = y2 + 16.0 - top.y;
    ui.allocate_space(vec2(w, h));
    h
}

/// A filled window cell: bars (percentage, reset, track) or a ring.
#[allow(clippy::too_many_arguments)]
fn window_cell(ui: &mut Ui, t: &Tokens, n: u32, c: &WindowCell, card: bool, rings: bool, pace: bool, now: i64, off: i64) {
    let id = Id::unique(("window", n, &c.key));
    let Some(w) = &c.window else {
        let r = kit::text(ui, "-", 12.0, t.text_3);
        let name = format!("{n} {}: -", c.key);
        r.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, &name));
        return;
    };
    let tip = view::window_tooltip(w, &c.tooltip_name, now, off);
    let reset = w.resets_at.as_deref().map(|r| reset_text(Some(r), now));
    let pace = if pace { w.expected_pct } else { None };
    let tn = tone(Some(w.pct));
    let a11y = format!("{n} {}: {}", c.key, pct(Some(w.pct)));
    if rings {
        let label = card.then_some(c.card_name.as_str());
        let r = usage::ring(ui, t, id, Some(w.pct), pace, label, reset.as_deref(), ui.available_width());
        r.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, &a11y));
        let _ = r.on_hover_text(tip);
        return;
    }
    let width = ui.available_width();
    let (top, _) = ui.allocate_exact_size(vec2(width, 16.0), Sense::hover());
    let p = ui.painter().clone();
    // right side first: ("ahead") reset
    let mut right = top.max.x;
    if let Some(r) = &reset {
        let g = kit::galley(ui, r.clone(), kit::font(12.0), t.text_3, width * 0.5);
        right -= g.size().x;
        p.galley(pos2(right, top.center().y - g.size().y / 2.0), g, t.text_3);
        right -= 4.0;
    }
    if !c.is_model && w.ahead_of_pace == Some(true) {
        let cw = kit::chip_width(ui, "ahead");
        let cr = Rect::from_min_size(pos2(right - cw, top.center().y - 9.0), vec2(cw, 18.0));
        kit::place(ui, UiBuilder::new().max_rect(cr), |ui| kit::chip(ui, t, "ahead", ChipTone::Warn));
        right -= cw + 4.0;
    }
    let mut x = top.min.x;
    if card {
        let g = kit::galley(ui, format!("{} ", c.card_name), kit::font(12.0), t.text_3, (right - x - 8.0).max(10.0));
        let gw = g.size().x;
        p.galley(pos2(x, top.center().y - g.size().y / 2.0), g, t.text_3);
        x += gw;
    }
    let g = kit::galley(ui, pct(Some(w.pct)), kit::font(12.0), usage::pct_color(t, tn), (right - x - 4.0).max(10.0));
    p.galley(pos2(x, top.center().y - g.size().y / 2.0), g, t.text);
    ui.add_space(4.0);
    usage::bar(ui, t, id, Some(w.pct), tn, pace, width);
    let whole = Rect::from_min_max(top.min, pos2(top.max.x, top.max.y + 9.0));
    let r = ui.interact(whole, id.with("tip"), Sense::hover());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, &a11y));
    let _ = r.on_hover_text(tip);
}

fn status_height(a: &Account) -> f32 {
    let extra = view::is_stale(a) || a.usage_age_seconds.is_some_and(|s| s >= 60.0);
    if extra { 18.0 + 4.0 + 14.0 } else { 18.0 }
}

fn status_extra(a: &Account) -> Option<(String, Option<&'static str>)> {
    if view::is_stale(a) {
        Some((format!("last good {}", age_seconds(a.last_good_age_seconds)), Some("Last successful usage measurement")))
    } else {
        a.usage_age_seconds.filter(|s| *s >= 60.0).map(|s| (age_seconds(Some(s)), None))
    }
}

fn status_block(ui: &mut Ui, t: &Tokens, a: &Account) {
    ui.spacing_mut().item_spacing.y = 4.0;
    let r = kit::chip(ui, t, &status_label(&a.usage_status), status_tone(&a.usage_status).into());
    let _ = r.on_hover_text(&a.usage_status);
    if let Some((s, tip)) = status_extra(a) {
        let r = kit::text(ui, s, 11.0, t.text_3);
        if let Some(tip) = tip {
            let _ = r.on_hover_text(tip);
        }
    }
}

fn status_inline(ui: &mut Ui, t: &Tokens, a: &Account) {
    let r = kit::chip(ui, t, &status_label(&a.usage_status), status_tone(&a.usage_status).into());
    let _ = r.on_hover_text(&a.usage_status);
    if let Some((s, _)) = status_extra(a) {
        kit::text(ui, s, 11.0, t.text_3);
    }
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
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, n.to_string(), kit::font(11.0), t.text_2);
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
            ui.set_width(w - 24.0);
            ui.horizontal(|ui| {
                kit::text(ui, text, 12.5, t.warn);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| kit::switch(ui, t, overwrite, "Overwrite slot", true));
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
    let out = kit::dialog(
        ctx,
        t,
        "add",
        "Add the current Claude login",
        440.0,
        |ui| {
            steps(ui, t, 1, |ui| {
                ui.horizontal_wrapped(|ui| {
                    kit::text(ui, "Open a terminal anywhere and start Claude Code:", 12.5, t.text_2);
                    kit::code(ui, t, "claude");
                });
            });
            steps(ui, t, 2, |ui| {
                ui.horizontal_wrapped(|ui| {
                    kit::text(ui, "Run", 12.5, t.text_2);
                    kit::code(ui, t, "/login");
                    kit::text(ui, "and finish the sign-in in the browser.", 12.5, t.text_2);
                });
                kit::para(
                    ui,
                    "Do not run /logout on the account you are leaving — Claude Code may revoke the token cswap stored for it.",
                    12.5,
                    t.text_2,
                );
            });
            steps(ui, t, 3, |ui| {
                kit::para(ui, "Come back here and add it. cswap stores that login so you can switch back to it later.", 12.5, t.text_2);
            });
            kit::para(ui, "Adding an account that is already managed refreshes its stored credentials instead of creating a duplicate.", 12.5, t.text_3);
            two_fields(
                ui,
                |ui| {
                    kit::field(ui, t, "Slot", Some("Optional. Empty = next free."), |ui| {
                        let w = ui.available_width();
                        if kit::input(ui, t, slot, "add-slot", "auto", w, false, false).changed() {
                            *slot = view::digits_only(slot);
                        }
                    });
                },
                |ui| {
                    kit::field(ui, t, "Alias", Some("Optional short name."), |ui| {
                        let w = ui.available_width();
                        kit::input(ui, t, alias, "add-alias", "e.g. work", w, false, false);
                    });
                },
            );
            if let Some(n) = slot_num.filter(|_| slot_taken) {
                slot_warning(ui, t, &format!("Slot {n} is taken. Overwrite it with the current login?"), overwrite);
            }
        },
        |ui| {
            submit = Btn::new("Add account").variant(Variant::Primary).loading(busy).enabled(can_submit).show(ui, t).clicked();
            cancel = Btn::new("Cancel").variant(Variant::Ghost).show(ui, t).clicked();
        },
    );
    if submit {
        let opts = AddAccountOptions { slot: slot_num, alias: Some(alias.clone()).filter(|s| !s.is_empty()), overwrite: *overwrite };
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
    let out = kit::dialog(
        ctx,
        t,
        "token",
        "Add from a token",
        440.0,
        |ui| {
            kit::para(
                ui,
                "A long-lived setup-token (claude setup-token) or a managed API key. The token is handed to cswap over stdin, never as a command-line argument, and is not kept by this app.",
                12.5,
                t.text_2,
            );
            let hint = view::token_kind(token).map(|k| format!("Detected: {k}")).unwrap_or_else(|| "sk-ant-oat01-… or sk-ant-api03-…".into());
            kit::field(ui, t, "Token", Some(&hint), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let w = ui.available_width() - 38.0;
                    kit::input(ui, t, token, "token-input", "", w, true, !*show);
                    let tip = if *show { "Hide token" } else { "Show token" };
                    if Btn::new("").variant(Variant::Ghost).icon(if *show { Icon::EyeOff } else { Icon::Eye }).a11y(tip).show(ui, t).clicked() {
                        *show = !*show;
                    }
                });
            });
            two_fields(
                ui,
                |ui| {
                    kit::field(ui, t, "Label (email)", Some("Optional."), |ui| {
                        let w = ui.available_width();
                        kit::input(ui, t, email, "token-email", "setup-token-N@token.local", w, false, false);
                    });
                },
                |ui| {
                    kit::field(ui, t, "Slot", Some("Optional."), |ui| {
                        let w = ui.available_width();
                        if kit::input(ui, t, slot, "token-slot", "auto", w, false, false).changed() {
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
            submit = Btn::new("Register token").variant(Variant::Primary).loading(busy).enabled(can_submit).show(ui, t).clicked();
            cancel = Btn::new("Cancel").variant(Variant::Ghost).show(ui, t).clicked();
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

fn alias_dialog(ctx: &egui::Context, cx: &mut Cx, t: &Tokens, a: &Account, alias: &mut String, run: &mut Option<(Pending<CsResult<PlainOutput>>, String)>) -> bool {
    if settle_msg(cx, run) == Some(true) {
        return false;
    }
    let n = a.number;
    let valid = view::alias_valid(alias);
    let busy = run.is_some();
    let (mut save, mut unset, mut cancel, mut enter) = (false, false, false, false);
    let can_save = !alias.is_empty() && valid && Some(alias.as_str()) != a.alias.as_deref();
    let preview = format!("cswap switch {}", if alias.is_empty() { "dev" } else { alias.as_str() });
    let out = kit::dialog(
        ctx,
        t,
        "alias",
        &format!("Alias for Account-{n}"),
        440.0,
        |ui| {
            ui.horizontal_wrapped(|ui| {
                kit::text(ui, format!("{}. An alias works anywhere a slot number or email does:", a.email), 12.5, t.text_2);
                kit::code(ui, t, &preview);
            });
            kit::field(ui, t, "Alias", Some("Letters, digits, . - _ — not purely numeric."), |ui| {
                let w = ui.available_width();
                let r = kit::input(ui, t, alias, "alias-input", "", w, false, false);
                if r.changed() {
                    *alias = alias.trim().to_string();
                }
                enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            });
            if !valid {
                kit::text(ui, "Invalid alias.", 12.0, t.danger);
            }
        },
        |ui| {
            save = Btn::new("Save").variant(Variant::Primary).loading(busy).enabled(can_save).show(ui, t).clicked();
            cancel = Btn::new("Cancel").variant(Variant::Ghost).show(ui, t).clicked();
            if a.alias.is_some() {
                unset = Btn::new("Remove alias").variant(Variant::Ghost).loading(busy).show(ui, t).clicked();
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
    let out = kit::dialog(
        ctx,
        t,
        "move",
        &format!("Move Account-{} to a slot", a.number),
        440.0,
        |ui| {
            kit::para(
                ui,
                "Slot numbers are the shortcuts you type (cswap switch 1) and the list order. Aliases, backups and session history move with the account.",
                12.5,
                t.text_2,
            );
            kit::field(ui, t, "Destination slot", None, |ui| {
                let w = ui.available_width();
                if kit::input(ui, t, slot, "move-input", "", w, false, false).changed() {
                    *slot = view::digits_only(slot);
                }
            });
            if let Some(o) = occupant {
                let name = o.alias.clone().unwrap_or_else(|| o.email.clone());
                kit::para(ui, format!("Slot {n} holds {name} — the two will trade places."), 12.0, t.text_2);
            }
        },
        |ui| {
            go = Btn::new(if occupant.is_some() { "Swap" } else { "Move" }).variant(Variant::Primary).loading(busy).enabled(can_go).show(ui, t).clicked();
            cancel = Btn::new("Cancel").variant(Variant::Ghost).show(ui, t).clicked();
        },
    );
    if go {
        let msg = if occupant.is_some() { format!("Swapped slots {} and {n}", a.number) } else { format!("Moved to slot {n}") };
        *run = Some((cx.services.cswap.move_account(&a.number.to_string(), n), msg));
    }
    !(out.close || cancel)
}

fn remove_dialog(ctx: &egui::Context, cx: &mut Cx, t: &Tokens, a: &Account, typed: &mut String, run: &mut Option<Pending<CsResult<PlainOutput>>>) -> bool {
    let n = a.number;
    if settle(cx, run, |_| format!("Removed Account-{n}")) == Some(true) {
        return false;
    }
    let ok = view::remove_confirmed(typed, a);
    let busy = run.is_some();
    let (mut go, mut cancel) = (false, false);
    let out = kit::dialog(
        ctx,
        t,
        "remove",
        "Remove account",
        440.0,
        |ui| {
            let live = if a.active { ", but this is the active account — the live login stays until you switch or /logout" } else { "" };
            kit::para(
                ui,
                format!(
                    "This deletes the stored login for {} (slot {n}) from cswap. Claude Code itself is not logged out{live}. Recovery means logging in with that account again and re-adding it.",
                    a.email
                ),
                12.5,
                t.text_2,
            );
            kit::field(ui, t, &format!("Type {n} or the email to confirm"), None, |ui| {
                let w = ui.available_width();
                kit::input(ui, t, typed, "remove-confirm", "", w, false, false);
            });
        },
        |ui| {
            go = Btn::new("Remove permanently").variant(Variant::Danger).loading(busy).enabled(ok).show(ui, t).clicked();
            cancel = Btn::new("Cancel").variant(Variant::Ghost).show(ui, t).clicked();
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
            ui.set_width(w - 24.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.set_width(w - 24.0 - 44.0);
                    kit::text(ui, title, 13.0, t.text);
                    kit::para(ui, sub, 12.0, t.text_3);
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| kit::switch(ui, t, on, label, true));
            });
        });
}

fn transfer_settle(cx: &mut Cx, run: &mut Option<Pending<CsResult<Transfer>>>, title: &str, fail: &str, import: bool) -> bool {
    let Some(p) = run.as_mut() else { return false };
    match p.take() {
        Some(Ok(v)) => {
            let body = if import && !v.output.stdout.trim().is_empty() { v.output.stdout.trim().to_string() } else { v.path.clone() };
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

fn export_dialog(ctx: &egui::Context, cx: &mut Cx, t: &Tokens, a: Option<&Account>, full: &mut bool, run: &mut Option<Pending<CsResult<Transfer>>>) -> bool {
    if transfer_settle(cx, run, "Exported", "Export failed", false) {
        return false;
    }
    let busy = run.is_some();
    let (mut go, mut cancel) = (false, false);
    let title = a.map(|a| format!("Export Account-{}", a.number)).unwrap_or_else(|| "Export all accounts".into());
    let out = kit::dialog(
        ctx,
        t,
        "export",
        &title,
        440.0,
        |ui| {
            kit::para(
                ui,
                "The export is plaintext JSON containing login credentials. Keep it somewhere private, or encrypt it. By default only each account's own login is included — machine-shared MCP/plugin OAuth tokens stay on this machine.",
                12.5,
                t.text_2,
            );
            switch_row(ui, t, "Full export (same-PC backup)", "Include the whole ~/.claude.json and credential object.", full, "Full export");
        },
        |ui| {
            go = Btn::new("Choose file & export").variant(Variant::Primary).loading(busy).show(ui, t).clicked();
            cancel = Btn::new("Cancel").variant(Variant::Ghost).show(ui, t).clicked();
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

fn import_dialog(ctx: &egui::Context, cx: &mut Cx, t: &Tokens, force: &mut bool, run: &mut Option<Pending<CsResult<Transfer>>>) -> bool {
    if transfer_settle(cx, run, "Import finished", "Import failed", true) {
        return false;
    }
    let busy = run.is_some();
    let (mut go, mut cancel) = (false, false);
    let out = kit::dialog(
        ctx,
        t,
        "import",
        "Import accounts",
        440.0,
        |ui| {
            kit::para(
                ui,
                "Accounts that already exist are skipped, except slots whose refresh token has died — those are replaced automatically. If the imported account is the one you are logged in as, activate it afterwards with Switch (use --force from the CLI to bypass the backup of the current login).",
                12.5,
                t.text_2,
            );
            switch_row(ui, t, "Overwrite existing accounts", "Passes --force. A stale export can carry an already-superseded token.", force, "Overwrite existing");
        },
        |ui| {
            go = Btn::new("Choose file & import").variant(Variant::Primary).loading(busy).show(ui, t).clicked();
            cancel = Btn::new("Cancel").variant(Variant::Ghost).show(ui, t).clicked();
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

fn diag_dialog(ctx: &egui::Context, t: &Tokens, run: &mut Pending<CsResult<PlainOutput>>, out_text: &mut Option<Result<String, String>>) -> bool {
    if out_text.is_none() {
        match run.take() {
            Some(Ok(v)) => *out_text = Some(Ok(view::strip_ansi(&v.stdout))),
            Some(Err(e)) => *out_text = Some(Err(e.message)),
            None if run.is_lost() => *out_text = Some(Err("No answer from cswap".into())),
            None => {}
        }
    }
    let mut close = false;
    let out = kit::dialog(
        ctx,
        t,
        "diag",
        "Token diagnostics",
        640.0,
        |ui| {
            ui.horizontal_wrapped(|ui| {
                kit::text(ui, "Output of", 12.5, t.text_2);
                kit::code(ui, t, "cswap list --token-status");
                kit::text(ui, ": where each account's OAuth token comes from and whether it is still valid.", 12.5, t.text_2);
            });
            match out_text {
                None => {
                    kit::text(ui, "Running…", 12.0, t.text_3);
                }
                Some(Err(e)) => {
                    kit::para(ui, e.clone(), 12.0, t.danger);
                }
                Some(Ok(s)) => {
                    let w = ui.available_width();
                    egui::Frame::new()
                        .fill(t.bg)
                        .stroke(Stroke::new(1.0, t.border))
                        .corner_radius(CornerRadius::same(crate::theme::RADIUS_MD))
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.set_width(w - 24.0);
                            egui::ScrollArea::vertical().max_height(ctx.content_rect().height() * 0.46).show(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(s.trim()).font(kit::mono(12.0)).color(t.text_2)).selectable(true).wrap());
                            });
                        });
                }
            }
        },
        |ui| {
            close = Btn::new("Close").show(ui, t).clicked();
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
    let out = kit::dialog(
        ctx,
        t,
        "run",
        &format!("Open a terminal as Account-{}", a.number),
        520.0,
        |ui| {
            let who = a.alias.clone().unwrap_or_else(|| a.email.clone());
            kit::para(
                ui,
                format!("Session mode: Claude Code runs as {who} in that terminal only. Your default login and every other terminal are untouched."),
                12.5,
                t.text_2,
            );
            kit::field(ui, t, "Start in", Some("Leave empty for your home directory."), |ui| {
                ui.horizontal(|ui| {
                    let b = Btn::new("Browse").icon(Icon::Folder);
                    let w = ui.available_width() - b.width(ui) - 8.0;
                    kit::input(ui, t, dir, "run-dir", "~", w, true, false);
                    browse = b.show(ui, t).clicked();
                });
            });
            if !mappings.is_empty() {
                kit::field(ui, t, "Or pick a mapped directory", Some("Directories you have mapped with cswap map."), |ui| {
                    let current = if mappings.iter().any(|m| &m.path == dir) { dir.clone() } else { "—".into() };
                    egui::ComboBox::from_id_salt("run-mapped").width(ui.available_width() - 8.0).selected_text(current).show_ui(ui, |ui| {
                        if ui.selectable_label(false, "—").clicked() {
                            dir.clear();
                        }
                        for m in mappings.iter() {
                            let label = match &m.account {
                                Some(x) => format!("{} — {}", m.path, x.alias.clone().unwrap_or_else(|| x.email.clone())),
                                None => m.path.clone(),
                            };
                            if ui.selectable_label(&m.path == dir, label).clicked() {
                                *dir = m.path.clone();
                            }
                        }
                    });
                });
            }
            let w = ui.available_width();
            egui::Frame::new()
                .fill(t.bg)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(crate::theme::RADIUS_MD))
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(w - 24.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let shown = if dir.trim().is_empty() { "~".to_string() } else { dir.trim().to_string() };
                        kit::mono_text(ui, format!("{shown}$ "), 12.0, t.text_3);
                        kit::mono_text(ui, format!("cswap run {}", a.number), 12.0, t.text_2);
                    });
                });
        },
        |ui| {
            go = Btn::new("Open terminal").variant(Variant::Primary).icon(Icon::Terminal).loading(busy).show(ui, t).clicked();
            cancel = Btn::new("Cancel").variant(Variant::Ghost).show(ui, t).clicked();
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
