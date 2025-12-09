// ABOUTME: ReaderResult struct for FFI-friendly reader output.
// ABOUTME: Maps ParseResult fields to a flat structure suitable for C/FFI consumers.

use serde::{Deserialize, Serialize};

/// FFI-friendly reader result containing extracted article data.
/// All fields are simple types (Strings, u64, u32, bool) for easy C binding.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReaderResult {
    pub title: String,
    pub author: String,
    pub excerpt: String,
    pub content: String,
    pub url: String,
    pub site_name: String,
    pub domain: String,
    pub language: String,
    pub lead_image_url: String,
    pub favicon: String,
    pub theme_color: String,
    /// Publication timestamp in milliseconds since Unix epoch, 0 if unavailable.
    pub published_ms: u64,
    pub word_count: u64,
    pub total_pages: u32,
    pub rendered_pages: u32,
    pub has_video_metadata: bool,
    pub video_url: String,
}

impl ReaderResult {
    /// Create a ReaderResult from a ParseResult.
    pub fn from_parse_result(pr: &crate::ParseResult) -> Self {
        let published_ms = pr
            .date_published
            .map(|dt| dt.timestamp_millis() as u64)
            .unwrap_or(0);

        ReaderResult {
            title: pr.title.clone(),
            author: pr.author.clone().unwrap_or_default(),
            excerpt: pr
                .excerpt
                .clone()
                .or_else(|| pr.description.clone())
                .unwrap_or_default(),
            content: pr.content.clone(),
            url: pr.url.clone(),
            site_name: pr.site_name.clone().unwrap_or_default(),
            domain: pr.domain.clone(),
            language: pr.language.clone().unwrap_or_default(),
            lead_image_url: pr.lead_image_url.clone().unwrap_or_default(),
            favicon: pr.favicon.clone().unwrap_or_default(),
            theme_color: pr.theme_color.clone().unwrap_or_default(),
            published_ms,
            word_count: pr.word_count.max(0) as u64,
            total_pages: pr.total_pages.unwrap_or(1).max(0) as u32,
            rendered_pages: pr.rendered_pages.unwrap_or(1).max(0) as u32,
            has_video_metadata: pr.video_metadata.is_some(),
            video_url: pr.video_url.clone().unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParseResult;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_from_parse_result_full() {
        let dt = Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let pr = ParseResult {
            url: "https://example.com/article".to_string(),
            title: "Test Article".to_string(),
            content: "Article content here.".to_string(),
            author: Some("John Doe".to_string()),
            date_published: Some(dt),
            lead_image_url: Some("https://example.com/image.jpg".to_string()),
            domain: "example.com".to_string(),
            excerpt: Some("An excerpt.".to_string()),
            word_count: 100,
            site_name: Some("Example Site".to_string()),
            language: Some("en".to_string()),
            theme_color: Some("#ffffff".to_string()),
            favicon: Some("https://example.com/favicon.ico".to_string()),
            video_url: Some("https://example.com/video.mp4".to_string()),
            video_metadata: Some(serde_json::json!({"width": 1920})),
            total_pages: Some(3),
            rendered_pages: Some(2),
            ..Default::default()
        };

        let rr = ReaderResult::from_parse_result(&pr);

        assert_eq!(rr.title, "Test Article");
        assert_eq!(rr.author, "John Doe");
        assert_eq!(rr.excerpt, "An excerpt.");
        assert_eq!(rr.content, "Article content here.");
        assert_eq!(rr.url, "https://example.com/article");
        assert_eq!(rr.site_name, "Example Site");
        assert_eq!(rr.domain, "example.com");
        assert_eq!(rr.language, "en");
        assert_eq!(rr.lead_image_url, "https://example.com/image.jpg");
        assert_eq!(rr.favicon, "https://example.com/favicon.ico");
        assert_eq!(rr.theme_color, "#ffffff");
        assert_eq!(rr.published_ms, dt.timestamp_millis() as u64);
        assert_eq!(rr.word_count, 100);
        assert_eq!(rr.total_pages, 3);
        assert_eq!(rr.rendered_pages, 2);
        assert!(rr.has_video_metadata);
        assert_eq!(rr.video_url, "https://example.com/video.mp4");
    }

    #[test]
    fn test_from_parse_result_minimal() {
        let pr = ParseResult::default();
        let rr = ReaderResult::from_parse_result(&pr);

        assert_eq!(rr.title, "");
        assert_eq!(rr.author, "");
        assert_eq!(rr.excerpt, "");
        assert_eq!(rr.published_ms, 0);
        assert_eq!(rr.word_count, 0);
        assert_eq!(rr.total_pages, 1);
        assert_eq!(rr.rendered_pages, 1);
        assert!(!rr.has_video_metadata);
    }

    #[test]
    fn test_excerpt_falls_back_to_description() {
        let pr = ParseResult {
            description: Some("Description fallback.".to_string()),
            ..Default::default()
        };
        let rr = ReaderResult::from_parse_result(&pr);
        assert_eq!(rr.excerpt, "Description fallback.");
    }
}
