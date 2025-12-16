// ABOUTME: Core feed parsing library for digests-core.
// ABOUTME: Provides feed parsing, time/duration parsing, HTML utilities, and image extraction.

pub mod duration_parse;
pub mod enrichment;
pub mod error;
pub mod html_utils;
pub mod image_utils;
pub mod item_enrichment;
pub mod itunes_ext;
pub mod models;
pub mod parser;
pub mod time_parse;

pub use duration_parse::parse_duration_seconds;
pub use enrichment::{apply_metadata_to_feed, enrich_feed_with_site_html};
pub use error::FeedError;
pub use html_utils::{decode_entities, strip_html};
pub use image_utils::{extract_first_image, is_valid_image_url, resolve_image_url};
pub use item_enrichment::{enrich_items_with_metadata, ItemEnrichmentStats};
pub use models::{Author, Enclosure, Feed, FeedItem};
pub use parser::parse_feed_bytes;
pub use time_parse::parse_flexible_time;

// ----------------------------------------------------------------------------
// URL utilities
// ----------------------------------------------------------------------------

use url::Url;

/// Extracts the base domain (scheme + host + optional port) from a URL.
pub fn base_domain(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    match parsed.port() {
        Some(port) => Some(format!("{}://{}:{}", parsed.scheme(), host, port)),
        None => Some(format!("{}://{}", parsed.scheme(), host)),
    }
}

/// Picks a representative site URL for a feed.
/// Prefers home_url if different from feed_url, otherwise falls back to base domain.
pub fn pick_site_url(feed: &Feed) -> Option<String> {
    if !feed.home_url.is_empty() && feed.home_url != feed.feed_url {
        return Some(feed.home_url.clone());
    }
    if !feed.feed_url.is_empty() {
        if let Some(base) = base_domain(&feed.feed_url) {
            return Some(base);
        }
        return Some(feed.feed_url.clone());
    }
    None
}
