// ABOUTME: Item-level enrichment using Hermes metadata.
// ABOUTME: Fills missing thumbnails/images for feed items by fetching page metadata.

use std::collections::HashMap;

use digests_hermes::Metadata;

use crate::models::Feed;

/// Stats returned from item enrichment to aid diagnostics/tests.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ItemEnrichmentStats {
    /// Number of unique item URLs we attempted to enrich.
    pub urls_queued: usize,
    /// Number of items that already had thumbnails and were skipped.
    pub skipped_with_thumbnails: usize,
    /// Number of items whose thumbnail/image was filled from metadata.
    pub items_updated: usize,
}

/// Enrich feed items with metadata-derived thumbnails/images.
///
/// Mirrors digests-api's `collectArticleMetadata` + `processThumbnails` behavior:
/// - Only items without an existing thumbnail are queued.
/// - URLs are deduplicated; a single metadata fetch can update multiple items.
/// - If metadata.image_url is present, it is applied to both `thumbnail_url` and
///   `image_url` (keeping them in sync, matching the parser's selection logic).
///
/// `fetch_metadata` should synchronously fetch the page at the URL and return
/// Hermes `Metadata` (or `None` on any failure). Errors are swallowed to avoid
/// failing the whole parse.
pub fn enrich_items_with_metadata<F>(feed: &mut Feed, mut fetch_metadata: F) -> ItemEnrichmentStats
where
    F: FnMut(&str) -> Option<Metadata>,
{
    let mut stats = ItemEnrichmentStats::default();

    // Map of article URL -> indices of items needing enrichment
    let mut url_to_indices: HashMap<String, Vec<usize>> = HashMap::new();

    for (idx, item) in feed.items.iter().enumerate() {
        let has_thumb = item
            .thumbnail_url
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false);

        if has_thumb {
            stats.skipped_with_thumbnails += 1;
            continue;
        }

        if item.url.is_empty() {
            continue;
        }

        url_to_indices
            .entry(item.url.clone())
            .or_default()
            .push(idx);
    }

    stats.urls_queued = url_to_indices.len();

    for (url, indices) in url_to_indices {
        if let Some(meta) = fetch_metadata(&url) {
            if meta.image_url.is_empty() {
                continue;
            }

            for idx in indices {
                let item = &mut feed.items[idx];

                // Only overwrite when still missing/empty to avoid clobbering feed data.
                if item
                    .thumbnail_url
                    .as_ref()
                    .map(|s| s.is_empty())
                    .unwrap_or(true)
                {
                    item.thumbnail_url = Some(meta.image_url.clone());
                    stats.items_updated += 1;
                }

                if item
                    .image_url
                    .as_ref()
                    .map(|s| s.is_empty())
                    .unwrap_or(true)
                {
                    item.image_url = Some(meta.image_url.clone());
                }
            }
        }
    }

    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_missing_thumbnails_and_images() {
        let mut feed = Feed {
            items: vec![
                // missing thumb
                crate::models::FeedItem {
                    url: "https://example.com/a".into(),
                    ..Default::default()
                },
                // already has thumb -> skipped
                crate::models::FeedItem {
                    thumbnail_url: Some("keep".into()),
                    url: "https://example.com/b".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let meta = Metadata {
            image_url: "https://example.com/og.jpg".into(),
            ..Default::default()
        };

        let stats = enrich_items_with_metadata(&mut feed, |_| Some(meta.clone()));

        assert_eq!(stats.urls_queued, 1);
        assert_eq!(stats.items_updated, 1);
        assert_eq!(stats.skipped_with_thumbnails, 1);
        assert_eq!(
            feed.items[0].thumbnail_url.as_deref(),
            Some("https://example.com/og.jpg")
        );
        assert_eq!(
            feed.items[0].image_url.as_deref(),
            Some("https://example.com/og.jpg")
        );
        assert_eq!(feed.items[1].thumbnail_url.as_deref(), Some("keep"));
    }

    #[test]
    fn dedupes_urls_and_only_updates_missing() {
        let mut call_count = 0usize;
        let mut feed = Feed {
            items: vec![
                crate::models::FeedItem {
                    url: "https://example.com/a".into(),
                    ..Default::default()
                },
                crate::models::FeedItem {
                    url: "https://example.com/a".into(),
                    thumbnail_url: Some(String::new()), // treat empty as missing
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let meta = Metadata {
            image_url: "https://example.com/og.jpg".into(),
            ..Default::default()
        };

        let stats = enrich_items_with_metadata(&mut feed, |_| {
            call_count += 1;
            Some(meta.clone())
        });

        assert_eq!(call_count, 1, "should dedupe identical item URLs");
        assert_eq!(stats.urls_queued, 1);
        assert_eq!(stats.items_updated, 2);
        assert_eq!(
            feed.items[0].thumbnail_url.as_deref(),
            Some("https://example.com/og.jpg")
        );
        assert_eq!(
            feed.items[1].thumbnail_url.as_deref(),
            Some("https://example.com/og.jpg")
        );
    }
}
