// ABOUTME: Core feed parsing library for digests-core.
// ABOUTME: Provides feed parsing, time/duration parsing, HTML utilities, and image extraction.

pub mod duration_parse;
pub mod error;
pub mod html_utils;
pub mod image_utils;
pub mod itunes_ext;
pub mod models;
pub mod parser;
pub mod time_parse;

pub use duration_parse::parse_duration_seconds;
pub use error::FeedError;
pub use html_utils::{decode_entities, strip_html};
pub use image_utils::{extract_first_image, is_valid_image_url, resolve_image_url};
pub use models::{Author, Enclosure, Feed, FeedItem};
pub use parser::parse_feed_bytes;
pub use time_parse::parse_flexible_time;
