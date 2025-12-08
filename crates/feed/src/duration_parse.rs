// ABOUTME: Duration string parsing for podcast episode lengths.
// ABOUTME: Supports integer seconds, HH:MM:SS, MM:SS, and Go-style duration strings.

/// Parses a duration string into seconds.
/// Supports:
/// - Plain integers (seconds)
/// - HH:MM:SS format
/// - MM:SS format
/// - Go-style durations like "1h30m", "45m", "2h"
/// Returns None if parsing fails or value doesn't fit in u32.
pub fn parse_duration_seconds(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Try plain integer first
    if let Ok(secs) = s.parse::<u64>() {
        return u32::try_from(secs).ok();
    }

    // Try HH:MM:SS or MM:SS
    if s.contains(':') {
        return parse_colon_format(s);
    }

    // Try Go-style duration (1h30m, 45m, 2h, etc.)
    if let Ok(duration) = parse_duration::parse(s) {
        let secs = duration.as_secs();
        return u32::try_from(secs).ok();
    }

    None
}

fn parse_colon_format(s: &str) -> Option<u32> {
    let parts: Vec<&str> = s.split(':').collect();

    match parts.len() {
        2 => {
            // MM:SS
            let mins: u64 = parts[0].parse().ok()?;
            let secs: u64 = parts[1].parse().ok()?;
            let total = mins * 60 + secs;
            u32::try_from(total).ok()
        }
        3 => {
            // HH:MM:SS
            let hours: u64 = parts[0].parse().ok()?;
            let mins: u64 = parts[1].parse().ok()?;
            let secs: u64 = parts[2].parse().ok()?;
            let total = hours * 3600 + mins * 60 + secs;
            u32::try_from(total).ok()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_integer() {
        assert_eq!(parse_duration_seconds("123"), Some(123));
        assert_eq!(parse_duration_seconds("0"), Some(0));
    }

    #[test]
    fn test_hhmmss() {
        assert_eq!(parse_duration_seconds("01:02:03"), Some(3723));
        assert_eq!(parse_duration_seconds("0:0:0"), Some(0));
    }

    #[test]
    fn test_mmss() {
        assert_eq!(parse_duration_seconds("05:30"), Some(330));
        assert_eq!(parse_duration_seconds("0:30"), Some(30));
    }

    #[test]
    fn test_go_duration() {
        assert_eq!(parse_duration_seconds("1h30m"), Some(5400));
        assert_eq!(parse_duration_seconds("45m"), Some(2700));
        assert_eq!(parse_duration_seconds("2h"), Some(7200));
    }

    #[test]
    fn test_empty_returns_none() {
        assert!(parse_duration_seconds("").is_none());
        assert!(parse_duration_seconds("   ").is_none());
    }

    #[test]
    fn test_invalid_returns_none() {
        assert!(parse_duration_seconds("not a duration").is_none());
    }
}
