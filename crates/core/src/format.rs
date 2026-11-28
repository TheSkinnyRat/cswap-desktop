//! Display formatting shared by the pages: countdowns, ages, percentages, pace, masked
//! emails. Port of `src/renderer/src/lib/format.ts`. OWNER: ui-accounts.
//!
//! Time is passed in as Unix milliseconds (`now_ms`) instead of read here, so the core
//! stays pure and a test (or a README screenshot) renders the same text every run.
//! Local-time output takes the UTC offset as an argument for the same reason.

use crate::types::{Account, UsageWindow};

/// Wall clock in Unix milliseconds. The one impure helper here; callers that need
/// stable output pass their own `now_ms` to everything else.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

// Howard Hinnant's days-from-civil and back: exact for the proleptic Gregorian calendar,
// and small enough that the core needs no date crate.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `2026-11-28T00:06:51.000Z`, the shape JavaScript's `toISOString` wrote in 0.1.x.
pub fn iso_from_ms(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let milli = ms.rem_euclid(1000);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let sod = secs.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{milli:03}Z", sod / 3600, sod % 3600 / 60, sod % 60)
}

/// Parses the ISO-8601 stamps cswap (Python `isoformat`, `+00:00`, microseconds) and the
/// app (`Z`, milliseconds) write. A naive stamp is read as UTC. `None` = unparseable,
/// which every caller shows as an empty string, as `new Date(x)` → NaN did in 0.1.x.
pub fn parse_iso_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    let b = s.as_bytes();
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let t = s.get(r)?;
        if t.is_empty() || !t.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        t.parse().ok()
    };
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let (mut h, mut mi, mut se, mut frac_ms, mut off) = (0, 0, 0, 0i64, 0i64);
    if b.len() > 10 {
        if b[10] != b'T' && b[10] != b' ' {
            return None;
        }
        h = num(11..13)?;
        if b.get(13) != Some(&b':') {
            return None;
        }
        mi = num(14..16)?;
        let mut i = 16;
        if b.get(16) == Some(&b':') {
            se = num(17..19)?;
            i = 19;
        }
        if b.get(i) == Some(&b'.') {
            let start = i + 1;
            let mut end = start;
            while end < b.len() && b[end].is_ascii_digit() {
                end += 1;
            }
            if end == start {
                return None;
            }
            let digits = &s[start..end.min(start + 3)];
            frac_ms = digits.parse::<i64>().ok()? * 10i64.pow(3 - digits.len() as u32);
            i = end;
        }
        match b.get(i) {
            None => {}
            Some(b'Z') | Some(b'z') if i + 1 == b.len() => {}
            Some(&c @ (b'+' | b'-')) => {
                let oh = num(i + 1..i + 3)?;
                let om = match b.get(i + 3) {
                    Some(b':') => num(i + 4..i + 6)?,
                    _ => num(i + 3..i + 5)?,
                };
                off = (oh * 60 + om) * 60 * if c == b'-' { -1 } else { 1 };
            }
            _ => return None,
        }
    }
    if h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let days = days_from_civil(y, mo, d);
    Some(((days * 86_400 + h * 3600 + mi * 60 + se) - off) * 1000 + frac_ms)
}

/// `45%`; nothing measured → an em dash, as 0.1.23.
pub fn pct(n: Option<f64>) -> String {
    match n {
        Some(n) if !n.is_nan() => format!("{}%", js_round(n)),
        _ => "—".into(),
    }
}

// JavaScript's Math.round: halves go up (towards +∞), unlike Rust's round().
fn js_round(n: f64) -> i64 {
    (n + 0.5).floor() as i64
}

fn duration_text(abs: i64) -> String {
    if abs < 60 {
        format!("{abs}s")
    } else if abs < 3600 {
        format!("{}m", abs / 60)
    } else if abs < 86_400 {
        format!("{}h {}m", abs / 3600, abs % 3600 / 60)
    } else {
        format!("{}d {}h", abs / 86_400, abs % 86_400 / 3600)
    }
}

/// `in 3h 30m` / `5m ago`.
pub fn rel_time(iso: Option<&str>, now_ms: i64) -> String {
    let Some(t) = iso.and_then(parse_iso_ms) else { return String::new() };
    let diff = js_round((t - now_ms) as f64 / 1000.0);
    let s = duration_text(diff.abs());
    if diff >= 0 { format!("in {s}") } else { format!("{s} ago") }
}

/// cswap's `usageAgeSeconds` as `just now` / `5m ago` / `2h ago` / `3d ago`.
pub fn age_seconds(sec: Option<f64>) -> String {
    let Some(sec) = sec else { return String::new() };
    if sec < 60.0 {
        "just now".into()
    } else if sec < 3600.0 {
        format!("{}m ago", (sec / 60.0).floor())
    } else if sec < 86_400.0 {
        format!("{}h ago", (sec / 3600.0).floor())
    } else {
        format!("{}d ago", (sec / 86_400.0).floor())
    }
}

/// `3h 30m` for a future reset. A reset already in the past (stale last-good data) says
/// so instead of printing "41d ago left". Just the duration: the column already says it
/// is a window, and the tooltip spells out "resets <clock>".
pub fn reset_text(iso: Option<&str>, now_ms: i64) -> String {
    let Some(t) = iso.and_then(parse_iso_ms) else { return String::new() };
    if t <= now_ms {
        return "reset passed".into();
    }
    let r = rel_time(iso, now_ms);
    r.strip_prefix("in ").map(str::to_owned).unwrap_or(r)
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Local wall-clock time of a stamp: `14:30` when it is today, `Nov 29 14:30` otherwise.
/// `offset_secs` is the local UTC offset (the UI reads it from the OS). 0.1.23 used the
/// browser locale's format; this is fixed 24-hour.
pub fn clock_of(iso: Option<&str>, now_ms: i64, offset_secs: i64) -> String {
    let Some(t) = iso.and_then(parse_iso_ms) else { return String::new() };
    let local = t.div_euclid(1000) + offset_secs;
    let today = (now_ms.div_euclid(1000) + offset_secs).div_euclid(86_400);
    let day = local.div_euclid(86_400);
    let sod = local.rem_euclid(86_400);
    let hm = format!("{:02}:{:02}", sod / 3600, sod % 3600 / 60);
    if day == today {
        hm
    } else {
        let (_, m, d) = civil_from_days(day);
        format!("{} {d} {hm}", MONTHS[(m - 1) as usize])
    }
}

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// Like [`clock_of`], but a stamp within the coming week reads `Tue 14:00`: the hero's
/// forecast talks about this week, where a weekday says more than a date.
pub fn weekday_clock(iso: Option<&str>, now_ms: i64, offset_secs: i64) -> String {
    let Some(t) = iso.and_then(parse_iso_ms) else { return String::new() };
    let local = t.div_euclid(1000) + offset_secs;
    let today = (now_ms.div_euclid(1000) + offset_secs).div_euclid(86_400);
    let day = local.div_euclid(86_400);
    if day <= today || day - today >= 7 {
        return clock_of(iso, now_ms, offset_secs);
    }
    let sod = local.rem_euclid(86_400);
    // 1970-01-01 was a Thursday (index 3).
    let wd = WEEKDAYS[(day + 3).rem_euclid(7) as usize];
    format!("{wd} {:02}:{:02}", sod / 3600, sod % 3600 / 60)
}

/// `14:30:05`, local 24-hour time of a stamp (the Auto feed's first column).
pub fn time_of(iso: &str, offset_secs: i64) -> String {
    let Some(t) = parse_iso_ms(iso) else { return String::new() };
    let sod = (t.div_euclid(1000) + offset_secs).rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}", sod / 3600, sod % 3600 / 60, sod % 60)
}

/// Label of cswap's `usageStatus`; unknown statuses (upstream adds them) show raw.
pub fn status_label(s: &str) -> String {
    match s {
        "ok" => "OK",
        "token_expired" => "Token expired",
        "api_key" => "API key",
        "keychain_unavailable" => "Keychain locked",
        "relogin_required" => "Re-login required",
        "foreign_credential" => "Foreign credential",
        "no_credentials" => "No credentials",
        "unavailable" => "Usage unavailable",
        other => other,
    }
    .into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Warn,
    Danger,
    Muted,
}

pub fn status_tone(s: &str) -> Tone {
    match s {
        "ok" => Tone::Ok,
        "api_key" | "unavailable" => Tone::Muted,
        "token_expired" | "relogin_required" | "no_credentials" => Tone::Danger,
        _ => Tone::Warn,
    }
}

/// Bar colour of a usage percentage: 100 = exhausted, 80 = getting there.
pub fn tone(p: Option<f64>) -> Tone {
    match p {
        None => Tone::Muted,
        Some(p) if p >= 100.0 => Tone::Danger,
        Some(p) if p >= 80.0 => Tone::Warn,
        Some(_) => Tone::Ok,
    }
}

pub fn display_name(a: &Account) -> &str {
    a.alias.as_deref().filter(|s| !s.is_empty()).unwrap_or(&a.email)
}

fn bullets(len: usize) -> String {
    "•".repeat(len.saturating_sub(3).clamp(3, 8))
}

/// `personal` or the organisation's name (masked like an email when masking is on).
pub fn org_tag(a: &Account, mask: bool) -> String {
    if !a.is_organization {
        return "personal".into();
    }
    let name = &a.organization_name;
    // Anthropic names a personal workspace "<email>'s Organization" — that is not a team.
    if name.is_empty() || name.ends_with("'s Organization") {
        return "personal".into();
    }
    // An organisation name identifies the company as plainly as the address does.
    if mask {
        let head: String = name.chars().take(3).collect();
        format!("{head}{}", bullets(name.chars().count()))
    } else {
        name.clone()
    }
}

/// Which window binds (highest utilization), the same idea cswap calls "binding".
pub fn binding(a: &Account) -> Option<(String, &UsageWindow)> {
    let u = a.usage.as_ref().or(a.last_good_usage.as_ref())?;
    let mut all: Vec<(String, &UsageWindow)> = Vec::new();
    if let Some(w) = &u.five_hour {
        all.push(("5h".into(), w));
    }
    if let Some(w) = &u.seven_day {
        all.push(("7d".into(), w));
    }
    for s in u.scoped.iter().flatten() {
        all.push((s.name.clone(), &s.window));
    }
    // reduce() keeps the first of equal maxima.
    let mut best: Option<(String, &UsageWindow)> = None;
    for x in all {
        if best.as_ref().is_none_or(|b| x.1.pct > b.1.pct) {
            best = Some(x);
        }
    }
    best
}

/// Privacy mask for screenshots and streams: keep the first three characters only (of
/// the local part), and hide the length of the rest.
pub fn mask_email(email: &str, on: bool) -> String {
    if !on || email.is_empty() {
        return email.into();
    }
    let local = email.split('@').next().unwrap_or(email);
    let head: String = local.chars().take(3).collect();
    format!("{head}{}", bullets(email.chars().count()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_795_800_000_000; // 2026-11-27T17:20:00Z (00:20 on the 28th in UTC+7)

    #[test]
    fn iso_round_trips_and_reads_python_stamps() {
        assert_eq!(iso_from_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_from_ms(NOW), "2026-11-27T17:20:00.000Z");
        assert_eq!(parse_iso_ms(&iso_from_ms(NOW + 123)), Some(NOW + 123));
        // cswap writes Python's isoformat: offset and microseconds.
        assert_eq!(parse_iso_ms("2026-11-28T00:20:00.123456+07:00"), Some(NOW + 123));
        assert_eq!(parse_iso_ms("2026-11-27T17:20:00"), Some(NOW));
        assert_eq!(parse_iso_ms("2024-02-29T00:00:00Z"), Some(1_709_164_800_000));
        assert_eq!(parse_iso_ms("not a date"), None);
        assert_eq!(parse_iso_ms("2026-13-01T00:00:00Z"), None);
        assert_eq!(parse_iso_ms(""), None);
    }

    #[test]
    fn pct_rounds_like_javascript() {
        assert_eq!(pct(Some(44.5)), "45%");
        assert_eq!(pct(Some(12.4)), "12%");
        assert_eq!(pct(None), "—");
        assert_eq!(pct(Some(f64::NAN)), "—");
    }

    #[test]
    fn rel_time_and_reset_text() {
        let at = |s: i64| iso_from_ms(NOW + s * 1000);
        assert_eq!(rel_time(Some(&at(12_600)), NOW), "in 3h 30m");
        assert_eq!(rel_time(Some(&at(-300)), NOW), "5m ago");
        assert_eq!(rel_time(Some(&at(30)), NOW), "in 30s");
        assert_eq!(rel_time(Some(&at(180_000)), NOW), "in 2d 2h");
        assert_eq!(rel_time(Some("garbage"), NOW), "");
        assert_eq!(rel_time(None, NOW), "");
        assert_eq!(reset_text(Some(&at(12_600)), NOW), "3h 30m");
        // A reset in the past (stale last-good data) must not read "41d ago left".
        assert_eq!(reset_text(Some(&at(-41 * 86_400)), NOW), "reset passed");
        assert_eq!(reset_text(Some(&at(0)), NOW), "reset passed");
    }

    #[test]
    fn ages() {
        assert_eq!(age_seconds(Some(40.0)), "just now");
        assert_eq!(age_seconds(Some(300.0)), "5m ago");
        assert_eq!(age_seconds(Some(7200.0)), "2h ago");
        assert_eq!(age_seconds(Some(3.0 * 86_400.0)), "3d ago");
        assert_eq!(age_seconds(None), "");
    }

    #[test]
    fn clock_of_is_local_and_drops_the_date_today() {
        let wib = 7 * 3600;
        assert_eq!(clock_of(Some(&iso_from_ms(NOW + 3_600_000)), NOW, wib), "01:20");
        // 13:20Z: still the 28th in UTC+7, so no date.
        assert_eq!(clock_of(Some(&iso_from_ms(NOW + 20 * 3_600_000)), NOW, wib), "20:20");
        assert_eq!(clock_of(Some(&iso_from_ms(NOW + 24 * 3_600_000)), NOW, wib), "Nov 29 00:20");
        // In UTC the same instant +1h is still the 27th: the offset decides "today".
        assert_eq!(clock_of(Some(&iso_from_ms(NOW + 7 * 3_600_000)), NOW, 0), "Nov 28 00:20");
        assert_eq!(clock_of(Some("x"), NOW, 0), "");
        assert_eq!(time_of(&iso_from_ms(NOW + 5_000), wib), "00:20:05");
    }

    #[test]
    fn weekday_clock_names_the_day_within_a_week() {
        let wib = 7 * 3600;
        let at = |h: i64| iso_from_ms(NOW + h * 3_600_000);
        // NOW is Saturday 28 Nov 00:20 in UTC+7
        assert_eq!(weekday_clock(Some(&at(1)), NOW, wib), "01:20", "today: the time alone");
        assert_eq!(weekday_clock(Some(&at(24)), NOW, wib), "Sun 00:20");
        assert_eq!(weekday_clock(Some(&at(3 * 24 + 14)), NOW, wib), "Tue 14:20");
        assert_eq!(weekday_clock(Some(&at(7 * 24)), NOW, wib), "Dec 5 00:20", "a week out: the date");
        assert_eq!(weekday_clock(Some("x"), NOW, 0), "");
    }

    #[test]
    fn org_tag_hides_personal_workspaces_and_masks_names() {
        let mut a = Account {
            is_organization: true,
            organization_name: "you@example.com's Organization".into(),
            ..Default::default()
        };
        assert_eq!(org_tag(&a, false), "personal");
        a.organization_name = "Company Inc".into();
        assert_eq!(org_tag(&a, false), "Company Inc");
        assert_eq!(org_tag(&a, true), "Com••••••••");
        a.is_organization = false;
        assert_eq!(org_tag(&a, false), "personal");
    }

    #[test]
    fn mask_email_keeps_three_characters() {
        assert_eq!(mask_email("you@example.com", true), "you••••••••");
        assert_eq!(mask_email("ab@x.io", true), "ab••••");
        assert_eq!(mask_email("you@example.com", false), "you@example.com");
        assert!(!mask_email("work@example.com", true).contains('@'));
    }

    #[test]
    fn tones() {
        assert_eq!(tone(Some(100.0)), Tone::Danger);
        assert_eq!(tone(Some(80.0)), Tone::Warn);
        assert_eq!(tone(Some(79.9)), Tone::Ok);
        assert_eq!(tone(None), Tone::Muted);
        assert_eq!(status_tone("token_expired"), Tone::Danger);
        assert_eq!(status_tone("api_key"), Tone::Muted);
        assert_eq!(status_tone("keychain_unavailable"), Tone::Warn);
        assert_eq!(status_tone("something_new"), Tone::Warn);
        assert_eq!(status_label("something_new"), "something_new");
    }

    #[test]
    fn binding_picks_the_fullest_window() {
        let w = |p| UsageWindow { pct: p, ..Default::default() };
        let a = Account {
            usage: Some(crate::types::Usage {
                five_hour: Some(w(45.0)),
                seven_day: Some(w(12.0)),
                scoped: Some(vec![crate::types::ScopedWindow { name: "Fable".into(), window: w(60.0) }]),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(binding(&a).map(|b| b.0), Some("Fable".into()));
        assert!(binding(&Account::default()).is_none());
    }
}
