//! Wall-clock helpers shared by host and league modules.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Days-to-civil conversion (Howard Hinnant's algorithm); returns `(year, month)` in UTC.
pub fn civil_year_month(unix_secs: u64) -> (i64, i64) {
    let days = (unix_secs / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month)
}

pub fn current_year() -> i64 {
    civil_year_month(unix_timestamp_secs()).0
}

#[cfg(test)]
mod tests {
    use super::civil_year_month;

    #[test]
    fn civil_year_month_handles_year_boundaries() {
        // 2025-12-31T23:59:59Z
        assert_eq!(civil_year_month(1_767_225_599), (2025, 12));
        // 2026-01-01T00:00:00Z
        assert_eq!(civil_year_month(1_767_225_600), (2026, 1));
    }
}
