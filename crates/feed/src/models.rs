// ABOUTME: Internal Rust models for parsed feed data.
// ABOUTME: Mirrors the ABI structs in .ai_agents/structs.md using native Rust types.

use serde::{Deserialize, Serialize};

/// Represents a media enclosure (audio, video, or image attachment).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Enclosure {
    pub url: String,
    pub mime_type: Option<String>,
    pub length: u64,
}

/// Represents an author with optional name, email, and URI.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Author {
    pub name: Option<String>,
    pub email: Option<String>,
    pub uri: Option<String>,
}

/// Represents a single item/entry within a feed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FeedItem {
    pub title: String,
    pub url: String,
    pub image_url: Option<String>,
    pub summary: String,
    pub content: String,
    pub guid: String,
    pub language: Option<String>,
    pub feed_type: String,
    pub published_ms: u64,
    pub updated_ms: u64,
    pub author: Option<Author>,
    pub categories: Vec<String>,
    pub enclosures: Vec<Enclosure>,
    pub primary_media_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub explicit_flag: bool,
    pub duration_seconds: u32,
}

/// Represents a parsed feed with metadata and items.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Feed {
    pub title: String,
    pub home_url: String,
    pub feed_url: String,
    pub description: String,
    pub language: Option<String>,
    pub image_url: Option<String>,
    pub author: Option<Author>,
    pub published_ms: u64,
    pub updated_ms: u64,
    pub items: Vec<FeedItem>,
    pub generator: Option<String>,
    pub copyright: Option<String>,
    pub feed_type: String,
}
