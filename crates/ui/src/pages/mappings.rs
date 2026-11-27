//! Port of `src/renderer/src/pages/Mappings.tsx`. OWNER: ui-accounts.
//!
//! `cswap run` with no account launches the account mapped to the current directory.
//! The list is cswap's `mappings.json` (read by the driver, joined with the account list);
//! writes go through `cswap map` / `cswap unmap`.

use egui::{Align, CornerRadius, Layout, Margin, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};

use cswap_core::view::ChipTone;
use cswap_core::{Account, CsResult, Event, LaunchedSession, Mapping, Pending, PlainOutput};

use super::accounts::notify;
use crate::style;
use crate::widgets::{self, Button, Variant, icon};
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
            None => notify(
                cx,
                ToastKind::Error,
                "No account for this directory",
                Some(&format!("{} is no longer managed by cswap.", m.email)),
            ),
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
            let b = Button::new("Map a directory").variant(Variant::Primary).icon(icon::PLUS).enabled(!list.is_empty());
            let bw = b.width(ui);
            widgets::page_header(ui, "Directory mappings", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                        widgets::line(ui, "cswap run", style::mono(12.0), t.text_2);
                        widgets::line(ui, " ", style::body(12.5), t.text_2);
                        let rest = "with no account launches the account mapped to the current directory — session mode, this terminal only. Open one straight from a row.";
                        for w in rest.split(' ') {
                            widgets::line(ui, format!("{w} "), style::body(12.5), t.text_2);
                        }
                    });
                }, bw, |ui| add = b.show(ui).clicked());
            egui::Frame::new().inner_margin(Margin { left: 24, right: 24, top: 0, bottom: 32 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = vec2(8.0, 12.0);
                match &self.rows {
                    Some(Err(e)) => widgets::notice(ui, e, ChipTone::Danger),
                    Some(Ok(rows)) if rows.is_empty() => {
                        widgets::empty(ui, None, "No directory mappings", "Map a project folder to an account, then `cswap run` inside it picks that account automatically.", |ui| add |= Button::new("Map a directory").enabled(!list.is_empty()).show(ui).clicked());
                    }
                    Some(Ok(rows)) => {
                        let w = ui.available_width();
                        widgets::card_frame(&t).show(ui, |ui| {
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

    fn add_dialog(&mut self, ctx: &egui::Context, cx: &mut Cx, _t: &Tokens, list: &[Account]) {
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
        let out = widgets::dialog(ctx, "map", "Map a directory to an account", 440.0, |ui| {
                widgets::field(ui, "Account", None, false, |ui| {
                    let label = |a: &Account| {
                        format!(
                            "{}. {}{}",
                            a.number,
                            a.alias.as_ref().map(|s| format!("{s} — ")).unwrap_or_default(),
                            a.email
                        )
                    };
                    let opts: Vec<(String, String)> = list.iter().map(|a| (a.number.to_string(), label(a))).collect();
                    let w = ui.available_width();
                    widgets::select(ui, "Account", &mut st.target, &opts, w);
                });
                widgets::field(ui, "Directory", None, false, |ui| {
                    ui.horizontal(|ui| {
                        let b = Button::new("Browse").icon(icon::FOLDER_OPEN);
                        let w = ui.available_width() - b.width(ui) - 8.0;
                        widgets::input_opts(ui, "", &mut st.path, "/path/to/project", true, w, false);
                        browse = b.show(ui).clicked();
                    });
                });
            }, |ui| {
                go = Button::new("Map").variant(Variant::Primary).loading(busy).enabled(can).show(ui).clicked();
                cancel = Button::new("Cancel").variant(Variant::Ghost).show(ui).clicked();
            });
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
    let open_b = Button::new("Open terminal").small().icon(icon::SQUARE_TERMINAL).enabled(m.account.is_some());
    let unmap_label = format!("Unmap {}", m.path);
    let unmap_b = Button::new("Unmap").small().variant(Variant::Ghost).icon(icon::TRASH_2).label(&unmap_label);
    let buttons_w = open_b.width(ui) + unmap_b.width(ui) + 8.0;
    widgets::paint_icon(ui, icon::FOLDER_OPEN, pos2(top_left.x + 12.0 + 7.5, rect.center().y), 15.0, t.text_3);
    let text_x = top_left.x + 12.0 + 15.0 + 12.0;
    let text_w = (rect.max.x - 12.0 - buttons_w - 12.0 - text_x).max(40.0);
    let g = widgets::galley(ui, m.path.clone(), style::mono(12.0), t.text, text_w);
    let path_rect = Rect::from_min_size(pos2(text_x, rect.min.y + 10.0), g.size());
    ui.painter().galley(path_rect.min, g, t.text);
    let _ = ui.interact(path_rect, ui.scope_id().with(("map-path", &m.path)), Sense::hover()).on_hover_text(&m.path);
    widgets::place(ui, UiBuilder::new().max_rect(Rect::from_min_size(pos2(text_x, rect.min.y + 30.0), vec2(text_w, 18.0))), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                match &m.account {
                    Some(a) => {
                        widgets::line(ui, "→", style::body(11.5), t.text_3);
                        let who = a.alias.clone().unwrap_or_else(|| a.email.clone());
                        widgets::line(ui, format!("Account-{} · {who}", a.number), style::body(11.5), t.text_2);
                    }
                    None => {
                        widgets::line(ui, format!("→ {}", m.email), style::body(11.5), t.text_3);
                        widgets::chip(ui, "account removed", ChipTone::Warn);
                    }
                }
            });
        });
    widgets::place(ui, UiBuilder::new()
            .max_rect(Rect::from_min_max(
                pos2(rect.max.x - 12.0 - buttons_w, rect.min.y),
                pos2(rect.max.x - 12.0, rect.max.y),
            ))
            .layout(Layout::right_to_left(Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if unmap_b.show(ui).clicked() {
                out = Some(false);
            }
            if open_b.show(ui).on_hover_text(open_tip).clicked() {
                out = Some(true);
            }
        });
    if !last {
        ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, t.border));
    }
    ui.allocate_rect(rect, Sense::hover());
    out
}
