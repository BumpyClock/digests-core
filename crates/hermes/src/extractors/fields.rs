// ABOUTME: Generic field extraction utilities for author, date, and lead image.
// ABOUTME: Provides helpers to extract meta content, attributes, and text with fallback selectors.

//! Generic field extraction utilities.
//!
//! This module provides helper functions for extracting common fields
//! (author, date_published, lead_image_url) from HTML documents using
//! a prioritized list of CSS selectors.
//!
//! Key behaviors:
//! - Selectors are tried in order; first non-empty match wins.
//! - Whitespace is normalized (collapsed to single spaces, trimmed).
//! - Empty strings are treated as no match.

use scraper::{Html, Selector};

/// Normalizes whitespace in a string by collapsing runs of whitespace into single spaces.
fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extracts an attribute value from the first matching selector that yields a non-empty result.
///
/// Iterates through selectors in order, returning the first non-empty attribute value found.
/// Useful for extracting href, src, content, or other attributes from meta tags and links.
///
/// # Arguments
/// * `doc` - The parsed HTML document
/// * `selectors` - Slice of CSS selector strings to try in order
/// * `attr` - The attribute name to extract
///
/// # Returns
/// `Some(String)` with the trimmed attribute value, or `None` if no match found.
pub fn extract_first_attr(doc: &Html, selectors: &[&str], attr: &str) -> Option<String> {
    for &sel_str in selectors {
        let sel = match Selector::parse(sel_str) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for el in doc.select(&sel) {
            if let Some(value) = el.value().attr(attr) {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

/// Extracts text content from the first matching selector that yields a non-empty result.
///
/// Iterates through selectors in order. For meta tags (selectors starting with "meta["),
/// extracts the `content` attribute. For other elements, extracts and normalizes inner text.
///
/// # Arguments
/// * `doc` - The parsed HTML document
/// * `selectors` - Slice of CSS selector strings to try in order
///
/// # Returns
/// `Some(String)` with the extracted text, or `None` if no match found.
pub fn extract_first_text(doc: &Html, selectors: &[&str]) -> Option<String> {
    extract_field_text_single(doc, selectors)
}

/// Normalizes a language/locale string to its primary language tag.
///
/// Converts to lowercase, splits on '-' or '_', and returns the first part.
/// For example: "en_US" -> "en", "EN-GB" -> "en", "fr" -> "fr".
///
/// # Arguments
/// * `value` - The language/locale string to normalize
///
/// # Returns
/// The normalized primary language tag (lowercase, trimmed).
pub fn normalize_lang(value: &str) -> String {
    let trimmed = value.trim().to_lowercase();
    // Split on '-' or '_' and take the first part
    trimmed
        .split(|c| c == '-' || c == '_')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Extracts the `content` attribute from the first matching meta tag.
///
/// Parses the selector (which should target a `<meta>` element), then
/// returns the trimmed `content` attribute value if present and non-empty.
///
/// # Arguments
/// * `doc` - The parsed HTML document
/// * `selector` - A CSS selector string targeting a meta element
///
/// # Returns
/// `Some(String)` with the content attribute value, or `None` if not found or empty.
pub fn extract_meta_content(doc: &Html, selector: &str) -> Option<String> {
    let sel = Selector::parse(selector).ok()?;
    for el in doc.select(&sel) {
        if let Some(content) = el.value().attr("content") {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Extracts an attribute value from the first matching element.
///
/// Parses the selector, finds matching elements, and returns the first
/// non-empty attribute value.
///
/// # Arguments
/// * `doc` - The parsed HTML document
/// * `selector` - A CSS selector string
/// * `attr` - The attribute name to extract
///
/// # Returns
/// `Some(String)` with the attribute value, or `None` if not found or empty.
pub fn extract_attr_first(doc: &Html, selector: &str, attr: &str) -> Option<String> {
    let sel = Selector::parse(selector).ok()?;
    for el in doc.select(&sel) {
        if let Some(value) = el.value().attr(attr) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Extracts text content from the first selector that yields a non-empty match.
///
/// Iterates through the provided selectors in order. For each selector:
/// - If targeting a meta tag (contains "meta["), extracts the `content` attribute.
/// - Otherwise, extracts and normalizes the inner text.
///
/// Returns the first non-empty trimmed string found.
///
/// # Arguments
/// * `doc` - The parsed HTML document
/// * `selectors` - Slice of CSS selector strings to try in order
///
/// # Returns
/// `Some(String)` with the extracted text, or `None` if no selector yields a match.
pub fn extract_field_text_single(doc: &Html, selectors: &[&str]) -> Option<String> {
    for &sel_str in selectors {
        // For meta tags, extract content attribute
        if sel_str.starts_with("meta[") {
            if let Some(value) = extract_meta_content(doc, sel_str) {
                return Some(value);
            }
            continue;
        }

        // For other elements, extract inner text
        let sel = match Selector::parse(sel_str) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for el in doc.select(&sel) {
            let text: String = el.text().collect::<Vec<_>>().join(" ");
            let normalized = normalize_whitespace(&text);
            if !normalized.is_empty() {
                return Some(normalized);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_HTML: &str = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <meta name="author" content="  Jane Doe  ">
            <meta property="og:image" content="https://example.com/og.jpg">
            <meta name="twitter:image" content="https://example.com/tw.jpg">
            <meta property="article:published_time" content="2024-01-15T10:00:00Z">
        </head>
        <body>
            <span class="byline">By John Smith</span>
            <p class="author">Author: Alice</p>
            <time datetime="2023-12-01T12:00:00Z">December 1, 2023</time>
            <img src="/local.jpg" alt="Local Image">
        </body>
        </html>
    "#;

    fn parse_html() -> Html {
        Html::parse_document(SAMPLE_HTML)
    }

    #[test]
    fn test_extract_meta_content_author() {
        let doc = parse_html();
        let result = extract_meta_content(&doc, "meta[name=author]");
        assert_eq!(result, Some("Jane Doe".to_string()));
    }

    #[test]
    fn test_extract_meta_content_og_image() {
        let doc = parse_html();
        let result = extract_meta_content(&doc, "meta[property='og:image']");
        assert_eq!(result, Some("https://example.com/og.jpg".to_string()));
    }

    #[test]
    fn test_extract_meta_content_not_found() {
        let doc = parse_html();
        let result = extract_meta_content(&doc, "meta[name=nonexistent]");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_attr_first_datetime() {
        let doc = parse_html();
        let result = extract_attr_first(&doc, "time[datetime]", "datetime");
        assert_eq!(result, Some("2023-12-01T12:00:00Z".to_string()));
    }

    #[test]
    fn test_extract_attr_first_img_src() {
        let doc = parse_html();
        let result = extract_attr_first(&doc, "img", "src");
        assert_eq!(result, Some("/local.jpg".to_string()));
    }

    #[test]
    fn test_extract_attr_first_not_found() {
        let doc = parse_html();
        let result = extract_attr_first(&doc, "video", "src");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_field_text_single_meta() {
        let doc = parse_html();
        let selectors = &["meta[name=author]", ".byline"];
        let result = extract_field_text_single(&doc, selectors);
        assert_eq!(result, Some("Jane Doe".to_string()));
    }

    #[test]
    fn test_extract_field_text_single_text() {
        let doc = parse_html();
        let selectors = &[".byline", ".author"];
        let result = extract_field_text_single(&doc, selectors);
        assert_eq!(result, Some("By John Smith".to_string()));
    }

    #[test]
    fn test_extract_field_text_single_fallback() {
        let doc = parse_html();
        let selectors = &[".nonexistent", ".author"];
        let result = extract_field_text_single(&doc, selectors);
        assert_eq!(result, Some("Author: Alice".to_string()));
    }

    #[test]
    fn test_extract_field_text_single_no_match() {
        let doc = parse_html();
        let selectors = &[".foo", ".bar", ".baz"];
        let result = extract_field_text_single(&doc, selectors);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_first_attr_ordered() {
        let doc = parse_html();
        // First selector matches, should return its value
        let selectors = &["meta[property='og:image']", "meta[name='twitter:image']"];
        let result = extract_first_attr(&doc, selectors, "content");
        assert_eq!(result, Some("https://example.com/og.jpg".to_string()));
    }

    #[test]
    fn test_extract_first_attr_fallback() {
        let doc = parse_html();
        // First selector doesn't exist, should fallback to second
        let selectors = &["meta[name=nonexistent]", "meta[property='og:image']"];
        let result = extract_first_attr(&doc, selectors, "content");
        assert_eq!(result, Some("https://example.com/og.jpg".to_string()));
    }

    #[test]
    fn test_extract_first_attr_no_match() {
        let doc = parse_html();
        let selectors = &["meta[name=foo]", "meta[name=bar]"];
        let result = extract_first_attr(&doc, selectors, "content");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_first_text_delegates() {
        let doc = parse_html();
        let selectors = &["meta[name=author]", ".byline"];
        let result = extract_first_text(&doc, selectors);
        assert_eq!(result, Some("Jane Doe".to_string()));
    }

    #[test]
    fn test_normalize_lang_underscore() {
        assert_eq!(normalize_lang("en_US"), "en");
    }

    #[test]
    fn test_normalize_lang_hyphen() {
        assert_eq!(normalize_lang("EN-GB"), "en");
    }

    #[test]
    fn test_normalize_lang_simple() {
        assert_eq!(normalize_lang("fr"), "fr");
    }

    #[test]
    fn test_normalize_lang_whitespace() {
        assert_eq!(normalize_lang("  de_AT  "), "de");
    }

    #[test]
    fn test_normalize_lang_empty() {
        assert_eq!(normalize_lang(""), "");
    }
}
