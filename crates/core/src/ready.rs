//! The Accounts page's 1.1.0 model (agreed with Purwa 28 Nov 2026): how ready each account
//! is to be switched to, the order the list shows them in, the hero's one-line forecast,
//! and the account cswap's `switch --strategy best` would pick (the Best next strip).
//! Pure, so the rules are tested here rather than read off a window.

use serde_json::Value;

use crate::format::{parse_iso_ms, reset_text, weekday_clock};
use crate::types::{Account, Usage};
use crate::view::{ChipTone, login_expiry, usage_of};

/// The list's groups, in the order it shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Readiness {
    /// The live account, in the list only while the hero is hidden.
    Active,
    Ready,
    /// At a 5-hour or 7-day limit (or refused by the API) until a reset.
    Limited,
    /// Disabled: cswap's rotation and strategies skip it.
    HeldOut,
    /// The stored login is gone or refused: only `/login` + `cswap add` fixes it.
    Relogin,
}

impl Readiness {
    pub fn label(self) -> &'static str {
        match self {
            Readiness::Active => "Active now",
            Readiness::Ready => "Ready",
            Readiness::Limited => "Limited",
            Readiness::HeldOut => "Held out",
            Readiness::Relogin => "Re-login needed",
        }
    }

    /// Colour of the dot before the label (`Muted` = grey).
    pub fn tone(self) -> ChipTone {
        match self {
            Readiness::Active | Readiness::Ready => ChipTone::Ok,
            Readiness::Limited => ChipTone::Warn,
            Readiness::HeldOut => ChipTone::Muted,
            Readiness::Relogin => ChipTone::Danger,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadyInfo {
    pub kind: Readiness,
    /// The small line under the label: `66% left`, `free again in 30m`, `disabled`, ...
    pub detail: String,
}

/// The 5-hour and 7-day utilizations a usage reports, plus the 100% a refused probe
/// (`rejected`, cswap >= 1.1) stands for until it lifts.
fn base_windows(u: &Usage) -> Vec<(f64, Option<&str>)> {
    let mut out: Vec<(f64, Option<&str>)> = Vec::new();
    for w in [&u.five_hour, &u.seven_day].into_iter().flatten() {
        out.push((w.pct, w.resets_at.as_deref()));
    }
    if let Some(Value::Object(r)) = u.extra.get("rejected") {
        out.push((100.0, r.get("resetsAt").and_then(Value::as_str)));
    }
    out
}

/// Readiness of one account, from what its bars show (`usage`, else the last good one).
/// The live account is "Active now"; [`arrange`] only lists it while the hero is hidden.
pub fn readiness(a: &Account, now_ms: i64) -> ReadyInfo {
    let info = |kind, detail: &str| ReadyInfo { kind, detail: detail.to_owned() };
    let usage = usage_of(a);
    let left = usage.map(base_windows).filter(|w| !w.is_empty()).map(|w| {
        let worst = w.iter().map(|(p, _)| *p).fold(f64::MIN, f64::max);
        (100.0 - worst, w)
    });
    let left_text = |l: f64| format!("{}% left", crate::format::pct(Some(l.max(0.0))).trim_end_matches('%'));
    if a.active {
        return match &left {
            Some((l, _)) => info(Readiness::Active, &left_text(*l)),
            None => info(Readiness::Active, "usage unknown"),
        };
    }
    let expired = login_expiry(a, now_ms, 0).is_some_and(|l| l.chip.is_some_and(|(_, t)| t == ChipTone::Danger));
    if expired {
        return info(Readiness::Relogin, "login expired");
    }
    match a.usage_status.as_str() {
        "relogin_required" => return info(Readiness::Relogin, "login refused"),
        "no_credentials" => return info(Readiness::Relogin, "no stored login"),
        _ => {}
    }
    if a.disabled == Some(true) {
        return info(Readiness::HeldOut, "disabled");
    }
    match left {
        Some((l, w)) if l <= 0.0 => {
            // Free again once the LAST of the windows at their limit resets.
            let at = w
                .iter()
                .filter(|(p, _)| *p >= 100.0)
                .filter_map(|(_, r)| r.and_then(|r| parse_iso_ms(r).map(|ms| (ms, r))))
                .max_by_key(|(ms, _)| *ms);
            match at {
                Some((_, r)) if parse_iso_ms(r).is_some_and(|ms| ms > now_ms) => {
                    info(Readiness::Limited, &format!("free again in {}", reset_text(Some(r), now_ms)))
                }
                _ => info(Readiness::Limited, "at its limit"),
            }
        }
        Some((l, _)) => info(Readiness::Ready, &left_text(l)),
        None if a.usage_status == "api_key" => info(Readiness::Ready, "API key, no quota"),
        None => info(Readiness::Ready, "usage unknown"),
    }
}

/// The accounts the list shows, in its order: with the hero shown, every account but the
/// live one; otherwise all of them, the live one first ("Active now"). Then Ready,
/// Limited, Held out, Re-login needed. Inside a group: accounts whose usage is known
/// before those without (an API key, a failed read), then slot order, so a refresh that
/// moves a percentage does not reshuffle rows under the pointer.
pub fn arrange(accounts: &[Account], hero_shown: bool, now_ms: i64) -> Vec<(&Account, ReadyInfo)> {
    let mut out: Vec<(&Account, ReadyInfo)> =
        accounts.iter().filter(|a| !(hero_shown && a.active)).map(|a| (a, readiness(a, now_ms))).collect();
    out.sort_by_key(|(a, r)| (r.kind, usage_of(a).is_none(), a.number));
    out
}

// ---- Best next: cswap's `switch --strategy best`, ported -------------------------------
//
// MIRRORS claude-swap 1.2.0: `Switcher._select_best_switchable` (switcher.py),
// `oauth.account_headroom` / `oauth.relevant_windows`, the `models` argument the CLI
// builds from `--model` or the `autoswitch.model` setting (`settings.parse_model_names`),
// over the decision-grade `usage` that `list --json` carries (`UsageEntry.decision_value`:
// a sentinel or a last-good older than 5 min is `null` there). cswap has no dry run for
// this pick, and the strip's Switch runs the real `switch --strategy best`, so a drift
// here shows a wrong name but can never switch to it. Re-read those functions when cswap
// changes them. Not mirrored, because `list --json` does not say it: cswap also skips a
// slot whose stored config backup is missing; `no_credentials` stands in for both.

/// `settings.parse_model_names`: comma separated, trimmed, deduplicated ignoring case.
pub fn parse_models(value: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in value.split(',') {
        let name = part.trim();
        if !name.is_empty() && !out.iter().any(|m| m.eq_ignore_ascii_case(name)) {
            out.push(name.to_owned());
        }
    }
    out
}

/// `oauth.account_headroom`: 100 minus the most used of the windows that gate the account
/// (5-hour, 7-day, the per-model weekly windows `models` names or `all`, a refused probe).
/// `None` = unknown: no usage, a sentinel, or a header reading missing a window it needs.
pub fn headroom(usage: Option<&Usage>, models: &[String]) -> Option<f64> {
    let u = usage?;
    let mut pcts: Vec<f64> = Vec::new();
    let from_headers = u.extra.get("source").and_then(Value::as_str) == Some("headers");
    let wants_all = models.iter().any(|m| m.eq_ignore_ascii_case("all"));
    let has = |m: &String| u.scoped.iter().flatten().any(|s| s.name.eq_ignore_ascii_case(m));
    // `_header_reading_complete`: the 7-day window always, every named model, never `all`.
    let complete = u.seven_day.is_some() && !wants_all && models.iter().all(has);
    if !from_headers || complete {
        pcts.extend([&u.five_hour, &u.seven_day].into_iter().flatten().map(|w| w.pct));
        if !models.is_empty() {
            for s in u.scoped.iter().flatten() {
                if wants_all || models.iter().any(|m| m.eq_ignore_ascii_case(&s.name)) {
                    pcts.push(s.window.pct);
                }
            }
        }
    }
    if matches!(u.extra.get("rejected"), Some(Value::Object(_))) {
        pcts.push(100.0);
    }
    pcts.into_iter().reduce(f64::max).map(|m| 100.0 - m)
}

/// The slot `cswap switch --strategy best` would move to, or `None` when it would stay
/// (or there is no live managed account to compare with: cswap's fresh-machine path
/// ignores the strategy). Candidates: every other slot, in list (sequence) order, not
/// disabled and with a stored login; the first with the most headroom wins, and only when
/// it beats the live account's strictly — a tie stays put, and so does an unknown.
pub fn best_next(accounts: &[Account], models: &[String]) -> Option<u32> {
    let current = accounts.iter().find(|a| a.active)?;
    let current_headroom = headroom(current.usage.as_ref(), models)?;
    let mut best: Option<(f64, u32)> = None;
    for a in accounts {
        if a.number == current.number || a.disabled == Some(true) || a.usage_status == "no_credentials" {
            continue;
        }
        if let Some(h) = headroom(a.usage.as_ref(), models)
            && best.is_none_or(|(b, _)| h > b)
        {
            best = Some((h, a.number));
        }
    }
    best.filter(|(h, _)| *h > current_headroom).map(|(_, n)| n)
}

// ---- The hero's forecast ----------------------------------------------------------------

/// One line from cswap's weekly pace fields (`willLastToReset`, `projectedExhaustionAt`),
/// `None` when cswap sent neither (a week younger than a day, or an older cswap).
pub fn forecast(a: &Account, now_ms: i64, offset_secs: i64) -> Option<(String, ChipTone)> {
    let w = a.usage.as_ref()?.seven_day.as_ref()?;
    let reset = w.resets_at.as_deref();
    let eta = w.projected_exhaustion_at.as_deref();
    let lasts = match (w.will_last_to_reset, eta) {
        (Some(v), _) => v,
        (None, Some(e)) => match (parse_iso_ms(e), reset.and_then(parse_iso_ms)) {
            (Some(e), Some(r)) => e >= r,
            _ => return None,
        },
        (None, None) => return None,
    };
    if lasts {
        let left = reset_text(reset, now_ms);
        let tail = if left.is_empty() || left == "reset passed" { String::new() } else { format!(" ({left})") };
        Some((format!("At this pace the week lasts until it resets{tail}."), ChipTone::Ok))
    } else {
        let at = weekday_clock(eta, now_ms, offset_secs);
        let text = if at.is_empty() {
            "At this pace the week runs out before it resets.".to_owned()
        } else {
            format!("At this pace the week runs out around {at}.")
        };
        Some((text, ChipTone::Warn))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::iso_from_ms;
    use crate::types::{ScopedWindow, UsageWindow};
    use serde_json::json;

    const NOW: i64 = 1_795_800_000_000;

    fn w(pct: f64) -> Option<UsageWindow> {
        Some(UsageWindow { pct, resets_at: Some(iso_from_ms(NOW + 1_800_000)), ..Default::default() })
    }

    fn usage(h5: f64, d7: f64) -> Usage {
        Usage { five_hour: w(h5), seven_day: w(d7), ..Default::default() }
    }

    fn acct(n: u32, u: Option<Usage>) -> Account {
        Account {
            number: n,
            email: format!("a{n}@example.com"),
            usage_status: "ok".into(),
            usage: u,
            ..Default::default()
        }
    }

    fn scoped(mut u: Usage, name: &str, pct: f64) -> Usage {
        u.scoped.get_or_insert_with(Vec::new).push(ScopedWindow { name: name.into(), window: w(pct).unwrap() });
        u
    }

    // TESTED RULE: the four groups and their small line, as the mockup words them.
    #[test]
    fn readiness_classifies_and_words_each_group() {
        let r = |a: &Account| readiness(a, NOW);
        let ready = acct(2, Some(usage(8.0, 34.0)));
        assert_eq!(r(&ready), ReadyInfo { kind: Readiness::Ready, detail: "66% left".into() });

        let limited = acct(3, Some(usage(100.0, 61.0)));
        assert_eq!(r(&limited), ReadyInfo { kind: Readiness::Limited, detail: "free again in 30m".into() });
        let mut weekly = usage(20.0, 100.0);
        weekly.seven_day.as_mut().unwrap().resets_at = Some(iso_from_ms(NOW + 2 * 86_400_000));
        assert_eq!(r(&acct(3, Some(weekly))).detail, "free again in 2d 0h", "the window that binds longest");
        let mut refused = usage(10.0, 10.0);
        refused.extra.insert("rejected".into(), json!({ "retryAfterS": 60, "resetsAt": iso_from_ms(NOW + 60_000) }));
        assert_eq!(r(&acct(3, Some(refused))).kind, Readiness::Limited, "a refused probe is a limit");

        let held = Account { disabled: Some(true), ..ready.clone() };
        assert_eq!(r(&held), ReadyInfo { kind: Readiness::HeldOut, detail: "disabled".into() });

        let gone = Account { login_expires_at: Some(iso_from_ms(NOW - 1000)), ..held.clone() };
        assert_eq!(r(&gone), ReadyInfo { kind: Readiness::Relogin, detail: "login expired".into() }, "beats disabled");
        let refused = Account { usage_status: "relogin_required".into(), usage: None, ..acct(4, None) };
        assert_eq!(r(&refused).kind, Readiness::Relogin);
        let empty = Account { usage_status: "no_credentials".into(), ..acct(4, None) };
        assert_eq!(r(&empty).detail, "no stored login");

        // token_expired heals by itself (cswap retries): not a re-login, usage unknown
        let healing = Account { usage_status: "token_expired".into(), ..acct(5, None) };
        assert_eq!(r(&healing), ReadyInfo { kind: Readiness::Ready, detail: "usage unknown".into() });
        // a last-good measurement still says what the bars show
        let stale = Account { last_good_usage: Some(usage(100.0, 1.0)), ..healing };
        assert_eq!(r(&stale).kind, Readiness::Limited);
        let key = Account { usage_status: "api_key".into(), ..acct(6, None) };
        assert_eq!(r(&key).detail, "API key, no quota");

        let live = Account { active: true, ..acct(1, Some(usage(45.0, 12.0))) };
        assert_eq!(r(&live), ReadyInfo { kind: Readiness::Active, detail: "55% left".into() });
    }

    // TESTED RULES: Ready, Limited, Held out, Re-login needed, slot order inside a group;
    // the hero takes the live account out of the list, hiding it puts it back on top.
    #[test]
    fn arrange_sorts_by_readiness_and_the_hero_owns_the_active_account() {
        let list = vec![
            Account { active: true, ..acct(1, Some(usage(45.0, 12.0))) },
            acct(2, Some(usage(100.0, 61.0))),
            Account { disabled: Some(true), ..acct(3, Some(usage(3.0, 40.0))) },
            Account { usage_status: "relogin_required".into(), ..acct(4, None) },
            acct(5, Some(usage(8.0, 34.0))),
            acct(6, Some(usage(50.0, 50.0))),
            Account { usage_status: "api_key".into(), ..acct(0, None) },
        ];
        let order = |hero| arrange(&list, hero, NOW).iter().map(|(a, _)| a.number).collect::<Vec<_>>();
        assert_eq!(order(true), [5, 6, 0, 2, 3, 4], "a Ready account without usage after those with it");
        assert_eq!(order(false), [1, 5, 6, 0, 2, 3, 4]);
        assert_eq!(arrange(&list, false, NOW)[0].1.kind, Readiness::Active);
    }

    // PARITY with claude-swap 1.2.0 `_select_best_switchable`, case by case.
    #[test]
    fn best_next_follows_cswaps_best_strategy() {
        let none: &[String] = &[];
        let live = |h5, d7| Account { active: true, ..acct(1, Some(usage(h5, d7))) };

        // most headroom wins (binding window = the most used one)
        let list = vec![live(45.0, 12.0), acct(2, Some(usage(100.0, 61.0))), acct(3, Some(usage(8.0, 34.0)))];
        assert_eq!(best_next(&list, none), Some(3));

        // strictly better only: a tie with the live account stays put
        let list = vec![live(40.0, 10.0), acct(2, Some(usage(40.0, 40.0)))];
        assert_eq!(best_next(&list, none), None);

        // ties between candidates: the first in sequence order
        let list = vec![live(90.0, 10.0), acct(4, Some(usage(20.0, 0.0))), acct(2, Some(usage(20.0, 5.0)))];
        assert_eq!(best_next(&list, none), Some(4));

        // disabled and credential-less slots are never candidates
        let list = vec![
            live(90.0, 10.0),
            Account { disabled: Some(true), ..acct(2, Some(usage(0.0, 0.0))) },
            Account { usage_status: "no_credentials".into(), ..acct(3, Some(usage(0.0, 0.0))) },
            acct(4, Some(usage(50.0, 0.0))),
        ];
        assert_eq!(best_next(&list, none), Some(4));

        // unknown usage is never picked; the live account unknown -> no pick at all
        let list = vec![live(90.0, 10.0), Account { usage_status: "token_expired".into(), ..acct(2, None) }];
        assert_eq!(best_next(&list, none), None);
        let list = vec![Account { active: true, ..acct(1, None) }, acct(2, Some(usage(0.0, 0.0)))];
        assert_eq!(best_next(&list, none), None, "current-unavailable");
        // no live managed account: cswap's fresh-machine path ignores the strategy
        assert_eq!(best_next(&[acct(2, Some(usage(0.0, 0.0)))], none), None);
        // a last-good measurement is display-only: `usage` alone decides
        let stale = Account { usage: None, last_good_usage: Some(usage(0.0, 0.0)), ..acct(2, None) };
        assert_eq!(best_next(&[live(90.0, 10.0), stale], none), None);

        // everything exhausted: stay
        let list = vec![live(100.0, 10.0), acct(2, Some(usage(100.0, 0.0)))];
        assert_eq!(best_next(&list, none), None);
    }

    #[test]
    fn best_next_folds_in_the_configured_models_like_cswap() {
        let models = parse_models(" fable, Fable ,,Opus");
        assert_eq!(models, ["fable", "Opus"], "trimmed, deduplicated ignoring case, first spelling");
        let live = Account { active: true, ..acct(1, Some(usage(50.0, 10.0))) };
        let maxed = acct(2, Some(scoped(usage(5.0, 5.0), "Fable", 100.0)));
        let fine = acct(3, Some(scoped(usage(30.0, 30.0), "Sonnet", 100.0)));
        let list = vec![live, maxed, fine];
        assert_eq!(best_next(&list, &[]), Some(2), "without models only 5h/7d count");
        assert_eq!(best_next(&list, &models), Some(3), "Fable at 100% binds account 2");
        assert_eq!(best_next(&list, &["all".into()]), None, "`all`: every scoped window binds");
        assert_eq!(headroom(list[2].usage.as_ref(), &["all".into()]), Some(0.0));
    }

    #[test]
    fn headroom_reads_header_probes_and_refusals_like_cswap() {
        let mut probe = usage(5.0, 20.0);
        probe.extra.insert("source".into(), json!("headers"));
        assert_eq!(headroom(Some(&probe), &[]), Some(80.0), "complete: 7d present, no models");
        assert_eq!(headroom(Some(&probe), &["Fable".into()]), None, "a needed model window is missing");
        let mut no_week = probe.clone();
        no_week.seven_day = None;
        assert_eq!(headroom(Some(&no_week), &[]), None, "a header reading without 7d is unknown");
        let mut refused = usage(5.0, 5.0);
        refused.extra.insert("rejected".into(), json!({ "retryAfterS": 30, "resetsAt": null }));
        assert_eq!(headroom(Some(&refused), &[]), Some(0.0));
        assert_eq!(headroom(Some(&Usage::default()), &[]), None, "no window data");
    }

    #[test]
    fn forecast_words_cswaps_pace_fields() {
        let mk = |last: Option<bool>, eta: Option<i64>| {
            let mut d7 = w(12.0).unwrap();
            d7.resets_at = Some(iso_from_ms(NOW + 2 * 86_400_000 + 2 * 3_600_000));
            d7.will_last_to_reset = last;
            d7.projected_exhaustion_at = eta.map(|h| iso_from_ms(NOW + h * 3_600_000));
            Account { usage: Some(Usage { seven_day: Some(d7), ..Default::default() }), ..Default::default() }
        };
        assert_eq!(
            forecast(&mk(Some(true), None), NOW, 0),
            Some(("At this pace the week lasts until it resets (2d 2h).".into(), ChipTone::Ok))
        );
        // NOW is Friday 27 Nov 17:20 UTC
        assert_eq!(
            forecast(&mk(Some(false), Some(4 * 24 - 4)), NOW, 0),
            Some(("At this pace the week runs out around Tue 13:20.".into(), ChipTone::Warn))
        );
        assert_eq!(
            forecast(&mk(Some(false), None), NOW, 0).unwrap().0,
            "At this pace the week runs out before it resets."
        );
        assert_eq!(forecast(&mk(None, Some(10)), NOW, 0).unwrap().1, ChipTone::Warn, "an ETA before the reset");
        assert_eq!(forecast(&mk(None, Some(100)), NOW, 0).unwrap().1, ChipTone::Ok, "an ETA after it");
        assert_eq!(forecast(&mk(None, None), NOW, 0), None, "nothing sent: no line");
        assert_eq!(forecast(&Account::default(), NOW, 0), None);
    }
}
