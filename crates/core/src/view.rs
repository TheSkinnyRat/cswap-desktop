//! View model of the Accounts page that is worth testing without a window: the generated
//! window columns (5-hour, 7-day, then one per scoped model across ALL accounts), the
//! table/card threshold, plan badge text. OWNER: ui-accounts.
//!
//! Also the pure parts of the Auto and Mappings pages (event lines, config specs), for the
//! same reason: a wrong word or a missing cell here fails silently in a window.

use serde_json::Value;

use crate::format::{Tone, clock_of, pct, rel_time, reset_text};
use crate::plan::{self, Standing};
use crate::types::{Account, AutoEvent, AutoOutcome, AutoStatus, PlainOutput, PlanInfo, Usage, UsageWindow};

/// Cards below this LIST width, table above. Deliberately NOT derived from the column
/// template (Purwa's call, 10 Sep 2026, PR #2): three window columns want ≈956 px, and
/// deriving it turned a usable window into cards. Between 780 and the template's minimum
/// the list scrolls sideways on purpose; nothing else does.
pub const CARD_BELOW: f32 = 780.0;

/// Column template, px (0.1.23's `T`): `#`, account (`minmax(160,2fr)`), one
/// `minmax(win,1fr)` per window, status, actions. Rings are narrower than bars.
pub const COL_NUM: f32 = 28.0;
pub const COL_ACCOUNT: f32 = 160.0;
pub const COL_WIN: f32 = 132.0;
pub const COL_RING: f32 = 116.0;
pub const COL_STATUS: f32 = 148.0;
pub const COL_ACTIONS: f32 = 104.0;
/// `gap-4` between columns and `px-3` inside a row.
pub const COL_GAP: f32 = 16.0;
pub const ROW_PAD_X: f32 = 12.0;

/// The usage a row shows: the live measurement, else the last good one (stale).
pub fn usage_of(a: &Account) -> Option<&Usage> {
    a.usage.as_ref().or(a.last_good_usage.as_ref())
}

/// Showing `lastGoodUsage` because the live fetch failed.
pub fn is_stale(a: &Account) -> bool {
    a.usage.is_none() && a.last_good_usage.is_some()
}

/// Every per-model window in the list, first-seen order. The columns belong to the table
/// and not to a row: an account without one needs an empty cell, or its status and
/// actions slide one column left of everyone else's.
pub fn models(accounts: &[Account]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for a in accounts {
        for s in usage_of(a).and_then(|u| u.scoped.as_ref()).into_iter().flatten() {
            if !seen.contains(&s.name) {
                seen.push(s.name.clone());
            }
        }
    }
    seen
}

/// Table or cards, from the width the LIST has (not the window's).
pub fn is_cards(list_width: f32) -> bool {
    list_width > 0.0 && list_width < CARD_BELOW
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowCell {
    /// `5h`, `7d` or `model-<name>`; stable, for tests and widget ids.
    pub key: String,
    /// What the tooltip calls it: `5-hour window`, `7-day window`, the model's name.
    pub tooltip_name: String,
    /// The name a card prints before the percentage (no header there to say it).
    pub card_name: String,
    /// `None` = an empty cell (`-`) holding the column.
    pub window: Option<UsageWindow>,
    /// Weekly windows carry a pace; the chip "ahead" only shows outside model cells.
    pub is_model: bool,
}

/// The window cells of one row. In a table: 5-hour, 7-day, then EVERY model the table
/// knows, empty where this account has none. In a card: there are no columns to hold and
/// no header naming them, so a model the account lacks is simply absent.
pub fn window_cells(a: &Account, models: &[String], cards: bool) -> Vec<WindowCell> {
    let u = usage_of(a);
    let mut cells = vec![
        WindowCell {
            key: "5h".into(),
            tooltip_name: "5-hour window".into(),
            card_name: "5-hour".into(),
            window: u.and_then(|u| u.five_hour.clone()),
            is_model: false,
        },
        WindowCell {
            key: "7d".into(),
            tooltip_name: "7-day window".into(),
            card_name: "7-day".into(),
            window: u.and_then(|u| u.seven_day.clone()),
            is_model: false,
        },
    ];
    for m in models {
        let w =
            u.and_then(|u| u.scoped.as_ref()).and_then(|s| s.iter().find(|s| &s.name == m)).map(|s| s.window.clone());
        if cards && w.is_none() {
            continue;
        }
        cells.push(WindowCell {
            key: format!("model-{m}"),
            tooltip_name: m.clone(),
            card_name: m.clone(),
            window: w,
            is_model: true,
        });
    }
    cells
}

/// Column widths of the table at a list width, like the CSS grid 0.1.23 used:
/// `[num, account (2fr), win × (2 + models) (1fr each), status, actions]`. Every window
/// column gets the same width; when the list is narrower than the minimums the columns
/// stay at their minimums and the list scrolls.
pub fn grid_columns(list_width: f32, n_models: usize, rings: bool) -> Vec<f32> {
    let win = if rings { COL_RING } else { COL_WIN };
    let n_win = 2 + n_models;
    let n_cols = 4 + n_win;
    let fixed = COL_NUM + COL_STATUS + COL_ACTIONS;
    let free = list_width - 2.0 * ROW_PAD_X - COL_GAP * (n_cols - 1) as f32 - fixed;
    // `minmax(min, N fr)` as CSS grid resolves it: share the free space by flex factor;
    // a track whose share is below its minimum is frozen at the minimum and the rest is
    // shared again among the others. Below the sum of minimums everything sits at its min.
    let mins: Vec<f32> = std::iter::once(COL_ACCOUNT).chain(std::iter::repeat_n(win, n_win)).collect();
    let flex: Vec<f32> = std::iter::once(2.0).chain(std::iter::repeat_n(1.0, n_win)).collect();
    let mut frozen = vec![false; mins.len()];
    let mut sizes = mins.clone();
    loop {
        let rest: f32 = free - (0..mins.len()).filter(|&i| frozen[i]).map(|i| mins[i]).sum::<f32>();
        let fr_total: f32 = (0..mins.len()).filter(|&i| !frozen[i]).map(|i| flex[i]).sum();
        if fr_total == 0.0 {
            break;
        }
        let fr = rest / fr_total;
        let mut changed = false;
        for i in 0..mins.len() {
            if !frozen[i] && flex[i] * fr < mins[i] {
                frozen[i] = true;
                changed = true;
            }
        }
        if !changed {
            for i in 0..mins.len() {
                sizes[i] = if frozen[i] { mins[i] } else { flex[i] * fr };
            }
            break;
        }
    }
    let (account, window) = (sizes[0], sizes[1]);
    let mut cols = vec![COL_NUM, account];
    cols.extend(std::iter::repeat_n(window, n_win));
    cols.extend([COL_STATUS, COL_ACTIONS]);
    cols
}

/// The width under which the table scrolls sideways: ≈956 px with three window columns.
pub fn table_min_width(n_models: usize, rings: bool) -> f32 {
    let cols = grid_columns(0.0, n_models, rings);
    cols.iter().sum::<f32>() + COL_GAP * (cols.len() - 1) as f32 + 2.0 * ROW_PAD_X
}

/// Hover text of a window cell.
pub fn window_tooltip(w: &UsageWindow, name: &str, now_ms: i64, offset_secs: i64) -> String {
    let mut lines = vec![format!("{name} · {} used", pct(Some(w.pct)))];
    if let Some(r) = &w.resets_at {
        lines.push(format!("resets {} · {}", clock_of(Some(r), now_ms, offset_secs), reset_text(Some(r), now_ms)));
    }
    if let Some(e) = w.expected_pct {
        let pace = if w.ahead_of_pace == Some(true) { "ahead of pace" } else { "on pace" };
        lines.push(format!("{pace} — spread evenly you'd be at ~{} by now", pct(Some(e))));
    }
    if w.will_last_to_reset == Some(false) {
        lines.push("at this rate it runs out before the reset".into());
    }
    lines.join("\n")
}

/// Hover text of a remembered plan badge (older cswap, see [`plan_badge`]). The label
/// itself is the driver's (`plan.rs`); a badge on a switched-away account is the plan
/// seen when it was last active.
pub fn plan_tooltip(plan: &PlanInfo, active: bool, now_ms: i64, offset_secs: i64) -> String {
    if active {
        match &plan.tier {
            Some(t) => format!("Subscription: {} ({t})", plan.label),
            None => format!("Subscription: {}", plan.label),
        }
    } else {
        format!(
            "Subscription when this account was last active, {}",
            clock_of(Some(&plan.seen_at), now_ms, offset_secs)
        )
    }
}

/// The plan chip of a row, plus a second, quiet chip when the subscription is canceled
/// or has lapsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanBadge {
    pub label: String,
    pub tooltip: String,
    pub standing: Standing,
}

impl PlanBadge {
    /// Word, tone and hover text of the standing chip; `None` while current. Words, not
    /// a colour alone: the two states must read apart without telling hues apart.
    pub fn standing_chip(&self) -> Option<(&'static str, ChipTone, &'static str)> {
        match self.standing {
            Standing::Current => None,
            Standing::Canceled => Some(("canceled", ChipTone::Muted, CANCELED_TIP)),
            Standing::Lapsed => Some(("lapsed", ChipTone::Warn, LAPSED_TIP)),
        }
    }
}

const CANCELED_TIP: &str = "Subscription canceled: the plan still applies until the paid period ends";
const LAPSED_TIP: &str = "Subscription lapsed: canceled and back on the free tier";

/// The badge of an account: cswap's `plan` when the row has one (every account, cswap >=
/// 0.30), else the label remembered in `plans.json` from Claude Code's credential file
/// (older cswap, or a slot cswap never read). Remembered labels carry no standing.
pub fn plan_badge(a: &Account, remembered: Option<&PlanInfo>, now_ms: i64, offset_secs: i64) -> Option<PlanBadge> {
    if let Some(p) = &a.plan
        && let Some((label, standing)) = plan::from_cswap(p)
    {
        let mut lines = vec![if p.tier.is_empty() {
            format!("Subscription: {label}")
        } else {
            format!("Subscription: {label} ({})", p.tier)
        }];
        match standing {
            Standing::Current => {}
            Standing::Canceled => lines.push(CANCELED_TIP.into()),
            Standing::Lapsed => lines.push(LAPSED_TIP.into()),
        }
        let at = clock_of(p.fetched_at.as_deref(), now_ms, offset_secs);
        if !at.is_empty() {
            lines.push(format!("Read by cswap {at}"));
        }
        return Some(PlanBadge { label, tooltip: lines.join("\n"), standing });
    }
    let r = remembered?;
    Some(PlanBadge {
        label: r.label.clone(),
        tooltip: plan_tooltip(r, a.active, now_ms, offset_secs),
        standing: Standing::Current,
    })
}

/// `4 accounts`.
pub fn count_text(n: usize) -> String {
    format!("{n} account{}", if n == 1 { "" } else { "s" })
}

/// An alias cswap accepts: letters, digits, `.`, `-`, `_`, not purely numeric (a number
/// is a slot). Empty is "valid" (nothing typed yet) but never submittable.
pub fn alias_valid(alias: &str) -> bool {
    alias.is_empty()
        || (alias.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
            && !alias.bytes().all(|c| c.is_ascii_digit()))
}

/// The remove dialog's guard: the slot number or the email, typed.
pub fn remove_confirmed(typed: &str, a: &Account) -> bool {
    let t = typed.trim();
    t == a.number.to_string() || t.to_lowercase() == a.email.to_lowercase()
}

/// What a pasted token looks like, for the hint under the field.
pub fn token_kind(token: &str) -> Option<&'static str> {
    if token.starts_with("sk-ant-api") {
        Some("API key")
    } else if token.starts_with("sk-ant-oat") {
        Some("setup-token")
    } else {
        None
    }
}

/// Slot field input: digits only (the field strips the rest as you type).
pub fn digits_only(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// The last line of a CLI's output, for a toast.
pub fn out_text(v: &PlainOutput) -> String {
    let s = if !v.stdout.is_empty() {
        &v.stdout
    } else if !v.stderr.is_empty() {
        &v.stderr
    } else {
        "Done"
    };
    s.trim().lines().last().unwrap_or("").to_string()
}

/// cswap colours its `--token-status` output; the dialog shows plain text.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\x1b' && it.peek() == Some(&'[') {
            it.next();
            while let Some(&n) = it.peek() {
                if n.is_ascii_digit() || n == ';' {
                    it.next();
                } else {
                    break;
                }
            }
            if it.peek() == Some(&'m') {
                it.next();
            }
            continue;
        }
        out.push(c);
    }
    out
}

// ---- Auto-switch page -------------------------------------------------------------

/// Chip colours; `Accent` exists only for chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipTone {
    Ok,
    Warn,
    Danger,
    Muted,
    Accent,
}

impl From<Tone> for ChipTone {
    fn from(t: Tone) -> Self {
        match t {
            Tone::Ok => ChipTone::Ok,
            Tone::Warn => ChipTone::Warn,
            Tone::Danger => ChipTone::Danger,
            Tone::Muted => ChipTone::Muted,
        }
    }
}

pub fn event_tone(kind: &str) -> ChipTone {
    match kind {
        "switch" => ChipTone::Accent,
        "error" | "all-exhausted" => ChipTone::Danger,
        "stderr" | "account-quarantined" | "config-warning" => ChipTone::Warn,
        "account-unquarantined" => ChipTone::Ok,
        _ => ChipTone::Muted,
    }
}

/// The feed's default filter hides the heartbeat (`poll`, `sleep`).
pub fn is_important(ev: &AutoEvent) -> bool {
    ev.event != "poll" && ev.event != "sleep"
}

fn field<'a>(ev: &'a AutoEvent, k: &str) -> Option<&'a Value> {
    ev.extra.get(k).filter(|v| !v.is_null())
}

// JSON.stringify for what the feed prints: integral numbers without ".0".
fn js_string(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", f as i64),
            _ => n.to_string(),
        },
        Some(v) => v.to_string(),
    }
}

fn account_ref(v: Option<&Value>) -> String {
    match v {
        Some(Value::Object(m)) => {
            let n = m.get("number").filter(|v| !v.is_null()).map(|v| js_string(Some(v))).unwrap_or_else(|| "?".into());
            let e = m.get("email").and_then(Value::as_str).unwrap_or("");
            format!("Account-{n} ({e})")
        }
        _ => "—".into(),
    }
}

fn round(v: &Value) -> String {
    v.as_f64().map(|f| format!("{}", (f + 0.5).floor() as i64)).unwrap_or_else(|| "?".into())
}

// JavaScript orders integer-like object keys numerically ("2" before "10").
fn sorted_entries(m: &serde_json::Map<String, Value>) -> Vec<(&String, &Value)> {
    let mut v: Vec<_> = m.iter().collect();
    v.sort_by_key(|(k, _)| k.parse::<u64>().map_or((1, u64::MAX), |n| (0, n)));
    v
}

/// One line of the event feed, as 0.1.23 wrote it.
pub fn describe_event(ev: &AutoEvent, now_ms: i64, offset_secs: i64) -> String {
    match ev.event.as_str() {
        "poll" => {
            let mut parts: Vec<String> = Vec::new();
            if let Some(Value::Object(w)) = field(ev, "windowsPct") {
                for (n, wins) in sorted_entries(w) {
                    let inner = match wins {
                        Value::Object(m) => sorted_entries(m)
                            .iter()
                            .map(|(k, v)| format!("{k} {}%", round(v)))
                            .collect::<Vec<_>>()
                            .join(" · "),
                        _ => String::new(),
                    };
                    parts.push(format!("#{n} {inner}"));
                }
            }
            if parts.is_empty()
                && let Some(Value::Object(h)) = field(ev, "headroomPct")
            {
                for (n, v) in sorted_entries(h) {
                    let used = v
                        .as_f64()
                        .map(|f| format!("{}%", (100.0 - f + 0.5).floor() as i64))
                        .unwrap_or_else(|| "?".into());
                    parts.push(format!("#{n} {used}"));
                }
            }
            format!(
                "active {} · threshold {}% · {}",
                account_ref(field(ev, "active")),
                js_string(field(ev, "threshold")),
                parts.join("  |  ")
            )
        }
        "switch" => {
            let dry = field(ev, "dryRun").and_then(Value::as_bool) == Some(true);
            let warnings: Vec<String> = field(ev, "warnings")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|w| js_string(Some(w))).collect())
                .unwrap_or_default();
            format!(
                "{} {} → {} · {}{}",
                if dry { "[dry run] would switch" } else { "switched" },
                account_ref(field(ev, "from")),
                account_ref(field(ev, "to")),
                js_string(field(ev, "trigger")),
                if warnings.is_empty() { String::new() } else { format!(" · {}", warnings.join("; ")) }
            )
        }
        "no-switch" => {
            let detail = js_string(field(ev, "detail"));
            let reason = js_string(field(ev, "reason"));
            if detail.is_empty() { reason } else { format!("{reason} — {detail}") }
        }
        "sleep" => {
            let until = js_string(field(ev, "until"));
            format!(
                "next check in {}s ({})",
                js_string(field(ev, "seconds")),
                clock_of(Some(&until), now_ms, offset_secs)
            )
        }
        "account-quarantined" | "account-unquarantined" => format!(
            "Account-{} ({}) · {}",
            js_string(field(ev, "number")),
            js_string(field(ev, "email")),
            js_string(field(ev, "reason"))
        ),
        "all-exhausted" => {
            let at = field(ev, "earliestResetAt").map(|v| js_string(Some(v)));
            let when = match at.filter(|s| !s.is_empty()) {
                Some(s) => rel_time(Some(&s), now_ms),
                None => "unknown".into(),
            };
            format!("every account is at its limit · earliest reset {when}")
        }
        _ => js_string(field(ev, "message").or_else(|| field(ev, "detail"))),
    }
}

/// Toast after `auto --once`: (tone, text).
pub fn once_outcome(outcome: AutoOutcome, exit_code: Option<i32>) -> (ChipTone, String) {
    match outcome {
        AutoOutcome::Switched => (ChipTone::Ok, "Switched account".into()),
        AutoOutcome::NoAction => (ChipTone::Muted, "Nothing to do — below threshold".into()),
        AutoOutcome::Blocked => (ChipTone::Danger, "Wanted to switch, but no viable target".into()),
        AutoOutcome::Error => (ChipTone::Danger, "Check reported an error".into()),
        AutoOutcome::Unknown => {
            (ChipTone::Muted, format!("Exit {}", exit_code.map_or_else(|| "null".to_string(), |c| c.to_string())))
        }
    }
}

/// The State stat: (text, tone).
pub fn auto_state(st: &AutoStatus) -> (String, ChipTone) {
    if st.running {
        let t = if st.dry_run == Some(true) { "Running · dry run" } else { "Running" };
        (t.into(), ChipTone::Ok)
    } else if let Some(c) = st.exit_code.filter(|c| *c != 0) {
        (format!("Exited ({c})"), ChipTone::Danger)
    } else {
        ("Stopped".into(), ChipTone::Muted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecKind {
    Number,
    Select(&'static [&'static str]),
    Bool,
    Text,
}

/// One cswap setting the Auto page edits (`cswap config set`).
#[derive(Debug, Clone, Copy)]
pub struct ConfigSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub kind: SpecKind,
}

pub const CONFIG_SPECS: &[ConfigSpec] = &[
    ConfigSpec {
        key: "autoswitch.threshold",
        label: "Threshold",
        hint: "Switch when the binding 5h/7d window reaches this %. 50–99.9",
        kind: SpecKind::Number,
    },
    ConfigSpec {
        key: "autoswitch.intervalSeconds",
        label: "Poll interval (s)",
        hint: "How often the loop checks. 15–3600",
        kind: SpecKind::Number,
    },
    ConfigSpec {
        key: "autoswitch.cooldownSeconds",
        label: "Cooldown (s)",
        hint: "Minimum time between proactive switches",
        kind: SpecKind::Number,
    },
    ConfigSpec {
        key: "autoswitch.hysteresisPct",
        label: "Hysteresis (%)",
        hint: "A target must beat the active account by this much",
        kind: SpecKind::Number,
    },
    ConfigSpec {
        key: "autoswitch.strategy",
        label: "Strategy",
        hint: "best = most quota left · consume-first = burn the soonest-resetting weekly window first",
        kind: SpecKind::Select(&["best", "consume-first"]),
    },
    ConfigSpec {
        key: "autoswitch.model",
        label: "Model limits",
        hint: "Also switch when these per-model weekly limits are hit, e.g. Fable or Fable,Opus or all",
        kind: SpecKind::Text,
    },
    ConfigSpec {
        key: "autoswitch.unhealthyTicks",
        label: "Unhealthy ticks",
        hint: "Consecutive failed polls before an account is unhealthy",
        kind: SpecKind::Number,
    },
    ConfigSpec {
        key: "autoswitch.includeApiKeyAccounts",
        label: "Include API-key accounts",
        hint: "Allow rotating onto managed API-key accounts (bill per token)",
        kind: SpecKind::Bool,
    },
];

/// A config value as the input shows it (`String(row.value ?? '')`).
pub fn config_text(v: &Value) -> String {
    js_string(Some(v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{auto_event, fixture_payload};
    use crate::types::ScopedWindow;
    use serde_json::json;

    fn acct(n: u32, scoped: &[&str]) -> Account {
        let w = |p| UsageWindow { pct: p, ..Default::default() };
        Account {
            number: n,
            usage: Some(Usage {
                five_hour: Some(w(1.0)),
                seven_day: Some(w(2.0)),
                scoped: (!scoped.is_empty())
                    .then(|| scoped.iter().map(|s| ScopedWindow { name: s.to_string(), window: w(3.0) }).collect()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    // TESTED RULE: cswap's `plan` is the badge of every account that has one; the label
    // remembered from the credential file is only the fallback (older cswap, unread slot).
    #[test]
    fn plan_badge_prefers_cswap_and_falls_back_to_plans_json() {
        use crate::types::CswapPlan;
        let now = crate::format::parse_iso_ms("2026-11-28T10:00:00Z").unwrap();
        let remembered = PlanInfo {
            label: "Pro".into(),
            tier: Some("default_claude_pro".into()),
            subscription: Some("pro".into()),
            seen_at: "2026-11-27T10:00:00Z".into(),
        };
        let seat = CswapPlan {
            label: "Team".into(),
            tier: "default_claude_max_5x".into(),
            organization_type: Some("claude_team".into()),
            subscription_status: Some("active".into()),
            fetched_at: Some("2026-11-28T09:30:00Z".into()),
            ..Default::default()
        };
        let a = Account { email: "w@example.com".into(), plan: Some(seat), ..Default::default() };
        let b = plan_badge(&a, Some(&remembered), now, 0).unwrap();
        assert_eq!(b.label, "Team · Max 5x", "cswap wins over a remembered label");
        assert_eq!(b.tooltip, "Subscription: Team · Max 5x (default_claude_max_5x)\nRead by cswap 09:30");
        assert_eq!(b.standing_chip(), None);

        // no `plan` on the row: the remembered label, as before
        let old = Account { email: "w@example.com".into(), ..Default::default() };
        let b = plan_badge(&old, Some(&remembered), now, 0).unwrap();
        assert_eq!((b.label.as_str(), b.standing), ("Pro", Standing::Current));
        assert!(b.tooltip.starts_with("Subscription when this account was last active"), "{}", b.tooltip);
        assert_eq!(plan_badge(&old, None, now, 0), None);

        // a `plan` that names nothing does not hide the remembered one
        let blank = Account { plan: Some(CswapPlan::default()), ..old.clone() };
        assert_eq!(plan_badge(&blank, Some(&remembered), now, 0).unwrap().label, "Pro");
    }

    #[test]
    fn canceled_and_lapsed_get_their_own_quiet_chip() {
        use crate::types::CswapPlan;
        let plan = |label: &str, tier: &str, status: &str, expired| Account {
            plan: Some(CswapPlan {
                label: label.into(),
                tier: tier.into(),
                subscription_status: Some(status.into()),
                expired,
                ..Default::default()
            }),
            ..Default::default()
        };
        let c = plan_badge(&plan("Max 20x", "default_claude_max_20x", "canceled", false), None, 0, 0).unwrap();
        let (word, tone, tip) = c.standing_chip().unwrap();
        assert_eq!((c.label.as_str(), word, tone), ("Max 20x", "canceled", ChipTone::Muted));
        assert!(c.tooltip.contains(tip), "the plan's own tooltip says it too: {}", c.tooltip);
        let l = plan_badge(&plan("Free", "default_claude_ai", "canceled", true), None, 0, 0).unwrap();
        let (word, tone, tip) = l.standing_chip().unwrap();
        assert_eq!((l.label.as_str(), word, tone), ("Free", "lapsed", ChipTone::Warn));
        assert!(l.tooltip.contains(tip));
        assert!(!l.tooltip.contains("Read by cswap"), "no fetchedAt, no empty clock");
    }

    // TESTED RULE (project notes): the column set is the UNION of models across accounts,
    // first-seen order, and it includes models only present in lastGoodUsage.
    #[test]
    fn model_columns_are_the_union_across_accounts() {
        let list = [acct(1, &[]), acct(2, &["Fable"]), acct(3, &["Opus", "Fable"])];
        assert_eq!(models(&list), ["Fable", "Opus"]);
        assert!(models(&[acct(1, &[])]).is_empty());
        // The fixture: Fable live on 1 and 2, Sonnet only in account 5's last-good usage.
        assert_eq!(models(&fixture_payload(0).accounts), ["Fable", "Sonnet"]);
    }

    // TESTED RULE: in a table an account without a model still gets that column, as an
    // empty cell; in a card it is absent.
    #[test]
    fn a_missing_model_is_an_empty_cell_in_a_table_and_absent_in_a_card() {
        let list = [acct(1, &[]), acct(2, &["Fable"])];
        let m = models(&list);
        let table = window_cells(&list[0], &m, false);
        assert_eq!(table.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(), ["5h", "7d", "model-Fable"]);
        assert!(table[2].window.is_none(), "empty cell holds the column");
        let card = window_cells(&list[0], &m, true);
        assert_eq!(card.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(), ["5h", "7d"]);
        // A row with no usage at all still has its 5h/7d cells (shown as "-").
        let none = Account { number: 4, ..Default::default() };
        let cells = window_cells(&none, &m, false);
        assert_eq!(cells.len(), 3);
        assert!(cells.iter().all(|c| c.window.is_none()));
    }

    // TESTED RULE: cards below a FIXED 780 px list width, not one derived from columns.
    #[test]
    fn cards_below_780_regardless_of_columns() {
        assert!(is_cards(779.0));
        assert!(!is_cards(780.0));
        assert!(!is_cards(800.0), "the table scrolls instead, even with many model columns");
        assert!(table_min_width(3, false) > 800.0);
        assert!(!is_cards(0.0), "unmeasured: not cards");
    }

    #[test]
    fn grid_matches_the_css_template() {
        // Three window columns want ≈956 px: the scroll band is 780..956.
        assert_eq!(table_min_width(1, false), 956.0);
        assert!(table_min_width(1, true) < 956.0);
        // Below the minimum: columns at their minimums (the list scrolls).
        let c = grid_columns(800.0, 1, false);
        assert_eq!(c, [28.0, 160.0, 132.0, 132.0, 132.0, 148.0, 104.0]);
        // Wide: window columns equal, account twice a window column, all filling the row.
        for w in [1000.0, 1240.0, 1700.0] {
            let c = grid_columns(w, 1, false);
            assert!((c[2] - c[3]).abs() < 0.01 && (c[3] - c[4]).abs() < 0.01, "{w}: {c:?}");
            let used: f32 = c.iter().sum::<f32>() + COL_GAP * 6.0 + 2.0 * ROW_PAD_X;
            assert!((used - w).abs() < 0.5, "{w}: fills the row ({used})");
            assert!(c[1] >= COL_ACCOUNT && c[2] >= COL_WIN);
        }
        let c = grid_columns(1700.0, 1, false);
        assert!((c[1] - 2.0 * c[2]).abs() < 0.01);
    }

    #[test]
    fn tooltip_names_the_window_and_the_pace() {
        let now = 1_795_800_000_000;
        let p = fixture_payload(now);
        let w = p.accounts[0].usage.as_ref().unwrap().seven_day.clone().unwrap();
        let t = window_tooltip(&w, "7-day window", now, 0);
        assert!(t.starts_with("7-day window · 12% used\nresets "), "{t}");
        assert!(t.contains("on pace — spread evenly you'd be at ~50% by now"), "{t}");
        let w2 = p.accounts[1].usage.as_ref().unwrap().seven_day.clone().unwrap();
        assert!(window_tooltip(&w2, "7-day window", now, 0).contains("ahead of pace"));
        let w5 = p.accounts[0].usage.as_ref().unwrap().five_hour.clone().unwrap();
        assert!(!window_tooltip(&w5, "5-hour window", now, 0).contains("pace"));
    }

    #[test]
    fn dialog_rules() {
        assert!(alias_valid("work") && alias_valid("a.b-c_1") && alias_valid(""));
        assert!(!alias_valid("123"), "a number is a slot");
        assert!(!alias_valid("has space") && !alias_valid("ü"));
        let a = Account { number: 3, email: "Spare@Example.com".into(), ..Default::default() };
        assert!(remove_confirmed(" 3 ", &a) && remove_confirmed("spare@example.com", &a));
        assert!(!remove_confirmed("nope", &a) && !remove_confirmed("", &a));
        assert_eq!(token_kind("sk-ant-oat01-x"), Some("setup-token"));
        assert_eq!(token_kind("sk-ant-api03-x"), Some("API key"));
        assert_eq!(token_kind("x"), None);
        assert_eq!(digits_only("1a2 "), "12");
        let o = PlainOutput { stdout: "line one\nAdded Account 6\n".into(), ..Default::default() };
        assert_eq!(out_text(&o), "Added Account 6");
        assert_eq!(out_text(&PlainOutput::default()), "Done");
        assert_eq!(strip_ansi("\x1b[32mvalid\x1b[0m ok"), "valid ok");
    }

    #[test]
    fn auto_event_lines() {
        let now = 1_795_800_000_000;
        let poll = crate::mock::poll_event(now);
        assert_eq!(
            describe_event(&poll, now, 0),
            "active Account-1 (you@example.com) · threshold 90% · #1 5h 45% · 7d 12%  |  #2 5h 100% · 7d 61%"
        );
        let head = auto_event(now, "poll", json!({ "threshold": 90.0, "headroomPct": { "10": 40, "2": null } }));
        assert_eq!(describe_event(&head, now, 0), "active — · threshold 90% · #2 ?  |  #10 60%");
        let sw = auto_event(
            now,
            "switch",
            json!({ "dryRun": true, "from": { "number": 1, "email": "a@example.com" }, "to": { "number": 2, "email": "b@example.com" }, "trigger": "threshold", "warnings": ["w1", "w2"] }),
        );
        assert_eq!(
            describe_event(&sw, now, 0),
            "[dry run] would switch Account-1 (a@example.com) → Account-2 (b@example.com) · threshold · w1; w2"
        );
        let ns = auto_event(now, "no-switch", json!({ "reason": "below-threshold", "detail": "active headroom 55%" }));
        assert_eq!(describe_event(&ns, now, 0), "below-threshold — active headroom 55%");
        let ex = auto_event(now, "all-exhausted", json!({}));
        assert_eq!(describe_event(&ex, now, 0), "every account is at its limit · earliest reset unknown");
        let err = auto_event(now, "error", json!({ "message": "boom" }));
        assert_eq!(describe_event(&err, now, 0), "boom");
        assert!(!is_important(&poll) && is_important(&sw));
        assert_eq!(event_tone("switch"), ChipTone::Accent);
        assert_eq!(event_tone("brand-new-kind"), ChipTone::Muted);
    }

    #[test]
    fn auto_state_and_outcomes() {
        let mut st = AutoStatus { running: true, dry_run: Some(true), ..Default::default() };
        assert_eq!(auto_state(&st).0, "Running · dry run");
        st = AutoStatus { exit_code: Some(3), ..Default::default() };
        assert_eq!(auto_state(&st), ("Exited (3)".into(), ChipTone::Danger));
        st.exit_code = Some(0);
        assert_eq!(auto_state(&st).0, "Stopped");
        assert_eq!(once_outcome(AutoOutcome::NoAction, Some(2)).1, "Nothing to do — below threshold");
        assert_eq!(once_outcome(AutoOutcome::Unknown, Some(7)).1, "Exit 7");
        assert_eq!(config_text(&json!(90)), "90");
        assert_eq!(config_text(&json!(false)), "false");
        assert_eq!(config_text(&json!(null)), "");
    }
}
