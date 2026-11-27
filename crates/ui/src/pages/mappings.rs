//! Port of `src/renderer/src/pages/Mappings.tsx`. OWNER: ui-accounts.
//!
//! `cswap run` with no account launches the account mapped to the current directory.
//! The list is cswap's `mappings.json` (read by the driver, joined with the account list);
//! writes go through `cswap map` / `cswap unmap`.

use egui::{Align, CornerRadius, Layout, Margin, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};

use cswap_core::view::ChipTone;
use cswap_core::{Account, CsResult, Event, LaunchedSession, Mapping, Pending, PlainOutput};

use super::accounts::notify;
use super::kit::{self, Btn, Icon, Variant};
use crate::cx::Cx;
use crate::theme::Tokens;
use crate::toast::ToastKind;

struct AddMap {
    target: String,
    path: String,
    run: Option<Pending<CsResult<PlainOutput>>>,
}

#[derive(Default)]
pub struct MappingsPage {
    rows: Option<Result<Vec<Mapping>, String>>,
    load: Option<Pending<CsResult<Vec<Mapping>>>>,
    /// The list's `fetchedAt` the rows were loaded against: a refresh reloads them, as
    /// 0.1.23's effect on `accounts.fetchedAt` did (an account renamed or removed).
    loaded_for: Option<Option<String>>,
    adding: Option<AddMap>,
    launching: Vec<Pending<CsResult<LaunchedSession>>>,
    unmapping: Vec<(String, Pending<CsResult<PlainOutput>>)>,
}

impl MappingsPage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_event(&mut self, _ev: &Event) {}

    fn reload(&mut self, cx: &Cx) {
        self.load = Some(cx.services.cswap.list_mappings());
        self.loaded_for = Some(cx.accounts.fetched_at.clone());
    }

    fn poll(&mut self, cx: &mut Cx) {
        if self.loaded_for.as_ref() != Some(&cx.accounts.fetched_at) && self.load.is_none() {
            self.reload(cx);
        }
        if let Some(p) = self.load.as_mut() {
            match p.take() {
                Some(r) => {
                    self.rows = Some(r.map_err(|e| e.message));
                    self.load = None;
                }
                None if p.is_lost() => self.load = None,
                None => {}
            }
        }
        let mut i = 0;
        while i < self.launching.len() {
            match self.launching[i].take() {
                Some(Ok(v)) => {
                    notify(cx, ToastKind::Success, &format!("Opened {}", v.command), Some(&v.cwd));
                    self.launching.remove(i);
                }
                Some(Err(e)) => {
                    notify(cx, ToastKind::Error, "Could not open a terminal", Some(&e.message));
                    self.launching.remove(i);
                }
                None if self.launching[i].is_lost() => {
                    self.launching.remove(i);
                }
                None => i += 1,
            }
        }
        let mut i = 0;
        let mut reload = false;
        while i < self.unmapping.len() {
            let (path, p) = &mut self.unmapping[i];
            match p.take() {
                Some(r) => {
                    match r {
                        Ok(_) => notify(cx, ToastKind::Success, &format!("Unmapped {path}"), None),
                        Err(e) => notify(cx, ToastKind::Error, "Unmap failed", Some(&e.message)),
                    }
                    self.unmapping.remove(i);
                    reload = true;
                }
                None if p.is_lost() => {
                    self.unmapping.remove(i);
                }
                None => i += 1,
            }
        }
        if reload {
            self.reload(cx);
        }
    }

    fn open(&mut self, cx: &mut Cx, m: &Mapping) {
        match &m.account {
            None => notify(cx, ToastKind::Error, "No account for this directory", Some(&format!("{} is no longer managed by cswap.", m.email))),
            Some(a) => self.launching.push(cx.services.cswap.launch_session(&a.number.to_string(), Some(&m.path))),
        }
    }

    fn start_add(&mut self, list: &[Account]) {
        let target = list.iter().find(|a| a.active).or(list.first()).map(|a| a.number.to_string()).unwrap_or_default();
        self.adding = Some(AddMap { target, path: String::new(), run: None });
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        self.poll(cx);
        let t = cx.tokens.clone();
        let list: Vec<Account> = cx.accounts.payload.as_ref().map(|p| p.accounts.clone()).unwrap_or_default();
        let mut add = false;
        let mut open: Option<Mapping> = None;
        let mut unmap: Option<String> = None;
        egui::ScrollArea::vertical().id_salt("mappings-page").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
            let b = Btn::new("Map a directory").variant(Variant::Primary).icon(Icon::Plus).enabled(!list.is_empty());
            let bw = b.width(ui);
            kit::page_header(
                ui,
                &t,
                "Directory mappings",
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                        kit::mono_text(ui, "cswap run ", 12.0, t.text_2);
                        let rest = "with no account launches the account mapped to the current directory — session mode, this terminal only. Open one straight from a row.";
                        for w in rest.split(' ') {
                            kit::text(ui, format!("{w} "), 12.5, t.text_2);
                        }
                    });
                },
                bw,
                |ui| add = b.show(ui, &t).clicked(),
            );
            egui::Frame::new().inner_margin(Margin { left: 24, right: 24, top: 0, bottom: 32 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = vec2(8.0, 12.0);
                match &self.rows {
                    Some(Err(e)) => kit::notice(ui, &t, e, ChipTone::Danger),
                    Some(Ok(rows)) if rows.is_empty() => {
                        kit::empty(
                            ui,
                            &t,
                            "No directory mappings",
                            "Map a project folder to an account, then `cswap run` inside it picks that account automatically.",
                            |ui| add |= Btn::new("Map a directory").enabled(!list.is_empty()).show(ui, &t).clicked(),
                        );
                    }
                    Some(Ok(rows)) => {
                        let w = ui.available_width();
                        kit::card_frame(&t).show(ui, |ui| {
                            ui.set_width(w - 2.0);
                            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                            for (i, m) in rows.iter().enumerate() {
                                match mapping_row(ui, &t, m, i + 1 == rows.len()) {
                                    Some(true) => open = Some(m.clone()),
                                    Some(false) => unmap = Some(m.path.clone()),
                                    None => {}
                                }
                            }
                        });
                    }
                    None => {}
                }
            });
        });
        if add {
            self.start_add(&list);
        }
        if let Some(m) = open {
            self.open(cx, &m);
        }
        if let Some(p) = unmap {
            let pend = cx.services.cswap.unmap_directory(&p);
            self.unmapping.push((p, pend));
        }
        self.add_dialog(ui.ctx(), cx, &t, &list);
    }

    fn add_dialog(&mut self, ctx: &egui::Context, cx: &mut Cx, t: &Tokens, list: &[Account]) {
        let Some(st) = self.adding.as_mut() else { return };
        if let Some(p) = st.run.as_mut() {
            match p.take() {
                Some(Ok(v)) => {
                    let first = v.stdout.trim().lines().next().unwrap_or("").to_string();
                    notify(cx, ToastKind::Success, if first.is_empty() { "Mapped" } else { &first }, None);
                    self.adding = None;
                    self.reload(cx);
                    return;
                }
                Some(Err(e)) => {
                    notify(cx, ToastKind::Error, "Map failed", Some(&e.message));
                    st.run = None;
                }
                None if p.is_lost() => st.run = None,
                None => {}
            }
        }
        let busy = st.run.is_some();
        let can = !st.path.trim().is_empty() && !st.target.is_empty();
        let (mut go, mut cancel, mut browse) = (false, false, false);
        let out = kit::dialog(
            ctx,
            t,
            "map",
            "Map a directory to an account",
            440.0,
            |ui| {
                kit::field(ui, t, "Account", None, |ui| {
                    let label = |a: &Account| format!("{}. {}{}", a.number, a.alias.as_ref().map(|s| format!("{s} — ")).unwrap_or_default(), a.email);
                    let current = list.iter().find(|a| a.number.to_string() == st.target).map(label).unwrap_or_default();
                    egui::ComboBox::from_id_salt("map-account").width(ui.available_width() - 8.0).selected_text(current).show_ui(ui, |ui| {
                        for a in list {
                            let n = a.number.to_string();
                            if ui.selectable_label(st.target == n, label(a)).clicked() {
                                st.target = n;
                            }
                        }
                    });
                });
                kit::field(ui, t, "Directory", None, |ui| {
                    ui.horizontal(|ui| {
                        let b = Btn::new("Browse").icon(Icon::Folder);
                        let w = ui.available_width() - b.width(ui) - 8.0;
                        kit::input(ui, t, &mut st.path, "map-path", "/path/to/project", w, true, false);
                        browse = b.show(ui, t).clicked();
                    });
                });
            },
            |ui| {
                go = Btn::new("Map").variant(Variant::Primary).loading(busy).enabled(can).show(ui, t).clicked();
                cancel = Btn::new("Cancel").variant(Variant::Ghost).show(ui, t).clicked();
            },
        );
        if browse && let Some(p) = cx.services.host.pick_directory("Map a directory") {
            st.path = p;
        }
        if go {
            st.run = Some(cx.services.cswap.map_directory(&st.target, st.path.trim()));
        }
        if out.close || cancel {
            self.adding = None;
        }
    }
}

/// One mapping. `Some(true)` = open a terminal, `Some(false)` = unmap.
fn mapping_row(ui: &mut Ui, t: &Tokens, m: &Mapping, last: bool) -> Option<bool> {
    let top_left = ui.cursor().min;
    let width = ui.available_width();
    let rect = Rect::from_min_size(top_left, vec2(width, 56.0));
    if ui.rect_contains_pointer(rect) {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, t.surface_2.gamma_multiply(0.5));
    }
    let mut out = None;
    let open_tip = match &m.account {
        Some(a) => format!("cswap run {} in {}", a.number, m.path),
        None => "The mapped account is no longer managed".into(),
    };
    let open_b = Btn::new("Open terminal").small().icon(Icon::Terminal).enabled(m.account.is_some());
    let unmap_label = format!("Unmap {}", m.path);
    let unmap_b = Btn::new("Unmap").small().variant(Variant::Ghost).icon(Icon::Trash).a11y(&unmap_label);
    let buttons_w = open_b.width(ui) + unmap_b.width(ui) + 8.0;
    kit::paint_icon(ui.painter(), pos2(top_left.x + 12.0 + 7.5, rect.center().y), Icon::Folder, t.text_3);
    let text_x = top_left.x + 12.0 + 15.0 + 12.0;
    let text_w = (rect.max.x - 12.0 - buttons_w - 12.0 - text_x).max(40.0);
    let g = kit::galley(ui, m.path.clone(), kit::mono(12.0), t.text, text_w);
    let path_rect = Rect::from_min_size(pos2(text_x, rect.min.y + 10.0), g.size());
    ui.painter().galley(path_rect.min, g, t.text);
    let _ = ui.interact(path_rect, ui.id().with(("map-path", &m.path)), Sense::hover()).on_hover_text(&m.path);
    ui.scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(pos2(text_x, rect.min.y + 30.0), vec2(text_w, 18.0))), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            match &m.account {
                Some(a) => {
                    kit::text(ui, "→", 11.5, t.text_3);
                    let who = a.alias.clone().unwrap_or_else(|| a.email.clone());
                    kit::text(ui, format!("Account-{} · {who}", a.number), 11.5, t.text_2);
                }
                None => {
                    kit::text(ui, format!("→ {}", m.email), 11.5, t.text_3);
                    kit::chip(ui, t, "account removed", ChipTone::Warn);
                }
            }
        });
    });
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(Rect::from_min_max(pos2(rect.max.x - 12.0 - buttons_w, rect.min.y), pos2(rect.max.x - 12.0, rect.max.y)))
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if unmap_b.show(ui, t).clicked() {
                out = Some(false);
            }
            if open_b.show(ui, t).on_hover_text(open_tip).clicked() {
                out = Some(true);
            }
        },
    );
    if !last {
        ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, t.border));
    }
    ui.allocate_rect(rect, Sense::hover());
    out
}
