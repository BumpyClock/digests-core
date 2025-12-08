// ABOUTME: ParseResult struct holding extracted article data from a parsed page.
// ABOUTME: Includes formatting helpers and convenience methods mirroring the Go API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The result of parsing a page, containing extracted article data.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ParseResult {
    pub url: String,
    pub title: String,
    pub content: String,
    pub author: Option<String>,
    pub date_published: Option<DateTime<Utc>>,
    pub lead_image_url: Option<String>,
    pub dek: Option<String>,
    pub domain: String,
    pub excerpt: Option<String>,
    pub word_count: i32,
    pub direction: Option<String>,
    pub total_pages: Option<i32>,
    pub rendered_pages: Option<i32>,
    pub site_name: Option<String>,
    pub site_title: Option<String>,
    pub site_image: Option<String>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub theme_color: Option<String>,
    pub favicon: Option<String>,
    pub video_url: Option<String>,
    pub video_metadata: Option<serde_json::Value>,
    pub next_page_url: Option<String>,
}

impl ParseResult {
    /// Format the result as a markdown document.
    pub fn format_markdown(&self) -> String {
        let mut parts = Vec::new();

        // Title
        if !self.title.is_empty() {
            parts.push(format!("# {}", self.title));
        }

        // Metadata line
        let mut meta = Vec::new();
        if let Some(ref author) = self.author {
            if !author.is_empty() {
                meta.push(format!("By {}", author));
            }
        }
        if let Some(date) = self.date_published {
            meta.push(date.format("%Y-%m-%d").to_string());
        }
        if !meta.is_empty() {
            parts.push(meta.join(" | "));
        }

        // Source
        if !self.url.is_empty() {
            parts.push(format!("Source: {}", self.url));
        }

        // Excerpt/Description
        if let Some(ref excerpt) = self.excerpt {
            if !excerpt.is_empty() {
                parts.push(format!("> {}", excerpt));
            }
        } else if let Some(ref description) = self.description {
            if !description.is_empty() {
                parts.push(format!("> {}", description));
            }
        }

        // Lead image
        if let Some(ref img) = self.lead_image_url {
            if !img.is_empty() {
                parts.push(format!("![Lead Image]({})", img));
            }
        }

        // Separator before content
        if !parts.is_empty() && !self.content.is_empty() {
            parts.push("---".to_string());
        }

        // Content
        if !self.content.is_empty() {
            parts.push(self.content.clone());
        }

        parts.join("\n\n")
    }

    /// Returns true if the result has no meaningful content.
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.content.is_empty()
    }

    /// Returns true if the result has an author.
    pub fn has_author(&self) -> bool {
        self.author.as_ref().map_or(false, |a| !a.is_empty())
    }

    /// Returns true if the result has a published date.
    pub fn has_date(&self) -> bool {
        self.date_published.is_some()
    }

    /// Returns true if the result has a lead image.
    pub fn has_image(&self) -> bool {
        self.lead_image_url
            .as_ref()
            .map_or(false, |u| !u.is_empty())
    }
}

/// Type alias for Go-like naming convention.
pub type Result = ParseResult;

/// Count words in a text string using whitespace splitting.
pub fn word_count(text: &str) -> i32 {
    text.split_whitespace().count() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_format_markdown_full() {
        let result = ParseResult {
            url: "https://example.com/article".to_string(),
            title: "Test Article".to_string(),
            content: "This is the article content.".to_string(),
            author: Some("John Doe".to_string()),
            date_published: Some(Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap()),
            lead_image_url: Some("https://example.com/image.jpg".to_string()),
            excerpt: Some("A brief excerpt of the article.".to_string()),
            domain: "example.com".to_string(),
            ..Default::default()
        };

        let md = result.format_markdown();
        assert!(md.contains("# Test Article"));
        assert!(md.contains("By John Doe"));
        assert!(md.contains("2024-06-15"));
        assert!(md.contains("Source: https://example.com/article"));
        assert!(md.contains("> A brief excerpt of the article."));
        assert!(md.contains("![Lead Image](https://example.com/image.jpg)"));
        assert!(md.contains("---"));
        assert!(md.contains("This is the article content."));
    }

    #[test]
    fn test_format_markdown_minimal() {
        let result = ParseResult {
            title: "Simple Title".to_string(),
            content: "Simple content.".to_string(),
            ..Default::default()
        };

        let md = result.format_markdown();
        assert_eq!(md, "# Simple Title\n\n---\n\nSimple content.");
    }

    #[test]
    fn test_is_empty_true() {
        let result = ParseResult::default();
        assert!(result.is_empty());
    }

    #[test]
    fn test_is_empty_false_with_title() {
        let result = ParseResult {
            title: "Has Title".to_string(),
            ..Default::default()
        };
        assert!(!result.is_empty());
    }

    #[test]
    fn test_is_empty_false_with_content() {
        let result = ParseResult {
            content: "Has content".to_string(),
            ..Default::default()
        };
        assert!(!result.is_empty());
    }

    #[test]
    fn test_has_author() {
        let mut result = ParseResult::default();
        assert!(!result.has_author());

        result.author = Some(String::new());
        assert!(!result.has_author());

        result.author = Some("Author Name".to_string());
        assert!(result.has_author());
    }

    #[test]
    fn test_has_date() {
        let mut result = ParseResult::default();
        assert!(!result.has_date());

        result.date_published = Some(Utc::now());
        assert!(result.has_date());
    }

    #[test]
    fn test_has_image() {
        let mut result = ParseResult::default();
        assert!(!result.has_image());

        result.lead_image_url = Some(String::new());
        assert!(!result.has_image());

        result.lead_image_url = Some("https://example.com/img.png".to_string());
        assert!(result.has_image());
    }
}
