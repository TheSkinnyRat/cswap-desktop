//! ISO-8601 UTC timestamps (`2026-11-27T17:06:49.123Z`, what JavaScript's `toISOString`
//! wrote in 0.1.x), without a date crate: the app only ever prints "now".

use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn now_iso() -> String {
    iso(SystemTime::now())
}

pub fn iso(t: SystemTime) -> String {
    let d = t.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let secs = d.as_secs();
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, day) = civil_from_days(days as i64);
    format!(
        "{y:04}-{m:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60,
        d.subsec_millis()
    )
}

// Howard Hinnant's days-to-civil algorithm (proleptic Gregorian).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_to_iso_string() {
        assert_eq!(iso(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        // 2024-02-29T23:59:59.250Z: a leap day, the end of a day, millis
        let t = UNIX_EPOCH + Duration::from_millis(1_709_251_199_250);
        assert_eq!(iso(t), "2024-02-29T23:59:59.250Z");
        let t = UNIX_EPOCH + Duration::from_secs(1_795_799_209);
        assert_eq!(iso(t), "2026-11-27T17:06:49.000Z");
    }
}
