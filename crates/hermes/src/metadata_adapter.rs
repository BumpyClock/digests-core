// ABOUTME: Lightweight metadata-only extractor for head parsing.
// ABOUTME: Extracts OG/Twitter/meta tags without full readability processing.

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::ParseError;

/// Metadata extracted from HTML head section.
/// Does not include full article content - just meta tags and basic info.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Metadata {
    pub title: String,
    pub description: String,
    pub site_name: String,
    /// Open Graph type (e.g., "article", "website")
    pub og_type: String,
    /// Canonical URL from meta tags
    pub url: String,
    /// Primary image URL (resolved to absolute)
    pub image_url: String,
    pub image_alt: String,
    /// Favicon URL (resolved to absolute)
    pub icon_url: String,
    pub theme_color: String,
    /// Document language (e.g., "en", "fr")
    pub language: String,
}

/// Helper to extract meta content by property attribute.
fn get_meta_property(document: &Html, property: &str) -> Option<String> {
    let sel_str = format!("meta[property='{}']", property);
    let sel = Selector::parse(&sel_str).ok()?;
    let elem = document.select(&sel).next()?;
    let content = elem.value().attr("content")?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Helper to extract meta content by name attribute.
fn get_meta_name(document: &Html, name: &str) -> Option<String> {
    let sel_str = format!("meta[name='{}']", name);
    let sel = Selector::parse(&sel_str).ok()?;
    let elem = document.select(&sel).next()?;
    let content = elem.value().attr("content")?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Helper to extract meta content by property first, then name as fallback.
fn get_meta(document: &Html, property: &str, name: &str) -> Option<String> {
    if !property.is_empty() {
        if let Some(val) = get_meta_property(document, property) {
            return Some(val);
        }
    }
    if !name.is_empty() {
        if let Some(val) = get_meta_name(document, name) {
            return Some(val);
        }
    }
    None
}

/// Extract metadata from HTML head without full readability processing.
///
/// # Arguments
/// * `html` - The raw HTML content to parse
/// * `base_url` - Base URL for resolving relative URLs
///
/// # Returns
/// Extracted `Metadata` or `ParseError::InvalidInput` if base_url is invalid.
pub fn extract_metadata_only(html: &str, base_url: &str) -> Result<Metadata, ParseError> {
    let base = Url::parse(base_url).map_err(|e| {
        ParseError::invalid_url(
            base_url,
            "extract_metadata_only",
            Some(anyhow::anyhow!("Invalid base URL: {}", e)),
        )
    })?;

    let document = Html::parse_document(html);
    let mut meta = Metadata::default();

    // Helper to resolve relative URLs
    let resolve_url = |url_str: &str| -> String {
        if url_str.is_empty() {
            return String::new();
        }
        match base.join(url_str) {
            Ok(resolved) => resolved.to_string(),
            Err(_) => url_str.to_string(),
        }
    };

    // Title: og:title > meta[name=title] > <title>
    meta.title = get_meta(&document, "og:title", "title").unwrap_or_else(|| {
        if let Ok(sel) = Selector::parse("title") {
            document
                .select(&sel)
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string())
                .unwrap_or_default()
        } else {
            String::new()
        }
    });

    // Description: og:description > description
    meta.description = get_meta(&document, "og:description", "description").unwrap_or_default();

    // Site name: og:site_name > application-name
    meta.site_name = get_meta(&document, "og:site_name", "application-name").unwrap_or_default();

    // OG type
    meta.og_type = get_meta(&document, "og:type", "").unwrap_or_default();

    // URL: og:url > canonical link
    meta.url = get_meta(&document, "og:url", "").unwrap_or_else(|| {
        if let Ok(sel) = Selector::parse("link[rel='canonical']") {
            document
                .select(&sel)
                .next()
                .and_then(|e| e.value().attr("href"))
                .map(|h| resolve_url(h))
                .unwrap_or_default()
        } else {
            String::new()
        }
    });

    // Image: og:image > twitter:image (twitter can be either property or name)
    let raw_image = get_meta(&document, "og:image", "")
        .or_else(|| get_meta(&document, "twitter:image", "twitter:image"))
        .unwrap_or_default();
    meta.image_url = resolve_url(&raw_image);

    // Image alt: og:image:alt > twitter:image:alt
    meta.image_alt = get_meta(&document, "og:image:alt", "")
        .or_else(|| get_meta(&document, "twitter:image:alt", "twitter:image:alt"))
        .unwrap_or_default();

    // Icon: link[rel='icon'] > link[rel='shortcut icon'] > link[rel='apple-touch-icon']
    let icon_selectors = [
        "link[rel='icon']",
        "link[rel='shortcut icon']",
        "link[rel='apple-touch-icon']",
    ];
    for sel_str in &icon_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(elem) = document.select(&sel).next() {
                if let Some(href) = elem.value().attr("href") {
                    let resolved = resolve_url(href);
                    if !resolved.is_empty() {
                        meta.icon_url = resolved;
                        break;
                    }
                }
            }
        }
    }

    // Theme color
    meta.theme_color = get_meta(&document, "", "theme-color").unwrap_or_default();

    // Language: html[lang] > og:locale > meta[name=language]
    if let Ok(sel) = Selector::parse("html") {
        if let Some(html_elem) = document.select(&sel).next() {
            if let Some(lang) = html_elem.value().attr("lang") {
                let trimmed = lang.trim();
                if !trimmed.is_empty() {
                    // Normalize to primary language tag (e.g., "en-US" -> "en")
                    meta.language = trimmed.split('-').next().unwrap_or(trimmed).to_lowercase();
                }
            }
        }
    }
    if meta.language.is_empty() {
        if let Some(locale) = get_meta(&document, "og:locale", "language") {
            meta.language = locale
                .split('-')
                .next()
                .unwrap_or(&locale)
                .split('_')
                .next()
                .unwrap_or(&locale)
                .to_lowercase();
        }
    }

    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_metadata_full() {
        let html = r##"
            <!DOCTYPE html>
            <html lang="en-US">
            <head>
                <title>Page Title</title>
                <meta property="og:title" content="OG Title">
                <meta property="og:description" content="OG Description">
                <meta property="og:site_name" content="Test Site">
                <meta property="og:type" content="article">
                <meta property="og:url" content="https://example.com/article">
                <meta property="og:image" content="/images/hero.jpg">
                <meta property="og:image:alt" content="Hero image">
                <meta name="theme-color" content="#ff0000">
                <link rel="icon" href="/favicon.ico">
            </head>
            <body></body>
            </html>
        "##;

        let result = extract_metadata_only(html, "https://example.com/post");
        assert!(result.is_ok());
        let meta = result.unwrap();

        assert_eq!(meta.title, "OG Title");
        assert_eq!(meta.description, "OG Description");
        assert_eq!(meta.site_name, "Test Site");
        assert_eq!(meta.og_type, "article");
        assert_eq!(meta.url, "https://example.com/article");
        assert_eq!(meta.image_url, "https://example.com/images/hero.jpg");
        assert_eq!(meta.image_alt, "Hero image");
        assert_eq!(meta.icon_url, "https://example.com/favicon.ico");
        assert_eq!(meta.theme_color, "#ff0000");
        assert_eq!(meta.language, "en");
    }

    #[test]
    fn test_extract_metadata_fallbacks() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Fallback Title</title>
                <meta name="description" content="Fallback Description">
                <meta name="application-name" content="My App">
                <link rel="canonical" href="https://example.com/canonical">
                <meta name="twitter:image" content="https://cdn.example.com/img.png">
                <meta name="language" content="fr_FR">
            </head>
            <body></body>
            </html>
        "#;

        let result = extract_metadata_only(html, "https://example.com/");
        assert!(result.is_ok());
        let meta = result.unwrap();

        assert_eq!(meta.title, "Fallback Title");
        assert_eq!(meta.description, "Fallback Description");
        assert_eq!(meta.site_name, "My App");
        assert_eq!(meta.url, "https://example.com/canonical");
        assert_eq!(meta.image_url, "https://cdn.example.com/img.png");
        assert_eq!(meta.language, "fr");
    }

    #[test]
    fn test_extract_metadata_relative_urls() {
        let html = r#"
            <!DOCTYPE html>
            <html lang="ja">
            <head>
                <meta property="og:image" content="../images/test.png">
                <link rel="icon" href="./favicon.ico">
            </head>
            <body></body>
            </html>
        "#;

        let result = extract_metadata_only(html, "https://example.com/blog/post/");
        assert!(result.is_ok());
        let meta = result.unwrap();

        assert_eq!(meta.image_url, "https://example.com/blog/images/test.png");
        assert_eq!(meta.icon_url, "https://example.com/blog/post/favicon.ico");
        assert_eq!(meta.language, "ja");
    }

    #[test]
    fn test_extract_metadata_invalid_base_url() {
        let result = extract_metadata_only("<html></html>", "not-a-valid-url");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_invalid_url());
    }

    #[test]
    fn test_extract_metadata_empty_html() {
        let result = extract_metadata_only("", "https://example.com/");
        assert!(result.is_ok());
        let meta = result.unwrap();
        assert_eq!(meta, Metadata::default());
    }
}
