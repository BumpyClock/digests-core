// ABOUTME: Output format converters for parsed content.
// ABOUTME: Handles conversion to HTML, Markdown, and plain text formats.

//! Output format conversion module.
//!
//! This module handles converting extracted content to various output formats
//! including cleaned HTML, Markdown, and plain text representations.

use scraper::{Html, Selector};

/// Sanitize HTML using ammonia with default policy.
///
/// Removes potentially dangerous elements like scripts, event handlers,
/// and other XSS vectors while preserving safe content.
pub fn sanitize_html(html: &str) -> String {
    ammonia::Builder::default().clean(html).to_string()
}

/// Convert HTML to Markdown using htmd.
///
/// On conversion error, returns the original HTML string unchanged.
pub fn html_to_markdown(html: &str) -> String {
    htmd::HtmlToMarkdown::new()
        .convert(html)
        .unwrap_or_else(|_| html.to_string())
}

/// Convert HTML to plain text by extracting text nodes.
///
/// Parses the HTML document and joins all text nodes with single spaces,
/// then trims leading/trailing whitespace.
pub fn html_to_text(html: &str) -> String {
    let document = Html::parse_document(html);
    document
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract title from HTML.
///
/// Tries selectors in order: `<title>`, `meta[property=og:title]`,
/// `meta[name=title]`, `<h1>`, `<h2>`. Returns the first non-empty trimmed text.
pub fn extract_title(html: &str) -> Option<String> {
    let document = Html::parse_document(html);

    // Try <title> tag first
    if let Ok(selector) = Selector::parse("title") {
        if let Some(element) = document.select(&selector).next() {
            let text: String = element.text().collect();
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    // Try og:title meta tag
    if let Ok(selector) = Selector::parse("meta[property='og:title']") {
        if let Some(element) = document.select(&selector).next() {
            if let Some(content) = element.value().attr("content") {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    // Try meta[name=title]
    if let Ok(selector) = Selector::parse("meta[name='title']") {
        if let Some(element) = document.select(&selector).next() {
            if let Some(content) = element.value().attr("content") {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    // Fall back to first <h1>
    if let Ok(selector) = Selector::parse("h1") {
        if let Some(element) = document.select(&selector).next() {
            let text: String = element.text().collect();
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    // Fall back to first <h2>
    if let Ok(selector) = Selector::parse("h2") {
        if let Some(element) = document.select(&selector).next() {
            let text: String = element.text().collect();
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    None
}

/// Extract excerpt from HTML.
///
/// Converts HTML to plain text, trims whitespace, and returns the first 200
/// characters. Returns None if the resulting text is empty.
pub fn extract_excerpt(html: &str) -> Option<String> {
    let text = html_to_text(html);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let excerpt: String = trimmed.chars().take(200).collect();
    Some(excerpt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_to_markdown_converts_h1() {
        let html = "<h1>Hello</h1>";
        let md = html_to_markdown(html);
        assert!(
            md.starts_with("# Hello"),
            "expected markdown h1, got: {}",
            md
        );
    }

    #[test]
    fn html_to_markdown_converts_complex_html() {
        let html = "<h2>Title</h2><p>Some <strong>bold</strong> text.</p>";
        let md = html_to_markdown(html);
        assert!(md.contains("## Title"), "expected markdown h2, got: {}", md);
        assert!(
            md.contains("**bold**"),
            "expected bold markdown, got: {}",
            md
        );
    }

    #[test]
    fn html_to_markdown_returns_original_on_empty() {
        let html = "";
        let md = html_to_markdown(html);
        assert_eq!(md, "");
    }

    #[test]
    fn html_to_text_extracts_text_and_collapses_whitespace() {
        let html = "<p>Hello   world</p>";
        let text = html_to_text(html);
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn html_to_text_strips_tags() {
        let html = "<div><span>One</span> <em>Two</em> <strong>Three</strong></div>";
        let text = html_to_text(html);
        assert_eq!(text, "One Two Three");
    }

    #[test]
    fn html_to_text_trims_whitespace() {
        let html = "   <p>  trimmed  </p>   ";
        let text = html_to_text(html);
        assert_eq!(text, "trimmed");
    }

    #[test]
    fn html_to_text_handles_nested_elements() {
        let html = "<html><body><div><p>Nested</p><p>Content</p></div></body></html>";
        let text = html_to_text(html);
        assert_eq!(text, "Nested Content");
    }

    #[test]
    fn extract_title_finds_title_tag() {
        let html =
            "<html><head><title>Page Title</title></head><body><h1>Heading</h1></body></html>";
        let title = extract_title(html);
        assert_eq!(title, Some("Page Title".to_string()));
    }

    #[test]
    fn extract_title_falls_back_to_h1() {
        let html = "<html><body><h1>Main Heading</h1><p>content</p></body></html>";
        let title = extract_title(html);
        assert_eq!(title, Some("Main Heading".to_string()));
    }

    #[test]
    fn extract_title_returns_none_when_no_title_or_h1() {
        let html = "<html><body><p>Just a paragraph</p></body></html>";
        let title = extract_title(html);
        assert_eq!(title, None);
    }

    #[test]
    fn extract_title_trims_whitespace() {
        let html = "<html><head><title>  Padded Title  </title></head></html>";
        let title = extract_title(html);
        assert_eq!(title, Some("Padded Title".to_string()));
    }

    #[test]
    fn extract_title_skips_empty_title_uses_h1() {
        let html = "<html><head><title>   </title></head><body><h1>Fallback</h1></body></html>";
        let title = extract_title(html);
        assert_eq!(title, Some("Fallback".to_string()));
    }

    #[test]
    fn extract_excerpt_returns_text_up_to_200_chars() {
        let html = "<p>Hello world</p>";
        let excerpt = extract_excerpt(html);
        assert_eq!(excerpt, Some("Hello world".to_string()));
    }

    #[test]
    fn extract_excerpt_truncates_long_text() {
        let long_text = "a".repeat(300);
        let html = format!("<p>{}</p>", long_text);
        let excerpt = extract_excerpt(&html);
        assert_eq!(excerpt.as_ref().map(|s| s.len()), Some(200));
        assert_eq!(excerpt, Some("a".repeat(200)));
    }

    #[test]
    fn extract_excerpt_returns_none_for_empty() {
        let html = "<html><body></body></html>";
        let excerpt = extract_excerpt(html);
        assert_eq!(excerpt, None);
    }

    #[test]
    fn extract_excerpt_trims_whitespace() {
        let html = "   <p>  trimmed content  </p>   ";
        let excerpt = extract_excerpt(html);
        assert_eq!(excerpt, Some("trimmed content".to_string()));
    }
}
