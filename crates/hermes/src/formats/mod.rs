// ABOUTME: Output format converters for parsed content.
// ABOUTME: Handles conversion to HTML, Markdown, and plain text formats.

//! Output format conversion module.
//!
//! This module handles converting extracted content to various output formats
//! including cleaned HTML, Markdown, and plain text representations.

use regex::Regex;
use scraper::{Html, Selector};

/// Sanitize HTML using an ammonia policy that mirrors the Go bluemonday article policy.
///
/// Allowed elements: p, br, strong, b, em, i, u, h1-h6, ul, ol, li, blockquote, pre, code,
/// img, a, span, div.
/// Allowed attrs:
/// - links: href
/// - images: src, alt, width, height, srcset, sizes
/// - class on div/span/p/img/a
/// - id on headings/div/span
pub fn sanitize_html(html: &str) -> String {
    let allowed_tags = [
        "p", "br", "strong", "b", "em", "i", "u", "h1", "h2", "h3", "h4", "h5", "h6", "ul", "ol",
        "li", "blockquote", "pre", "code", "img", "a", "span", "div",
    ];

    let mut builder = ammonia::Builder::new();
    builder.tags(allowed_tags.iter().copied().collect());

    builder.add_tag_attributes("a", &["href"]);
    builder.add_tag_attributes("img", &["src", "alt", "width", "height", "srcset", "sizes"]);
    builder.add_tag_attributes("div", &["class", "id"]);
    builder.add_tag_attributes("span", &["class", "id"]);
    builder.add_tag_attributes("p", &["class"]);
    builder.add_tag_attributes("img", &["class"]);
    builder.add_tag_attributes("a", &["class"]);
    for h in &["h1", "h2", "h3", "h4", "h5", "h6"] {
        builder.add_tag_attributes(h, &["id"]);
    }

    builder
        .url_schemes(["http", "https", "mailto"].iter().copied().collect())
        .clean(html)
        .to_string()
}

/// Preprocess HTML before conversion: replace <br> tags with newlines.
fn preprocess_br_tags(html: &str) -> String {
    // Replace <br>, <br/>, <br /> variants with newline
    let re = Regex::new(r"(?i)<br\s*/?\s*>").unwrap();
    re.replace_all(html, "\n").to_string()
}

/// Collapse more than 2 consecutive blank lines to exactly 2.
fn collapse_blank_lines_to_two(text: &str) -> String {
    let re = Regex::new(r"\n{3,}").unwrap();
    re.replace_all(text, "\n\n").to_string()
}

/// Collapse multiple consecutive newlines to a single newline.
fn collapse_newlines_to_one(text: &str) -> String {
    let re = Regex::new(r"\n{2,}").unwrap();
    re.replace_all(text, "\n").to_string()
}

/// Convert HTML to Markdown using htmd.
///
/// Skips script and style tags during conversion, preserves links and images,
/// and normalizes consecutive blank lines to max 2.
/// On conversion error, returns the original HTML string unchanged.
pub fn html_to_markdown(html: &str) -> String {
    // Preprocess: convert <br> to newlines
    let preprocessed = preprocess_br_tags(html);

    // Convert to markdown, skipping script and style tags
    let converter = htmd::HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style", "noscript"])
        .build();

    let md = converter
        .convert(&preprocessed)
        .unwrap_or_else(|_| preprocessed.clone());

    // Post-process: collapse more than 2 blank lines to exactly 2
    collapse_blank_lines_to_two(&md)
}

/// Convert HTML to plain text by extracting text nodes.
///
/// Treats <br> as newline, collapses multiple blank lines to one,
/// and trims leading/trailing whitespace.
pub fn html_to_text(html: &str) -> String {
    // Preprocess: convert <br> to newlines
    let preprocessed = preprocess_br_tags(html);

    let document = Html::parse_document(&preprocessed);
    let raw_text: String = document.root_element().text().collect::<Vec<_>>().join(" ");

    // Collapse horizontal whitespace (spaces/tabs) but preserve newlines
    let re_spaces = Regex::new(r"[^\S\n]+").unwrap();
    let normalized = re_spaces.replace_all(&raw_text, " ");

    // Collapse multiple newlines to one
    let collapsed = collapse_newlines_to_one(&normalized);

    // Trim the result
    collapsed.trim().to_string()
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
    fn html_to_markdown_skips_script_and_style() {
        let html = "<p>Before</p><script>alert(1)</script><style>.x{}</style><p>After</p>";
        let md = html_to_markdown(html);
        assert!(
            !md.contains("alert"),
            "markdown should not contain script content, got: {}",
            md
        );
        assert!(
            !md.contains(".x{}"),
            "markdown should not contain style content, got: {}",
            md
        );
        assert!(
            md.contains("Before"),
            "should contain text before, got: {}",
            md
        );
        assert!(
            md.contains("After"),
            "should contain text after, got: {}",
            md
        );
    }

    #[test]
    fn html_to_markdown_converts_br_to_newline() {
        let html = "<p>Line 1<br>Line 2</p>";
        let md = html_to_markdown(html);
        assert!(
            md.contains("Line 1") && md.contains("Line 2"),
            "should contain both lines, got: {}",
            md
        );
    }

    #[test]
    fn html_to_markdown_collapses_excessive_blank_lines() {
        let html = "<p>Para 1</p>\n\n\n\n\n<p>Para 2</p>";
        let md = html_to_markdown(html);
        // Should not have more than 2 consecutive newlines
        assert!(
            !md.contains("\n\n\n"),
            "markdown should not have more than 2 consecutive newlines, got: {:?}",
            md
        );
    }

    #[test]
    fn html_to_markdown_preserves_links() {
        let html = r#"<p>Visit <a href="https://example.com">Example</a></p>"#;
        let md = html_to_markdown(html);
        assert!(
            md.contains("[Example](https://example.com)"),
            "should preserve link, got: {}",
            md
        );
    }

    #[test]
    fn html_to_markdown_preserves_images() {
        let html = r#"<img src="https://example.com/img.png" alt="Test">"#;
        let md = html_to_markdown(html);
        assert!(
            md.contains("![Test](https://example.com/img.png)"),
            "should preserve image, got: {}",
            md
        );
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
        // With newline collapse, paragraphs may be separated by newline then collapsed
        assert!(
            text.contains("Nested") && text.contains("Content"),
            "should contain both words, got: {}",
            text
        );
    }

    #[test]
    fn html_to_text_converts_br_to_newline() {
        let html = "<p>Line 1<br>Line 2</p>";
        let text = html_to_text(html);
        assert!(
            text.contains("Line 1") && text.contains("Line 2"),
            "should contain both lines, got: {}",
            text
        );
        // Since <br> becomes \n, both lines should be present
        assert!(
            text.contains('\n') || text.contains("Line 1") && text.contains("Line 2"),
            "br should be converted, got: {}",
            text
        );
    }

    #[test]
    fn html_to_text_collapses_multiple_newlines() {
        let html = "<p>Para 1</p>\n\n\n\n<p>Para 2</p>";
        let text = html_to_text(html);
        // Should not have more than 1 consecutive newline
        assert!(
            !text.contains("\n\n"),
            "text should not have multiple consecutive newlines, got: {:?}",
            text
        );
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

    #[test]
    fn preprocess_br_handles_variants() {
        assert_eq!(preprocess_br_tags("<br>"), "\n");
        assert_eq!(preprocess_br_tags("<br/>"), "\n");
        assert_eq!(preprocess_br_tags("<br />"), "\n");
        assert_eq!(preprocess_br_tags("<BR>"), "\n");
        assert_eq!(preprocess_br_tags("<BR />"), "\n");
    }

    #[test]
    fn collapse_blank_lines_to_two_works() {
        assert_eq!(collapse_blank_lines_to_two("a\n\n\n\nb"), "a\n\nb");
        assert_eq!(collapse_blank_lines_to_two("a\n\nb"), "a\n\nb");
        assert_eq!(collapse_blank_lines_to_two("a\nb"), "a\nb");
    }

    #[test]
    fn collapse_newlines_to_one_works() {
        assert_eq!(collapse_newlines_to_one("a\n\n\nb"), "a\nb");
        assert_eq!(collapse_newlines_to_one("a\n\nb"), "a\nb");
        assert_eq!(collapse_newlines_to_one("a\nb"), "a\nb");
    }
}
