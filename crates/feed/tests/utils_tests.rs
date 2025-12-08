// ABOUTME: Integration tests for feed utility modules.
// ABOUTME: Tests time/duration parsing, HTML utilities, and image extraction.

use digests_feed::{
    decode_entities, extract_first_image, is_valid_image_url, parse_duration_seconds,
    parse_flexible_time, resolve_image_url, strip_html,
};

mod time_parse_tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_parse_rfc3339() {
        let result = parse_flexible_time("2023-06-15T14:30:00Z");
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt, Utc.with_ymd_and_hms(2023, 6, 15, 14, 30, 0).unwrap());
    }

    #[test]
    fn test_parse_rfc2822_with_timezone() {
        let result = parse_flexible_time("Mon, 02 Jan 2006 15:04:05 -0700");
        assert!(result.is_some());
        let dt = result.unwrap();
        // -0700 means 15:04:05 local = 22:04:05 UTC
        assert_eq!(dt, Utc.with_ymd_and_hms(2006, 1, 2, 22, 4, 5).unwrap());
    }

    #[test]
    fn test_parse_naive_datetime_assumes_utc() {
        let result = parse_flexible_time("2006-01-02 15:04:05");
        assert!(result.is_some());
        let dt = result.unwrap();
        // Without timezone, assumed UTC
        assert_eq!(dt.timezone(), Utc);
        assert_eq!(dt, Utc.with_ymd_and_hms(2006, 1, 2, 15, 4, 5).unwrap());
    }

    #[test]
    fn test_parse_date_only() {
        let result = parse_flexible_time("2023-12-25");
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt, Utc.with_ymd_and_hms(2023, 12, 25, 0, 0, 0).unwrap());
    }
}

mod duration_parse_tests {
    use super::*;

    #[test]
    fn test_plain_integer_seconds() {
        assert_eq!(parse_duration_seconds("123"), Some(123));
        assert_eq!(parse_duration_seconds("0"), Some(0));
        assert_eq!(parse_duration_seconds("3600"), Some(3600));
    }

    #[test]
    fn test_hhmmss_format() {
        // 01:02:03 = 1*3600 + 2*60 + 3 = 3723
        assert_eq!(parse_duration_seconds("01:02:03"), Some(3723));
        assert_eq!(parse_duration_seconds("1:2:3"), Some(3723));
        assert_eq!(parse_duration_seconds("00:00:00"), Some(0));
    }

    #[test]
    fn test_mmss_format() {
        // 05:30 = 5*60 + 30 = 330
        assert_eq!(parse_duration_seconds("05:30"), Some(330));
        assert_eq!(parse_duration_seconds("5:30"), Some(330));
        assert_eq!(parse_duration_seconds("00:30"), Some(30));
    }

    #[test]
    fn test_go_style_duration() {
        // 1h30m = 90 minutes = 5400 seconds
        assert_eq!(parse_duration_seconds("1h30m"), Some(5400));
        assert_eq!(parse_duration_seconds("45m"), Some(2700));
        assert_eq!(parse_duration_seconds("2h"), Some(7200));
        assert_eq!(parse_duration_seconds("30s"), Some(30));
    }

    #[test]
    fn test_empty_and_invalid() {
        assert_eq!(parse_duration_seconds(""), None);
        assert_eq!(parse_duration_seconds("   "), None);
        assert_eq!(parse_duration_seconds("not a duration"), None);
    }
}

mod html_utils_tests {
    use super::*;

    #[test]
    fn test_strip_html_removes_tags() {
        assert_eq!(strip_html("<p>Hello World</p>"), "Hello World");
        assert_eq!(
            strip_html("<div><b>Bold</b> and <i>italic</i></div>"),
            "Bold and italic"
        );
    }

    #[test]
    fn test_strip_html_decodes_entities() {
        assert_eq!(strip_html("<p>Tom &amp; Jerry</p>"), "Tom & Jerry");
        assert_eq!(strip_html("&lt;script&gt;alert&lt;/script&gt;"), "<script>alert</script>");
    }

    #[test]
    fn test_strip_html_collapses_whitespace() {
        assert_eq!(strip_html("<p>Hello</p>  <p>World</p>"), "Hello World");
        assert_eq!(strip_html("Multiple    spaces"), "Multiple spaces");
        assert_eq!(strip_html("Line\n\n\nbreaks"), "Line breaks");
    }

    #[test]
    fn test_decode_entities_nbsp() {
        assert_eq!(decode_entities("Hello&nbsp;World"), "Hello World");
    }

    #[test]
    fn test_decode_entities_common() {
        assert_eq!(decode_entities("&amp;"), "&");
        assert_eq!(decode_entities("&lt;"), "<");
        assert_eq!(decode_entities("&gt;"), ">");
        assert_eq!(decode_entities("&quot;"), "\"");
        assert_eq!(decode_entities("&mdash;"), "—");
    }
}

mod image_utils_tests {
    use super::*;

    #[test]
    fn test_is_valid_image_url_accepts_normal() {
        assert!(is_valid_image_url("https://example.com/photo.jpg"));
        assert!(is_valid_image_url("https://cdn.example.com/image.png"));
        assert!(is_valid_image_url("https://example.com/uploads/banner.webp"));
    }

    #[test]
    fn test_is_valid_image_url_rejects_tracking_pixel() {
        assert!(!is_valid_image_url("https://example.com/tracking/pixel.gif"));
        assert!(!is_valid_image_url("https://analytics.example.com/img.png"));
        assert!(!is_valid_image_url("https://example.com/beacon.gif"));
        assert!(!is_valid_image_url("https://example.com/spacer.gif"));
        assert!(!is_valid_image_url("https://example.com/1x1.gif"));
    }

    #[test]
    fn test_resolve_image_url_absolute_unchanged() {
        let result = resolve_image_url("https://example.com/image.jpg", None);
        assert_eq!(result, Some("https://example.com/image.jpg".to_string()));
    }

    #[test]
    fn test_resolve_image_url_relative_with_base() {
        let result = resolve_image_url("/images/photo.jpg", Some("https://example.com/article/123"));
        assert_eq!(result, Some("https://example.com/images/photo.jpg".to_string()));
    }

    #[test]
    fn test_resolve_image_url_relative_path() {
        let result = resolve_image_url("../images/photo.jpg", Some("https://example.com/articles/tech/post.html"));
        assert_eq!(result, Some("https://example.com/articles/images/photo.jpg".to_string()));
    }

    #[test]
    fn test_extract_first_image_basic() {
        let html = r#"<article><p>Text</p><img src="https://example.com/image.jpg" alt="Photo"></article>"#;
        let result = extract_first_image(html, None);
        assert_eq!(result, Some("https://example.com/image.jpg".to_string()));
    }

    #[test]
    fn test_extract_first_image_skips_invalid() {
        let html = r#"
            <img src="https://example.com/pixel.gif">
            <img src="https://tracking.example.com/beacon.png">
            <img src="https://example.com/real-image.jpg">
        "#;
        let result = extract_first_image(html, None);
        assert_eq!(result, Some("https://example.com/real-image.jpg".to_string()));
    }

    #[test]
    fn test_extract_first_image_resolves_relative() {
        let html = r#"<img src="/uploads/photo.jpg">"#;
        let result = extract_first_image(html, Some("https://blog.example.com/posts/1"));
        assert_eq!(result, Some("https://blog.example.com/uploads/photo.jpg".to_string()));
    }

    #[test]
    fn test_extract_first_image_none_when_no_images() {
        let html = "<p>No images in this content</p>";
        let result = extract_first_image(html, None);
        assert_eq!(result, None);
    }
}
