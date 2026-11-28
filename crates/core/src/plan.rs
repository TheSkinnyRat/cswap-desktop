//! The plan badge: label from Claude Code's `subscriptionType` (+ `rateLimitTier` only for
//! the Max multiplier and Team/Enterprise seats). Pure part of `src/main/plan.ts`; reading
//! the credential file and `plans.json` lives in the driver. OWNER: driver.

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

#[cfg(test)]
mod tests {
    use super::label_for;

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
