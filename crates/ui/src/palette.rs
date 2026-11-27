//! Command palette (Ctrl/Cmd+K). Port of `components/CommandPalette.tsx`. OWNER: ui-shell.
//!
//! Typing filters the accounts by email, alias, slot number or organisation; a page name
//! also offers "Go to <page>". Enter switches to the highlighted account through the CLI
//! (or opens the page); the result comes back as a toast.

use cswap_core::{Account, AccountsState, CsResult, Page, Pending, SwitchPayload};
use egui::{Align, Align2, Color32, CornerRadius, Frame, Key, Layout, Margin, Modifiers, Sense, Stroke, Ui, pos2, vec2};

use crate::cx::Cx;
use crate::style;
use crate::widgets::{self, icon};

#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    Account(Box<Account>),
    Page(Page),
}

const PAGES: [(Page, &str); 5] = [
    (Page::Accounts, "accounts"),
    (Page::Auto, "auto"),
    (Page::Mappings, "mappings"),
    (Page::Log, "log"),
    (Page::Settings, "settings"),
];

/// The rows for a query: matching accounts first (all of them for an empty query), then the
/// pages whose name contains it (none for an empty query, so the list stays about accounts).
pub fn rows(accounts: &AccountsState, query: &str) -> Vec<Row> {
    let needle = query.trim().to_lowercase();
    let list = accounts.payload.as_ref().map(|p| p.accounts.as_slice()).unwrap_or(&[]);
    let mut out: Vec<Row> = list
        .iter()
        .filter(|a| {
            needle.is_empty()
                || a.email.to_lowercase().contains(&needle)
                || a.alias.as_deref().is_some_and(|al| al.to_lowercase().contains(&needle))
                || a.number.to_string() == needle
                || a.organization_name.to_lowercase().contains(&needle)
        })
        .map(|a| Row::Account(Box::new(a.clone())))
        .collect();
    if !needle.is_empty() {
        out.extend(PAGES.iter().filter(|(_, n)| n.contains(&needle)).map(|(p, _)| Row::Page(*p)));
    }
    out
}

/// Keeps the first three characters of an address (the "Hide email addresses" setting).
pub(crate) fn mask_email(email: &str, on: bool) -> String {
    if !on || email.is_empty() {
        return email.to_owned();
    }
    let local = email.split('@').next().unwrap_or(email);
    let head: String = local.chars().take(3).collect();
    let n = email.chars().count().saturating_sub(3).clamp(3, 8);
    format!("{head}{}", "•".repeat(n))
}

fn pct(v: Option<f64>) -> String {
    match v {
        Some(p) if !p.is_nan() => format!("{}%", p.round() as i64),
        _ => "—".into(),
    }
}

#[derive(Default)]
pub struct Palette {
    pub open: bool,
    query: String,
    index: usize,
    focus_pending: bool,
    switching: Option<Pending<CsResult<SwitchPayload>>>,
}

impl Palette {
    pub fn toggle(&mut self) {
        if self.open {
            self.open = false;
        } else {
            self.open_fresh();
        }
    }

    pub fn open_fresh(&mut self) {
        self.open = true;
        self.query.clear();
        self.index = 0;
        self.focus_pending = true;
    }

    /// Polls a switch started from the palette; the palette may be closed by then.
    pub fn poll(&mut self, cx: &mut Cx) {
        let Some(p) = &mut self.switching else { return };
        if let Some(r) = p.take() {
            self.switching = None;
            match r {
                Ok(v) => {
                    let text = if v.warnings.is_empty() { v.message.clone() } else { format!("{}\n{}", v.message, v.warnings.join("\n")) };
                    if v.switched {
                        cx.toasts.success(text)
                    } else {
                        cx.toasts.info(text)
                    }
                }
                Err(e) => cx.toasts.error(format!("Switch failed\n{}", e.message)),
            }
        } else if p.is_lost() {
            self.switching = None;
            cx.toasts.error("Switch failed\nthe service stopped answering");
        }
    }

    fn run(&mut self, row: Option<&Row>, cx: &mut Cx) {
        let Some(row) = row else { return };
        self.open = false;
        match row {
            Row::Page(p) => *cx.navigate = Some(*p),
            Row::Account(a) if a.active => {}
            Row::Account(a) => self.switching = Some(cx.services.cswap.switch_to(&a.number.to_string(), false)),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, cx: &mut Cx) {
        if !self.open {
            return;
        }
        let rows = rows(cx.accounts, &self.query);
        if self.index >= rows.len() {
            self.index = rows.len().saturating_sub(1);
        }
        // Escape is read globally, not from the input: it used to live on the input's keydown
        // and did nothing once focus had moved elsewhere.
        let (esc, down, up, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        if esc {
            self.open = false;
            return;
        }
        if down {
            self.index = (self.index + 1).min(rows.len().saturating_sub(1));
        }
        if up {
            self.index = self.index.saturating_sub(1);
        }
        if enter {
            self.run(rows.get(self.index), cx);
            return;
        }

        let t = style::tokens(ctx);
        let screen = ctx.content_rect();
        let width = 520f32.min(screen.width() - 32.0);
        let mut clicked: Option<usize> = None;
        let mut close = false;
        let mut query_changed = false;
        let area = egui::Modal::default_area(egui::Id::new("palette"))
            .anchor(Align2::CENTER_TOP, vec2(0.0, screen.height() * 0.14));
        let modal = egui::Modal::new(egui::Id::new("palette"))
            .area(area)
            .backdrop_color(Color32::from_black_alpha(64))
            .frame(
                Frame::new()
                    .fill(t.surface)
                    .stroke(Stroke::new(1.0, t.border))
                    .corner_radius(CornerRadius::same(12))
                    .shadow(style::pop_shadow(&t)),
            )
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 0.0;
                let id = egui::Id::new("palette-input");
                let edit = egui::TextEdit::singleline(&mut self.query)
                    .id(id)
                    .font(style::body(14.0))
                    .text_color(t.text)
                    .hint_text(widgets::rich("Switch to account… (type an email, alias or slot)", style::body(14.0), t.text_3))
                    .frame(Frame::new().inner_margin(Margin::symmetric(16, 13)))
                    .desired_width(width - 32.0);
                let r = ui.add(edit);
                widgets::name(ui, &r, "Switch to account");
                if self.focus_pending {
                    r.request_focus();
                    self.focus_pending = false;
                }
                query_changed = r.changed();
                let line = ui.available_rect_before_wrap();
                ui.painter().hline(line.x_range(), line.top(), Stroke::new(1.0, t.border));

                egui::ScrollArea::vertical().max_height(340.0).auto_shrink([false, true]).show(ui, |ui| {
                    Frame::new().inner_margin(Margin::same(6)).show(ui, |ui| {
                        ui.set_width(width - 12.0);
                        if rows.is_empty() {
                            ui.add_space(18.0);
                            ui.vertical_centered(|ui| widgets::text(ui, "No matches", style::body(12.5), t.text_3));
                            ui.add_space(18.0);
                        }
                        for (i, row) in rows.iter().enumerate() {
                            if self.row(ui, row, i, cx.settings.mask_emails, &t).clicked() {
                                clicked = Some(i);
                            }
                        }
                    });
                });
                let line = ui.available_rect_before_wrap();
                ui.painter().hline(line.x_range(), line.top(), Stroke::new(1.0, t.border));
                Frame::new().inner_margin(Margin::symmetric(12, 6)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let hint = |ui: &mut Ui, s: &str| widgets::text(ui, s, style::body(11.0), t.text_3);
                        widgets::kbd(ui, "↑");
                        widgets::kbd(ui, "↓");
                        hint(ui, "navigate");
                        ui.add_space(8.0);
                        widgets::kbd(ui, icon::CORNER_DOWN_LEFT);
                        hint(ui, "switch");
                        ui.add_space(8.0);
                        widgets::kbd(ui, "esc");
                        hint(ui, "close");
                    });
                });
            });
        if modal.backdrop_response.clicked() {
            close = true;
        }
        if query_changed {
            self.index = 0;
        }
        if let Some(i) = clicked {
            self.run(rows.get(i), cx);
        } else if close {
            self.open = false;
        }
    }

    fn row(&mut self, ui: &mut Ui, row: &Row, i: usize, mask: bool, t: &crate::theme::Tokens) -> egui::Response {
        let selected = i == self.index;
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 36.0), Sense::click());
        let label = match row {
            Row::Account(a) => format!("{} {}", a.alias.as_deref().unwrap_or(""), mask_email(&a.email, mask)).trim().to_owned(),
            Row::Page(p) => format!("Go to {}", page_title(*p)),
        };
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &label));
        if resp.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
            // `onMouseEnter`: the pointer moves the highlight, a still pointer does not fight
            // the arrow keys.
            self.index = i;
        }
        if selected {
            ui.painter().rect_filled(rect, CornerRadius::same(crate::theme::RADIUS_MD), t.surface_2);
        }
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(10.0, 0.0))).layout(Layout::left_to_right(Align::Center)));
        child.spacing_mut().item_spacing.x = 12.0;
        match row {
            Row::Account(a) => {
                let (r, _) = child.allocate_exact_size(vec2(20.0, 16.0), Sense::hover());
                child.painter().text(pos2(r.right(), r.center().y), Align2::RIGHT_CENTER, a.number.to_string(), style::mono(12.0), t.text_3);
                let (d, _) = child.allocate_exact_size(vec2(6.0, 6.0), Sense::hover());
                child.painter().circle_filled(d.center(), 3.0, if a.active { t.accent } else { t.border_strong });
                // right side first so the name truncates, not the figures
                child.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    if a.active {
                        widgets::text(ui, "active", style::body(11.0), t.accent);
                    } else {
                        widgets::icon(ui, icon::ARROW_RIGHT_LEFT, 13.0, t.text_3);
                    }
                    if let Some(u) = a.usage.as_ref().filter(|u| u.five_hour.is_some()) {
                        let s = format!(
                            "5h {} · 7d {}",
                            pct(u.five_hour.as_ref().map(|w| w.pct)),
                            pct(u.seven_day.as_ref().map(|w| w.pct))
                        );
                        widgets::text(ui, s, style::body(11.5), t.text_3);
                    }
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        let mut job = egui::text::LayoutJob::default();
                        if let Some(al) = &a.alias {
                            job.append(al, 0.0, egui::TextFormat::simple(style::body_medium(13.0), t.text));
                            job.append(" ", 0.0, egui::TextFormat::simple(style::body(13.0), t.text));
                        }
                        let email_color = if a.alias.is_some() { t.text_2 } else { t.text };
                        job.append(&mask_email(&a.email, mask), 0.0, egui::TextFormat::simple(style::body(13.0), email_color));
                        job.wrap.max_rows = 1;
                        job.wrap.break_anywhere = true;
                        job.wrap.max_width = ui.available_width();
                        ui.add(egui::Label::new(job).selectable(false).truncate());
                    });
                });
            }
            Row::Page(p) => {
                let (r, _) = child.allocate_exact_size(vec2(20.0, 16.0), Sense::hover());
                child.painter().text(pos2(r.right(), r.center().y), Align2::RIGHT_CENTER, "→", style::mono(12.0), t.text_3);
                widgets::text(&mut child, format!("Go to {}", page_title(*p)), style::body(13.0), t.text);
            }
        }
        resp
    }
}

pub fn page_title(p: Page) -> &'static str {
    match p {
        Page::Accounts => "Accounts",
        Page::Auto => "Auto",
        Page::Mappings => "Mappings",
        Page::Log => "Log",
        Page::Settings => "Settings",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cswap_core::ListPayload;

    fn acc(n: u32, email: &str, alias: Option<&str>, org: &str) -> Account {
        Account {
            number: n,
            email: email.into(),
            alias: alias.map(Into::into),
            organization_name: org.into(),
            ..Default::default()
        }
    }

    fn state() -> AccountsState {
        AccountsState {
            payload: Some(ListPayload {
                schema_version: 1,
                accounts: vec![
                    acc(1, "you@example.com", Some("main"), ""),
                    acc(2, "work@company.com", None, "Company Inc"),
                    acc(12, "twelve@example.com", None, ""),
                ],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn numbers(r: &[Row]) -> Vec<String> {
        r.iter()
            .map(|r| match r {
                Row::Account(a) => a.number.to_string(),
                Row::Page(p) => format!("{p:?}"),
            })
            .collect()
    }

    #[test]
    fn filters_by_email_alias_exact_slot_and_org() {
        let s = state();
        assert_eq!(numbers(&rows(&s, "")), ["1", "2", "12"], "empty query: every account, no pages");
        assert_eq!(numbers(&rows(&s, "MAIN")), ["1"]);
        assert_eq!(numbers(&rows(&s, "company")), ["2"]);
        assert_eq!(numbers(&rows(&s, "inc")), ["2"]);
        // a slot matches exactly: "1" is not a prefix search that would also return 12
        assert_eq!(numbers(&rows(&s, "1")), ["1"]);
        assert_eq!(numbers(&rows(&s, " set")), ["Settings"]);
        assert_eq!(numbers(&rows(&s, "zzz")), Vec::<String>::new());
    }

    #[test]
    fn masking_keeps_three_characters() {
        assert_eq!(mask_email("work@company.com", true), "wor••••••••");
        assert_eq!(mask_email("ab@x.io", true), "ab••••");
        assert_eq!(mask_email("work@company.com", false), "work@company.com");
    }
}
