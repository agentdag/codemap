//! Minimal, dependency-free UTC timestamp formatting (RFC3339).
//!
//! Kept tiny on purpose: the only consumer is `root.analyzedAt`, so a full date
//! crate isn't warranted. The golden test normalizes this field, so real time
//! here never makes the test flaky.

use std::time::{SystemTime, UNIX_EPOCH};

/// Current UTC time as an RFC3339 string, e.g. `2026-06-12T14:03:09Z`.
pub fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    rfc3339_from_unix(secs)
}

/// Format Unix epoch seconds as RFC3339 UTC (`YYYY-MM-DDThh:mm:ssZ`).
fn rfc3339_from_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let tod = (secs % 86_400) as i64;
    let (hour, min, sec) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Convert days since 1970-01-01 to a `(year, month, day)` civil date.
/// Howard Hinnant's algorithm; valid across the proleptic Gregorian calendar.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if month <= 2 { year + 1 } else { year }, month as u32, day)
}

#[cfg(test)]
mod tests {
    use super::rfc3339_from_unix;

    #[test]
    fn formats_known_timestamps() {
        assert_eq!(rfc3339_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_from_unix(946_684_800), "2000-01-01T00:00:00Z");
        assert_eq!(rfc3339_from_unix(1_234_567_890), "2009-02-13T23:31:30Z");
        assert_eq!(rfc3339_from_unix(1_609_459_200), "2021-01-01T00:00:00Z");
    }
}
