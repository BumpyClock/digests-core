// ABOUTME: Feed-level enrichment helpers aligning with digests-api behavior.
// ABOUTME: Applies Hermes metadata HTML to fill missing feed fields (title/description/image).

use crate::error::FeedError;
use crate::models::Feed;
use digests_hermes::{extract_metadata_only, Metadata};

/// Apply metadata fallbacks to an already-parsed feed.
/// Mirrors digests-api's UnifiedEnrichmentService.applyMetadataToFeeds logic.
pub fn apply_metadata_to_feed(feed: &mut Feed, metadata: &Metadata) {
    if feed.title.is_empty() && !metadata.title.is_empty() {
        feed.title = metadata.title.clone();
    }
    if feed.description.is_empty() && !metadata.description.is_empty() {
        feed.description = metadata.description.clone();
    }
    if feed.image_url.is_none() || feed.image_url.as_deref().unwrap_or("").is_empty() {
        if !metadata.image_url.is_empty() {
            feed.image_url = Some(metadata.image_url.clone());
        }
    }
}

/// Convenience: parse site HTML to metadata and apply to feed.
/// This expects the caller to supply the website HTML (not the feed XML).
pub fn enrich_feed_with_site_html(
    feed: &mut Feed,
    site_html: &str,
    site_url: &str,
) -> Result<(), FeedError> {
    let meta = extract_metadata_only(site_html, site_url)
        .map_err(|e| FeedError::invalid(format!("metadata extraction failed: {e}")))?;
    apply_metadata_to_feed(feed, &meta);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_missing_fields() {
        let html = r#"
        <html><head>
          <title>Site Title</title>
          <meta name="description" content="Site Description">
          <meta property="og:image" content="https://example.com/img.jpg">
        </head><body></body></html>
        "#;
        let mut feed = Feed {
            title: "".into(),
            description: "".into(),
            image_url: None,
            ..Default::default()
        };
        enrich_feed_with_site_html(&mut feed, html, "https://example.com/").unwrap();
        assert_eq!(feed.title, "Site Title");
        assert_eq!(feed.description, "Site Description");
        assert_eq!(
            feed.image_url.as_deref(),
            Some("https://example.com/img.jpg")
        );
    }

    #[test]
    fn does_not_override_existing() {
        let html = r#"<html><head><title>Meta</title><meta name=\"description\" content=\"Desc\"></head></html>"#;
        let mut feed = Feed {
            title: "Keep".into(),
            description: "KeepDesc".into(),
            image_url: Some("keep".into()),
            ..Default::default()
        };
        enrich_feed_with_site_html(&mut feed, html, "https://example.com/").unwrap();
        assert_eq!(feed.title, "Keep");
        assert_eq!(feed.description, "KeepDesc");
        assert_eq!(feed.image_url.as_deref(), Some("keep"));
    }
}
