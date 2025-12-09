// ABOUTME: Selector-based field extraction utilities for extracting text and attributes from HTML.
// ABOUTME: Supports CSS selectors with text or attribute extraction, respecting allow_multiple and selector precedence.

//! Selector-based field extraction utilities.
//!
//! This module provides functions to extract text content or attribute values
//! from HTML documents using CSS selectors defined in `FieldExtractor` configs.
//!
//! Key behaviors:
//! - Selectors are tried in order; first selector yielding matches wins.
//! - Text extraction joins inner text with spaces and normalizes whitespace.
//! - Attribute extraction returns the attribute value trimmed.
//! - `allow_multiple`: when true, returns all matches from the winning selector;
//!   when false, returns only the first match.

use dom_query::Document;

use crate::extractors::compiled::get_or_compile;
use crate::extractors::custom::{FieldExtractor, SelectorSpec};

/// Normalizes whitespace in a string by collapsing runs of whitespace into single spaces.
fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extracts text or attribute values from an HTML document based on a `FieldExtractor`.
///
/// Iterates through `fe.selectors` in order. For each selector:
/// - `Css(s)`: selects nodes and extracts inner text (joined with spaces, normalized).
/// - `CssAttr([selector, attr])`: selects nodes and extracts the specified attribute value.
///
/// Returns values from the first selector that yields at least one non-empty match.
/// If `fe.allow_multiple` is false, returns only the first non-empty value (as a single-element vec).
/// If no selector yields matches, returns `None`.
pub fn extract_field_text(doc: &Document, fe: &FieldExtractor) -> Option<Vec<String>> {
    for spec in &fe.selectors {
        let results = extract_from_spec(doc, spec);
        if !results.is_empty() {
            if fe.allow_multiple {
                return Some(results);
            } else {
                // Return only the first match
                return Some(vec![results.into_iter().next().unwrap()]);
            }
        }
    }
    None
}

/// Convenience function that returns only the first extracted value.
///
/// Uses `extract_field_text` and returns the first element, or `None` if empty.
pub fn extract_field_first_text(doc: &Document, fe: &FieldExtractor) -> Option<String> {
    extract_field_text(doc, fe).and_then(|v| v.into_iter().next())
}

/// Extracts values from a single selector spec.
fn extract_from_spec(doc: &Document, spec: &SelectorSpec) -> Vec<String> {
    match spec {
        SelectorSpec::Css(css) => extract_text_from_css(doc, css),
        SelectorSpec::CssAttr(parts) => {
            if parts.len() >= 2 {
                extract_attr_from_css(doc, &parts[0], &parts[1])
            } else if parts.len() == 1 {
                // Fallback: treat single-element CssAttr as text extraction
                extract_text_from_css(doc, &parts[0])
            } else {
                vec![]
            }
        }
    }
}

/// Extracts inner text from elements matching a CSS selector.
fn extract_text_from_css(doc: &Document, css: &str) -> Vec<String> {
    // Use pre-compiled selector from cache
    let matcher = match get_or_compile(css) {
        Some(m) => m,
        None => return vec![], // Invalid selector
    };

    doc.select_matcher(&matcher)
        .iter()
        .filter_map(|el| {
            let text = el.text();
            let normalized = normalize_whitespace(&text);
            if normalized.is_empty() {
                None
            } else {
                Some(normalized)
            }
        })
        .collect()
}

/// Extracts an attribute value from elements matching a CSS selector.
fn extract_attr_from_css(doc: &Document, css: &str, attr: &str) -> Vec<String> {
    // Use pre-compiled selector from cache
    let matcher = match get_or_compile(css) {
        Some(m) => m,
        None => return vec![], // Invalid selector
    };

    doc.select_matcher(&matcher)
        .iter()
        .filter_map(|el| {
            el.attr(attr).map(|v| {
                let trimmed = v.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            })
        })
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_HTML: &str = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test Page</title></head>
        <body>
            <h1>  Main   Title  </h1>
            <h2>Subtitle</h2>
            <img class="hero" src="/images/hero.jpg" alt="Hero Image">
            <img class="thumb" src="/images/thumb.png" alt="Thumbnail">
            <ul class="items">
                <li>Item One</li>
                <li>Item Two</li>
                <li>Item Three</li>
            </ul>
            <div class="empty"></div>
            <p class="intro">Hello world</p>
        </body>
        </html>
    "#;

    fn parse_html() -> Document {
        Document::from(SAMPLE_HTML)
    }

    #[test]
    fn test_extract_title_via_css() {
        let doc = parse_html();
        let fe = FieldExtractor {
            selectors: vec![SelectorSpec::Css("h1".to_string())],
            allow_multiple: false,
            ..Default::default()
        };

        let result = extract_field_text(&doc, &fe);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], "Main Title"); // whitespace normalized
    }

    #[test]
    fn test_extract_attribute_via_css_attr() {
        let doc = parse_html();
        let fe = FieldExtractor {
            selectors: vec![SelectorSpec::CssAttr(vec![
                "img.hero".to_string(),
                "src".to_string(),
            ])],
            allow_multiple: false,
            ..Default::default()
        };

        let result = extract_field_first_text(&doc, &fe);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "/images/hero.jpg");
    }

    #[test]
    fn test_allow_multiple_returns_all_list_items() {
        let doc = parse_html();
        let fe = FieldExtractor {
            selectors: vec![SelectorSpec::Css("ul.items li".to_string())],
            allow_multiple: true,
            ..Default::default()
        };

        let result = extract_field_text(&doc, &fe);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 3);
        assert_eq!(values[0], "Item One");
        assert_eq!(values[1], "Item Two");
        assert_eq!(values[2], "Item Three");
    }

    #[test]
    fn test_selector_precedence_first_match_wins() {
        let doc = parse_html();
        // First selector matches h1, second matches h2 - should only return h1 result
        let fe = FieldExtractor {
            selectors: vec![
                SelectorSpec::Css("h1".to_string()),
                SelectorSpec::Css("h2".to_string()),
            ],
            allow_multiple: true,
            ..Default::default()
        };

        let result = extract_field_text(&doc, &fe);
        assert!(result.is_some());
        let values = result.unwrap();
        // Should only contain h1 result, not h2
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], "Main Title");
    }

    #[test]
    fn test_returns_none_when_no_selectors_match() {
        let doc = parse_html();
        let fe = FieldExtractor {
            selectors: vec![
                SelectorSpec::Css("article".to_string()),
                SelectorSpec::Css("section.nonexistent".to_string()),
            ],
            allow_multiple: false,
            ..Default::default()
        };

        let result = extract_field_text(&doc, &fe);
        assert!(result.is_none());
    }

    #[test]
    fn test_skips_empty_elements() {
        let doc = parse_html();
        // div.empty has no text content
        let fe = FieldExtractor {
            selectors: vec![
                SelectorSpec::Css("div.empty".to_string()),
                SelectorSpec::Css("p.intro".to_string()),
            ],
            allow_multiple: false,
            ..Default::default()
        };

        let result = extract_field_text(&doc, &fe);
        assert!(result.is_some());
        // Should skip empty div and return p.intro
        assert_eq!(result.unwrap()[0], "Hello world");
    }

    #[test]
    fn test_extract_multiple_images_attribute() {
        let doc = parse_html();
        let fe = FieldExtractor {
            selectors: vec![SelectorSpec::CssAttr(vec![
                "img".to_string(),
                "src".to_string(),
            ])],
            allow_multiple: true,
            ..Default::default()
        };

        let result = extract_field_text(&doc, &fe);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], "/images/hero.jpg");
        assert_eq!(values[1], "/images/thumb.png");
    }

    #[test]
    fn test_invalid_selector_returns_empty() {
        let doc = parse_html();
        let fe = FieldExtractor {
            selectors: vec![SelectorSpec::Css("[[[invalid".to_string())],
            allow_multiple: false,
            ..Default::default()
        };

        let result = extract_field_text(&doc, &fe);
        assert!(result.is_none());
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("  hello   world  "), "hello world");
        assert_eq!(normalize_whitespace("no\textra\nspaces"), "no extra spaces");
        assert_eq!(normalize_whitespace(""), "");
    }
}
