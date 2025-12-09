// ABOUTME: Feed enrichment helpers using metadata extraction outputs.
// ABOUTME: Applies Hermes metadata as fallbacks to parsed feeds.

use crate::models::Feed;
use digests_hermes::Metadata;

/// Applies metadata fallbacks to a feed.
///
/// Follows Go parity rules from UnifiedEnrichmentService.applyMetadataToFeeds:
/// - If feed title is empty, fill with metadata title.
/// - If feed description is empty, fill with metadata description.
/// - If feed image is empty, fill with metadata image_url (thumbnail).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_missing_fields_from_metadata() {
        let mut feed = Feed {
            title: String::new(),
            description: String::new(),
            image_url: None,
            ..Default::default()
        };

        let meta = Metadata {
            title: "Meta Title".to_string(),
            description: "Meta Description".to_string(),
            image_url: "https://example.com/img.jpg".to_string(),
            ..Default::default()
        };

        apply_metadata_to_feed(&mut feed, &meta);

        assert_eq!(feed.title, "Meta Title");
        assert_eq!(feed.description, "Meta Description");
        assert_eq!(feed.image_url.as_deref(), Some("https://example.com/img.jpg"));
    }

    #[test]
    fn does_not_override_existing_values() {
        let mut feed = Feed {
            title: "Keep Title".to_string(),
            description: "Keep Desc".to_string(),
            image_url: Some("https://existing.com/img.png".to_string()),
            ..Default::default()
        };

        let meta = Metadata {
            title: "Meta Title".to_string(),
            description: "Meta Description".to_string(),
            image_url: "https://example.com/img.jpg".to_string(),
            ..Default::default()
        };

        apply_metadata_to_feed(&mut feed, &meta);

        assert_eq!(feed.title, "Keep Title");
        assert_eq!(feed.description, "Keep Desc");
        assert_eq!(feed.image_url.as_deref(), Some("https://existing.com/img.png"));
    }
}
