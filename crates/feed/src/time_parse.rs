// ABOUTME: Flexible time parsing for RSS/Atom feed dates.
// ABOUTME: Tries multiple date formats matching Go's ParseFlexibleTime behavior.

use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};

/// Parses a datetime string using multiple common RSS/Atom formats.
/// Returns UTC datetime if successful, None if no format matches.
///
/// Covers Go time formats: RFC3339, RFC3339Nano, RFC1123, RFC1123Z, RFC822, RFC822Z,
/// and common variants with single-digit days and named timezones.
pub fn parse_flexible_time(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Try RFC3339 first (most common for Atom) - handles RFC3339Nano too
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // Try RFC2822 (common RSS format) - handles RFC1123Z, RFC822Z
    if let Ok(dt) = DateTime::parse_from_rfc2822(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // Try named timezone parsing (MST, PST, EST, etc.)
    if let Some(dt) = parse_with_named_timezone(s) {
        return Some(dt);
    }

    // Formats with numeric timezone offset
    let formats_with_tz = [
        // RFC1123Z with numeric offset: "Mon, 02 Jan 2006 15:04:05 -0700"
        "%a, %d %b %Y %H:%M:%S %z",
        // Single digit day: "Mon, 2 Jan 2006 15:04:05 -0700"
        "%a, %e %b %Y %H:%M:%S %z",
        // Without weekday: "02 Jan 2006 15:04:05 -0700"
        "%d %b %Y %H:%M:%S %z",
        // Single digit day without weekday: "2 Jan 2006 15:04:05 -0700"
        "%e %b %Y %H:%M:%S %z",
        // ISO-like with offset: "2006-01-02T15:04:05-07:00"
        "%Y-%m-%dT%H:%M:%S%:z",
        // ISO-like compact offset: "2006-01-02T15:04:05-0700"
        "%Y-%m-%dT%H:%M:%S%z",
    ];

    for fmt in &formats_with_tz {
        if let Ok(dt) = DateTime::parse_from_str(s, fmt) {
            return Some(dt.with_timezone(&Utc));
        }
    }

    // Formats without timezone (assume UTC)
    let formats_naive = [
        // ISO datetime: "2006-01-02T15:04:05"
        "%Y-%m-%dT%H:%M:%S",
        // Space-separated: "2006-01-02 15:04:05"
        "%Y-%m-%d %H:%M:%S",
        // Without weekday: "02 Jan 2006 15:04:05"
        "%d %b %Y %H:%M:%S",
        // Single digit day: "2 Jan 2006 15:04:05"
        "%e %b %Y %H:%M:%S",
        // Date only: "02 Jan 2006"
        "%d %b %Y",
    ];

    for fmt in &formats_naive {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(Utc.from_utc_datetime(&naive));
        }
    }

    // Try date-only format: "2006-01-02"
    if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let naive_dt = naive_date.and_hms_opt(0, 0, 0)?;
        return Some(Utc.from_utc_datetime(&naive_dt));
    }

    None
}

/// Parses datetime strings with named timezones (MST, PST, EST, etc.).
/// chrono's %Z doesn't parse these reliably, so we handle them manually.
fn parse_with_named_timezone(s: &str) -> Option<DateTime<Utc>> {
    // Common named timezone offsets (in seconds from UTC)
    let tz_offsets: &[(&str, i32)] = &[
        ("GMT", 0),
        ("UTC", 0),
        ("EST", -5 * 3600),
        ("EDT", -4 * 3600),
        ("CST", -6 * 3600),
        ("CDT", -5 * 3600),
        ("MST", -7 * 3600),
        ("MDT", -6 * 3600),
        ("PST", -8 * 3600),
        ("PDT", -7 * 3600),
        ("AKST", -9 * 3600),
        ("AKDT", -8 * 3600),
        ("HST", -10 * 3600),
        ("HAST", -10 * 3600),
        ("HADT", -9 * 3600),
        ("AST", -4 * 3600),
        ("ADT", -3 * 3600),
        ("NST", -(3 * 3600 + 30 * 60)),
        ("NDT", -(2 * 3600 + 30 * 60)),
        // European
        ("WET", 0),
        ("WEST", 1 * 3600),
        ("CET", 1 * 3600),
        ("CEST", 2 * 3600),
        ("EET", 2 * 3600),
        ("EEST", 3 * 3600),
        ("BST", 1 * 3600), // British Summer Time
        ("IST", 1 * 3600), // Irish Standard Time (summer)
        // Asia/Pacific
        ("JST", 9 * 3600),
        ("KST", 9 * 3600),
        ("CST", 8 * 3600), // China Standard Time (note: conflicts with US Central)
        ("IST", 5 * 3600 + 30 * 60), // India Standard Time
        ("AEST", 10 * 3600),
        ("AEDT", 11 * 3600),
        ("AWST", 8 * 3600),
        ("NZST", 12 * 3600),
        ("NZDT", 13 * 3600),
    ];

    // Try to find and replace timezone abbreviation with offset
    for (tz_name, offset_secs) in tz_offsets {
        if s.ends_with(tz_name) {
            let base = s.trim_end_matches(tz_name).trim_end();

            // Try parsing the base datetime
            let formats = [
                "%a, %d %b %Y %H:%M:%S",
                "%a, %e %b %Y %H:%M:%S",
                "%d %b %Y %H:%M:%S",
                "%e %b %Y %H:%M:%S",
            ];

            for fmt in &formats {
                if let Ok(naive) = NaiveDateTime::parse_from_str(base, fmt) {
                    let offset = FixedOffset::east_opt(*offset_secs)?;
                    let dt = offset.from_local_datetime(&naive).single()?;
                    return Some(dt.with_timezone(&Utc));
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_rfc3339() {
        let result = parse_flexible_time("2023-06-15T14:30:00Z");
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2023);
        assert_eq!(dt.month(), 6);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_rfc2822() {
        let result = parse_flexible_time("Mon, 02 Jan 2006 15:04:05 -0700");
        assert!(result.is_some());
    }

    #[test]
    fn test_naive_datetime_assumes_utc() {
        let result = parse_flexible_time("2006-01-02 15:04:05");
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.timezone(), Utc);
    }

    #[test]
    fn test_empty_returns_none() {
        assert!(parse_flexible_time("").is_none());
        assert!(parse_flexible_time("   ").is_none());
    }

    #[test]
    fn test_invalid_returns_none() {
        assert!(parse_flexible_time("not a date").is_none());
    }

    #[test]
    fn test_named_timezone_mst() {
        // "Mon, 02 Jan 2006 15:04:05 MST" - MST is UTC-7
        let result = parse_flexible_time("Mon, 02 Jan 2006 15:04:05 MST");
        assert!(result.is_some());
        let dt = result.unwrap();
        // 15:04:05 MST = 22:04:05 UTC
        assert_eq!(dt.year(), 2006);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 2);
    }

    #[test]
    fn test_single_digit_day_with_mst() {
        // "Mon, 2 Jan 2006 15:04:05 MST" - single digit day variant
        let result = parse_flexible_time("Mon, 2 Jan 2006 15:04:05 MST");
        assert!(result.is_some());
    }

    #[test]
    fn test_without_weekday_offset() {
        // "02 Jan 2006 15:04:05 -0700"
        let result = parse_flexible_time("02 Jan 2006 15:04:05 -0700");
        assert!(result.is_some());
    }
}
