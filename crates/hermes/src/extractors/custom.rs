// ABOUTME: Custom site-specific extractor data models and registry.
// ABOUTME: Defines configurable selectors and transforms for per-domain content extraction.

//! Custom extractor definitions for site-specific content extraction.
//!
//! This module provides data structures for defining custom extraction rules
//! on a per-domain basis, including CSS selectors, attribute extraction,
//! and content transforms.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Specifies how to select content from the DOM.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SelectorSpec {
    /// A simple CSS selector string, e.g., "h1.title"
    Css(String),
    /// A CSS selector with attribute extraction, e.g., ["img", "src"]
    CssAttr(Vec<String>),
}

impl Default for SelectorSpec {
    fn default() -> Self {
        SelectorSpec::Css(String::new())
    }
}

/// Specifies a transformation to apply to extracted content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransformSpec {
    /// Rename the element to a different tag
    Tag { value: String },
    /// Do nothing (placeholder)
    Noop,
    /// Replace <noscript> with <div> preserving inner HTML
    NoscriptToDiv,
    /// Remove the element but keep its children in place
    Unwrap,
    /// If element has attr `from`, copy value to `to` (overwrites existing `to`)
    MoveAttr { from: String, to: String },
    /// Set attribute to a fixed value
    SetAttr { name: String, value: String },
}

impl Default for TransformSpec {
    fn default() -> Self {
        TransformSpec::Noop
    }
}

/// Configuration for extracting a single field from a page.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FieldExtractor {
    /// List of selectors to try in order
    #[serde(default)]
    pub selectors: Vec<SelectorSpec>,
    /// Whether multiple matches are allowed
    #[serde(default)]
    pub allow_multiple: bool,
    /// Whether to apply default content cleaning
    #[serde(default)]
    pub default_cleaner: bool,
    /// Optional format string for date parsing
    #[serde(default)]
    pub format: Option<String>,
    /// Optional timezone for date parsing
    #[serde(default)]
    pub timezone: Option<String>,
}

/// Configuration for extracting the main content body.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContentExtractor {
    /// Base field extraction settings (includes selectors, allow_multiple, default_cleaner, format, timezone)
    #[serde(flatten)]
    pub field: FieldExtractor,
    /// List of selectors for elements to remove from content
    #[serde(default)]
    pub clean: Vec<String>,
    /// Transforms to apply to specific elements
    #[serde(default)]
    pub transforms: HashMap<String, TransformSpec>,
}

/// A complete custom extractor configuration for a domain.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomExtractor {
    /// Primary domain this extractor applies to
    pub domain: String,
    /// Additional domains this extractor supports
    #[serde(default)]
    pub supported_domains: Vec<String>,
    /// Title field extractor
    #[serde(default)]
    pub title: Option<FieldExtractor>,
    /// Author field extractor
    #[serde(default)]
    pub author: Option<FieldExtractor>,
    /// Main content extractor
    #[serde(default)]
    pub content: Option<ContentExtractor>,
    /// Publication date extractor
    #[serde(default)]
    pub date_published: Option<FieldExtractor>,
    /// Lead image URL extractor
    #[serde(default)]
    pub lead_image_url: Option<FieldExtractor>,
    /// Dek (subheadline) extractor
    #[serde(default)]
    pub dek: Option<FieldExtractor>,
    /// Next page URL extractor for paginated content
    #[serde(default)]
    pub next_page_url: Option<FieldExtractor>,
    /// Excerpt/summary extractor
    #[serde(default)]
    pub excerpt: Option<FieldExtractor>,
    /// Additional custom field extractors
    #[serde(default)]
    pub extend: HashMap<String, FieldExtractor>,
}

/// Registry for looking up custom extractors by domain.
#[derive(Debug, Default, Clone)]
pub struct ExtractorRegistry {
    map: HashMap<String, CustomExtractor>,
}

impl ExtractorRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an extractor for its primary and supported domains.
    pub fn register(&mut self, extractor: CustomExtractor) {
        let primary = extractor.domain.clone();
        let shared = extractor.clone();
        self.map.insert(primary, extractor);
        for dom in &shared.supported_domains {
            self.map.insert(dom.clone(), shared.clone());
        }
    }

    /// Looks up an extractor by domain.
    pub fn get(&self, domain: &str) -> Option<&CustomExtractor> {
        self.map.get(domain)
    }

    /// Returns the number of registered domain mappings.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns true if no extractors are registered.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Parses a selector spec into a CSS selector string and optional attribute name.
///
/// Returns (css_selector, optional_attribute).
pub fn parse_selector(selector: &SelectorSpec) -> (String, Option<String>) {
    match selector {
        SelectorSpec::Css(css) => (css.clone(), None),
        SelectorSpec::CssAttr(parts) => {
            if parts.len() >= 2 {
                (parts[0].clone(), Some(parts[1].clone()))
            } else if parts.len() == 1 {
                (parts[0].clone(), None)
            } else {
                (String::new(), None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_roundtrip() {
        let extractor = CustomExtractor {
            domain: "example.com".to_string(),
            supported_domains: vec!["www.example.com".to_string()],
            title: Some(FieldExtractor {
                selectors: vec![
                    SelectorSpec::Css("h1.title".to_string()),
                    SelectorSpec::CssAttr(vec![
                        "meta[property='og:title']".to_string(),
                        "content".to_string(),
                    ]),
                ],
                allow_multiple: false,
                default_cleaner: true,
                format: None,
                timezone: None,
            }),
            author: None,
            content: Some(ContentExtractor {
                field: FieldExtractor {
                    selectors: vec![SelectorSpec::Css("article.content".to_string())],
                    default_cleaner: true,
                    ..Default::default()
                },
                clean: vec![".ads".to_string(), ".social-share".to_string()],
                transforms: {
                    let mut m = HashMap::new();
                    m.insert(
                        "h2".to_string(),
                        TransformSpec::Tag {
                            value: "h3".to_string(),
                        },
                    );
                    m.insert("div.code".to_string(), TransformSpec::Noop);
                    m
                },
            }),
            date_published: None,
            lead_image_url: Some(FieldExtractor {
                selectors: vec![SelectorSpec::CssAttr(vec![
                    "img.hero".to_string(),
                    "src".to_string(),
                ])],
                ..Default::default()
            }),
            dek: None,
            next_page_url: None,
            excerpt: None,
            extend: HashMap::new(),
        };

        let json = serde_json::to_string_pretty(&extractor).expect("serialize");
        let parsed: CustomExtractor = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.domain, "example.com");
        assert_eq!(parsed.supported_domains, vec!["www.example.com"]);
        assert!(parsed.title.is_some());
        assert!(parsed.content.is_some());
        assert!(parsed.lead_image_url.is_some());

        let title = parsed.title.unwrap();
        assert_eq!(title.selectors.len(), 2);

        let content = parsed.content.unwrap();
        assert_eq!(content.clean.len(), 2);
        assert_eq!(content.transforms.len(), 2);
    }

    #[test]
    fn test_registry_lookup() {
        let mut registry = ExtractorRegistry::new();
        assert!(registry.is_empty());

        let extractor = CustomExtractor {
            domain: "example.com".to_string(),
            supported_domains: vec!["www.example.com".to_string()],
            title: Some(FieldExtractor {
                selectors: vec![SelectorSpec::Css("h1.title".to_string())],
                ..Default::default()
            }),
            ..Default::default()
        };

        registry.register(extractor);

        assert_eq!(registry.len(), 2);
        assert!(!registry.is_empty());

        let primary = registry.get("example.com");
        assert!(primary.is_some());
        let primary = primary.unwrap();
        assert_eq!(primary.domain, "example.com");

        let alias = registry.get("www.example.com");
        assert!(alias.is_some());
        let alias = alias.unwrap();
        assert_eq!(alias.domain, "example.com");

        // Both should have same title selector
        let primary_title = primary.title.as_ref().unwrap();
        let alias_title = alias.title.as_ref().unwrap();
        assert_eq!(primary_title.selectors.len(), alias_title.selectors.len());

        // Non-existent domain
        assert!(registry.get("other.com").is_none());
    }

    #[test]
    fn test_parse_selector_css() {
        let selector = SelectorSpec::Css("div.content".to_string());
        let (css, attr) = parse_selector(&selector);
        assert_eq!(css, "div.content");
        assert!(attr.is_none());
    }

    #[test]
    fn test_parse_selector_css_attr() {
        let selector = SelectorSpec::CssAttr(vec!["img.hero".to_string(), "src".to_string()]);
        let (css, attr) = parse_selector(&selector);
        assert_eq!(css, "img.hero");
        assert_eq!(attr, Some("src".to_string()));
    }

    #[test]
    fn test_parse_selector_css_attr_single() {
        let selector = SelectorSpec::CssAttr(vec!["img".to_string()]);
        let (css, attr) = parse_selector(&selector);
        assert_eq!(css, "img");
        assert!(attr.is_none());
    }

    #[test]
    fn test_parse_selector_css_attr_empty() {
        let selector = SelectorSpec::CssAttr(vec![]);
        let (css, attr) = parse_selector(&selector);
        assert_eq!(css, "");
        assert!(attr.is_none());
    }

    #[test]
    fn test_transform_spec_default() {
        let t: TransformSpec = Default::default();
        assert!(matches!(t, TransformSpec::Noop));
    }

    #[test]
    fn test_selector_spec_default() {
        let s: SelectorSpec = Default::default();
        assert!(matches!(s, SelectorSpec::Css(ref css) if css.is_empty()));
    }
}
