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
