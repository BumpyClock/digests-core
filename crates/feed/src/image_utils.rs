// ABOUTME: Image URL extraction and validation for feed content.
// ABOUTME: Extracts first valid image from HTML and filters tracking pixels.

use scraper::{Html, Selector};
use url::Url;

/// Patterns indicating tracking pixels or invalid images (case-insensitive check).
const INVALID_PATTERNS: &[&str] = &[
    "pixel",
    "tracking",
    "analytics",
    "beacon",
    "spacer",
    "clear.gif",
    "blank.gif",
    "1x1",
    "data:image/gif;base64,r0lgodlhaqabai",
];

/// Extracts the first valid image URL from HTML content.
/// Resolves relative URLs using the provided base URL if available.
pub fn extract_first_image(html: &str, base_url: Option<&str>) -> Option<String> {
    let document = Html::parse_fragment(html);
    let selector = Selector::parse("img[src]").ok()?;

    for element in document.select(&selector) {
        if let Some(src) = element.value().attr("src") {
            // Skip empty sources
            if src.trim().is_empty() {
                continue;
            }

            // Resolve URL
            let resolved = resolve_image_url(src, base_url);
            if let Some(ref url) = resolved {
                if is_valid_image_url(url) {
                    return resolved;
                }
            }
        }
    }

    None
}

/// Resolves a potentially relative image URL against a base URL.
/// Returns None if resolution fails or the input is invalid.
pub fn resolve_image_url(src: &str, base_url: Option<&str>) -> Option<String> {
    let src = src.trim();
    if src.is_empty() {
        return None;
    }

    // If already absolute, return as-is
    if src.starts_with("http://") || src.starts_with("https://") {
        return Some(src.to_string());
    }

    // Data URIs are already absolute
    if src.starts_with("data:") {
        return Some(src.to_string());
    }

    // Need a base URL to resolve relative paths
    let base = base_url?;
    let base_parsed = Url::parse(base).ok()?;

    // Resolve relative URL against base
    let resolved = base_parsed.join(src).ok()?;
    Some(resolved.to_string())
}

/// Checks if an image URL is valid (not a tracking pixel or similar).
/// Returns false for URLs matching known tracking/pixel patterns.
pub fn is_valid_image_url(url: &str) -> bool {
    let url_lower = url.to_lowercase();

    // Check for invalid patterns
    for pattern in INVALID_PATTERNS {
        if url_lower.contains(pattern) {
            return false;
        }
    }

    // Check for 1x1 dimensions in URL or query string
    if contains_tiny_dimensions(&url_lower) {
        return false;
    }

    true
}

/// Checks if URL contains indicators of 1x1 pixel images.
fn contains_tiny_dimensions(url: &str) -> bool {
    // Check for width=1 or height=1 in query parameters
    if url.contains("width=1") || url.contains("height=1") {
        return true;
    }

    // Check for w=1 or h=1 (common short forms)
    if url.contains("w=1&") || url.contains("&w=1") || url.ends_with("w=1") {
        return true;
    }
    if url.contains("h=1&") || url.contains("&h=1") || url.ends_with("h=1") {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_image_url_accepts_normal() {
        assert!(is_valid_image_url("https://example.com/image.jpg"));
        assert!(is_valid_image_url("https://example.com/photo.png"));
        assert!(is_valid_image_url("https://cdn.example.com/uploads/header.webp"));
    }

    #[test]
    fn test_is_valid_image_url_rejects_tracking() {
        assert!(!is_valid_image_url("https://example.com/tracking.gif"));
        assert!(!is_valid_image_url("https://example.com/pixel.png"));
        assert!(!is_valid_image_url("https://analytics.example.com/img.gif"));
        assert!(!is_valid_image_url("https://example.com/beacon/img.gif"));
        assert!(!is_valid_image_url("https://example.com/spacer.gif"));
        assert!(!is_valid_image_url("https://example.com/clear.gif"));
        assert!(!is_valid_image_url("https://example.com/blank.gif"));
        assert!(!is_valid_image_url("https://example.com/1x1.gif"));
    }

    #[test]
    fn test_is_valid_image_url_rejects_tiny_dimensions() {
        assert!(!is_valid_image_url("https://example.com/img.gif?width=1&height=1"));
        assert!(!is_valid_image_url("https://example.com/img.gif?w=1&h=1"));
    }

    #[test]
    fn test_is_valid_image_url_rejects_data_uri_pixel() {
        assert!(!is_valid_image_url("data:image/gif;base64,R0lGODlhAQABAI"));
    }

    #[test]
    fn test_resolve_image_url_absolute() {
        let result = resolve_image_url("https://example.com/image.jpg", None);
        assert_eq!(result, Some("https://example.com/image.jpg".to_string()));
    }

    #[test]
    fn test_resolve_image_url_relative_with_base() {
        let result = resolve_image_url("/images/photo.jpg", Some("https://example.com/article/1"));
        assert_eq!(result, Some("https://example.com/images/photo.jpg".to_string()));

        let result = resolve_image_url("photo.jpg", Some("https://example.com/article/"));
        assert_eq!(result, Some("https://example.com/article/photo.jpg".to_string()));
    }

    #[test]
    fn test_resolve_image_url_relative_without_base() {
        let result = resolve_image_url("/images/photo.jpg", None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_image_url_empty() {
        assert_eq!(resolve_image_url("", None), None);
        assert_eq!(resolve_image_url("   ", Some("https://example.com")), None);
    }

    #[test]
    fn test_extract_first_image_basic() {
        let html = r#"<p>Text</p><img src="https://example.com/image.jpg"><img src="https://example.com/second.jpg">"#;
        let result = extract_first_image(html, None);
        assert_eq!(result, Some("https://example.com/image.jpg".to_string()));
    }

    #[test]
    fn test_extract_first_image_skips_tracking() {
        let html = r#"<img src="https://example.com/pixel.gif"><img src="https://example.com/real.jpg">"#;
        let result = extract_first_image(html, None);
        assert_eq!(result, Some("https://example.com/real.jpg".to_string()));
    }

    #[test]
    fn test_extract_first_image_resolves_relative() {
        let html = r#"<img src="/images/photo.jpg">"#;
        let result = extract_first_image(html, Some("https://example.com/article/1"));
        assert_eq!(result, Some("https://example.com/images/photo.jpg".to_string()));
    }

    #[test]
    fn test_extract_first_image_no_images() {
        let html = "<p>No images here</p>";
        let result = extract_first_image(html, None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_first_image_empty_src() {
        let html = r#"<img src=""><img src="https://example.com/real.jpg">"#;
        let result = extract_first_image(html, None);
        assert_eq!(result, Some("https://example.com/real.jpg".to_string()));
    }
}
