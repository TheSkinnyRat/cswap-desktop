//! The plan badge. From cswap >= 0.30 every `list --json` row carries `plan` (label, tier,
//! organization type, subscription status), and that wins: it covers every account, not
//! only the live one, and says when a subscription lapsed ([`from_cswap`]). Older cswap
//! has none, so the label is still derived from Claude Code's `subscriptionType`
//! (+ `rateLimitTier` only for the Max multiplier and Team/Enterprise seats,
//! [`label_for`]); reading the credential file and `plans.json` lives in the driver.
//! OWNER: driver.

use crate::types::CswapPlan;

const PLANS: [(&str, &str); 5] =
    [("free", "Free"), ("pro", "Pro"), ("max", "Max"), ("team", "Team"), ("enterprise", "Enterprise")];

fn known(sub: &str) -> Option<&'static str> {
    PLANS.iter().find(|(k, _)| *k == sub).map(|(_, v)| *v)
}

/// `titled('some_new-plan')` = `Some New Plan`; `5x` stays lower case.
fn titled(v: &str) -> String {
    v.split(|c: char| c == '_' || c == '-' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let lower = w.to_ascii_lowercase();
            if lower.len() > 1 && lower.ends_with('x') && lower[..lower.len() - 1].bytes().all(|b| b.is_ascii_digit()) {
                lower
            } else {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The first plan name inside the tier (`/(free|pro|max|team|enterprise)/i`).
fn plan_in(tier: &str) -> Option<&'static str> {
    // Bytes, not str slices: an index inside a multi-byte character would panic.
    let t = tier.to_ascii_lowercase().into_bytes();
    (0..t.len()).find_map(|i| PLANS.iter().find(|(k, _)| t[i..].starts_with(k.as_bytes())).map(|(k, _)| *k))
}

/// The `N` of the first `N x` / `Nx` in the tier (`/(\d+)\s*x/i`).
fn multiplier(tier: &str) -> Option<&str> {
    let b = tier.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        // A maximal digit run is the only candidate: a shorter one is followed by a digit.
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let end = i;
        let mut j = i;
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if j < b.len() && b[j].eq_ignore_ascii_case(&b'x') {
            return Some(&tier[start..end]);
        }
    }
    None
}

fn tier_plan(tier: Option<&str>) -> Option<String> {
    let tier = tier?;
    let plan = plan_in(tier)?;
    let base = known(plan)?;
    if plan != "max" {
        return Some(base.to_owned());
    }
    Some(multiplier(tier).map_or_else(|| base.to_owned(), |m| format!("Max {m}x")))
}

/// The plan as a badge.
///
/// `subscriptionType` answers the question (free / pro / max / team). `rateLimitTier` is an
/// internal string: reading all of it as the label was generalised from one sample
/// (`default_claude_max_5x`) and badged a Pro account "Ai". So the tier is only read for
/// which plan it names and the multiplier. A Team seat running at Max 20x shows both
/// ("Team · Max 20x"); that half is inferred (no Team account seen), and degrades to the
/// subscription alone rather than guessing.
pub fn label_for(tier: Option<&str>, subscription: Option<&str>) -> Option<String> {
    let sub = subscription.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
    let from_tier = tier_plan(tier);
    let Some(sub) = sub else { return from_tier };
    let base = known(&sub).map_or_else(|| titled(&sub), str::to_owned);
    if sub == "max" {
        return Some(tier.and_then(multiplier).map_or(base, |m| format!("Max {m}x")));
    }
    match from_tier {
        Some(t) if t != base && !t.starts_with(&base) => Some(format!("{base} · {t}")),
        _ => Some(base),
    }
}

/// Where an account's subscription stands, as far as cswap can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Current,
    /// `subscriptionStatus: canceled` while not yet on the free tier: still usable.
    Canceled,
    /// cswap's `expired`: canceled and already back on the free tier.
    Lapsed,
}

/// The badge text and standing of cswap's `plan`, or `None` when it names nothing (the
/// caller then falls back to `plans.json`).
///
/// cswap's label of a Team/Enterprise seat is the organization's ("Team") whatever tier
/// the seat runs at, so the seat's quota is read out of the tier the same way as for the
/// credential file ("Team · Max 5x"). Personal plans already name their tier ("Max 5x").
pub fn from_cswap(p: &CswapPlan) -> Option<(String, Standing)> {
    let label =
        Some(p.label.trim()).filter(|l| !l.is_empty()).map(str::to_owned).or_else(|| tier_plan(Some(&p.tier)))?;
    let seat = matches!(p.organization_type.as_deref(), Some("claude_team" | "claude_enterprise"))
        .then(|| tier_plan(Some(&p.tier)))
        .flatten()
        .filter(|s| *s != label && !s.starts_with(&label));
    let label = seat.map_or_else(|| label.clone(), |s| format!("{label} · {s}"));
    let standing = if p.expired {
        Standing::Lapsed
    } else if p.subscription_status.as_deref().is_some_and(|s| s.eq_ignore_ascii_case("canceled")) {
        Standing::Canceled
    } else {
        Standing::Current
    };
    Some((label, standing))
}

#[cfg(test)]
mod tests {
    use super::{Standing, from_cswap, label_for};
    use crate::types::CswapPlan;

    fn cs(label: &str, tier: &str, org: Option<&str>, status: Option<&str>, expired: bool) -> CswapPlan {
        CswapPlan {
            label: label.into(),
            tier: tier.into(),
            organization_type: org.map(Into::into),
            subscription_status: status.map(Into::into),
            expired,
            ..Default::default()
        }
    }

    #[test]
    fn cswaps_label_is_the_badge_for_personal_plans() {
        let p = cs("Max 5x", "default_claude_max_5x", Some("claude_max"), Some("active"), false);
        assert_eq!(from_cswap(&p), Some(("Max 5x".into(), Standing::Current)));
        // a paying Pro sits on the default tier; cswap's label already says Pro
        let p = cs("Pro", "default_claude_ai", Some("claude_pro"), Some("active"), false);
        assert_eq!(from_cswap(&p), Some(("Pro".into(), Standing::Current)));
        // Ultra shares the Max 20x tier: the label, not the tier, names it
        let p = cs("Ultra", "default_claude_max_20x", Some("claude_max"), None, false);
        assert_eq!(from_cswap(&p), Some(("Ultra".into(), Standing::Current)));
    }

    #[test]
    fn a_seat_shows_its_tier_beside_the_organization() {
        let p = cs("Team", "default_claude_max_5x", Some("claude_team"), Some("active"), false);
        assert_eq!(from_cswap(&p).unwrap().0, "Team · Max 5x");
        let p = cs("Enterprise", "default_claude_max_20x", Some("claude_enterprise"), None, false);
        assert_eq!(from_cswap(&p).unwrap().0, "Enterprise · Max 20x");
        // an Enterprise seat on its own tier name: nothing to add, nothing guessed
        let p = cs("Enterprise", "default_claude_zero", Some("claude_enterprise"), None, false);
        assert_eq!(from_cswap(&p).unwrap().0, "Enterprise");
        // the tier is only read for seats: a personal plan's label is not doubled
        let p = cs("Max 20x", "default_claude_max_20x", Some("claude_max"), None, false);
        assert_eq!(from_cswap(&p).unwrap().0, "Max 20x");
    }

    #[test]
    fn canceled_and_lapsed_are_told_apart() {
        let p = cs("Max 20x", "default_claude_max_20x", Some("claude_max"), Some("canceled"), false);
        assert_eq!(from_cswap(&p).unwrap().1, Standing::Canceled);
        let p = cs("Free", "default_claude_ai", Some("claude_max"), Some("canceled"), true);
        assert_eq!(from_cswap(&p), Some(("Free".into(), Standing::Lapsed)));
        // `expired` is cswap's verdict: it wins whatever the status string says
        let p = cs("Free", "default_claude_ai", None, None, true);
        assert_eq!(from_cswap(&p).unwrap().1, Standing::Lapsed);
        let p = cs("Pro", "default_claude_ai", Some("claude_pro"), Some("past_due"), false);
        assert_eq!(from_cswap(&p).unwrap().1, Standing::Current);
    }

    #[test]
    fn a_blank_label_falls_back_to_the_tier_then_to_nothing() {
        assert_eq!(from_cswap(&cs(" ", "default_claude_max_5x", None, None, false)).unwrap().0, "Max 5x");
        assert_eq!(from_cswap(&cs("", "default_claude_ai", None, None, false)), None);
        assert_eq!(from_cswap(&CswapPlan::default()), None);
    }

    fn l(tier: Option<&str>, sub: Option<&str>) -> Option<String> {
        label_for(tier, sub)
    }
    fn s(v: &str) -> Option<String> {
        Some(v.to_owned())
    }

    #[test]
    fn names_the_plan_from_the_subscription_not_the_tier() {
        // a Pro account's tier is not "default_claude_pro"; reading it as the label put "Ai"
        // on the badge
        assert_eq!(l(Some("default_claude_ai"), Some("pro")), s("Pro"));
        assert_eq!(l(Some("some_internal_name"), Some("pro")), s("Pro"));
        assert_eq!(l(None, Some("pro")), s("Pro"));
        assert_eq!(l(Some("anything"), Some("free")), s("Free"));
        assert_eq!(l(Some("anything"), Some("team")), s("Team"));
        assert_eq!(l(Some("anything"), Some("enterprise")), s("Enterprise"));
    }

    #[test]
    fn reads_the_multiplier_out_of_the_tier_but_only_for_max() {
        assert_eq!(l(Some("default_claude_max_5x"), Some("max")), s("Max 5x"));
        assert_eq!(l(Some("default_claude_max_20x"), Some("max")), s("Max 20x"));
        assert_eq!(l(Some("default_claude_max_50x"), Some("max")), s("Max 50x"));
        assert_eq!(l(Some("something_else"), Some("max")), s("Max"));
        assert_eq!(l(Some("default_claude_ai_5x"), Some("pro")), s("Pro"));
        assert_eq!(l(Some("tier 12 x"), Some("max")), s("Max 12x"));
    }

    #[test]
    fn falls_back_to_the_tier_only_when_it_names_a_plan() {
        assert_eq!(l(Some("default_claude_max_20x"), None), s("Max 20x"));
        assert_eq!(l(Some("default_claude_pro"), None), s("Pro"));
        assert_eq!(l(Some("default_claude_ai"), None), None);
        assert_eq!(l(None, None), None);
        assert_eq!(l(Some("default_claude_pro"), Some("  ")), s("Pro"));
    }

    #[test]
    fn shows_both_when_the_seat_runs_at_a_different_tier() {
        assert_eq!(l(Some("default_claude_max_20x"), Some("team")), s("Team · Max 20x"));
        assert_eq!(l(Some("default_claude_max_5x"), Some("team")), s("Team · Max 5x"));
        assert_eq!(l(Some("default_claude_pro"), Some("team")), s("Team · Pro"));
        assert_eq!(l(Some("default_claude_max_20x"), Some("enterprise")), s("Enterprise · Max 20x"));
        assert_eq!(l(Some("default_claude_ai"), Some("team")), s("Team"));
        assert_eq!(l(Some("default_claude_pro"), Some("pro")), s("Pro"));
        assert_eq!(l(Some("default_claude_team"), Some("team")), s("Team"));
    }

    #[test]
    fn titles_an_unfamiliar_subscription_rather_than_dropping_it() {
        assert_eq!(l(None, Some("startup")), s("Startup"));
        assert_eq!(l(None, Some("MAX")), s("Max"));
        assert_eq!(l(None, Some("big_corp-plan 3x")), s("Big Corp Plan 3x"));
    }

    #[test]
    fn a_non_ascii_tier_is_read_not_a_panic() {
        assert_eq!(l(Some("défault_ünïcode_max_5x"), None), s("Max 5x"));
        assert_eq!(l(Some("日本語"), Some("max")), s("Max"));
    }
}
