// ABOUTME: Feed parsing implementation using feed-rs.
// ABOUTME: Maps feed-rs types to internal models with iTunes metadata extraction.

use crate::error::FeedError;
use crate::html_utils::strip_html;
use crate::image_utils::extract_first_image;
use crate::itunes_ext::{
    is_explicit, parse_item_duration, parse_itunes_extensions, ItemITunesExt,
    ParsedITunesExtensions,
};
use crate::models::{Author, Enclosure, Feed, FeedItem};
use chrono::Utc;
use feed_rs::model::{Entry, Feed as FeedRsFeed, Link, Person};
use std::collections::HashSet;

/// Parses feed bytes into a Feed struct.
///
/// # Arguments
/// * `data` - Raw feed bytes (RSS, Atom, or JSON Feed)
/// * `feed_url` - The URL the feed was fetched from (stored as-is)
///
/// # Returns
/// * `Ok(Feed)` - Successfully parsed feed with items
/// * `Err(FeedError)` - Parse failed, invalid feed, or empty feed
pub fn parse_feed_bytes(data: &[u8], feed_url: &str) -> Result<Feed, FeedError> {
    let parsed = feed_rs::parser::parse(data).map_err(FeedError::parse)?;

    // Parse iTunes extensions from raw XML (feed-rs doesn't expose all iTunes metadata)
    let itunes_ext = parse_itunes_extensions(data);

    let feed_type = detect_feed_type(&parsed, &itunes_ext);
    let feed_language = parsed.language.clone();

    // Extract feed-level author (iTunes author overrides if no standard author)
    let feed_author = extract_feed_author(&parsed, &itunes_ext);

    // Extract feed-level image (iTunes image has priority)
    let feed_image_url = extract_feed_image(&parsed, &itunes_ext);

    // Map items
    let items: Vec<FeedItem> = parsed
        .entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let item_ext = itunes_ext
                .items
                .get(&entry.id)
                .or_else(|| itunes_ext.items_by_index.get(idx))
                .cloned()
                .unwrap_or_default();
            map_entry(entry, &feed_type, feed_language.as_deref(), &item_ext)
        })
        .collect();

    // Build feed
    let feed = Feed {
        title: parsed.title.map(|t| t.content).unwrap_or_default(),
        home_url: extract_home_url(&parsed.links),
        feed_url: feed_url.to_string(),
        description: parsed.description.map(|d| d.content).unwrap_or_default(),
        language: feed_language,
        image_url: feed_image_url,
        author: feed_author,
        published_ms: parsed
            .published
            .map(|dt| dt.timestamp_millis() as u64)
            .unwrap_or(0),
        updated_ms: parsed
            .updated
            .map(|dt| dt.timestamp_millis() as u64)
            .or_else(|| parsed.published.map(|dt| dt.timestamp_millis() as u64))
            .unwrap_or_else(|| Utc::now().timestamp_millis() as u64),
        items,
        generator: parsed.generator.map(|g| g.content),
        copyright: parsed.rights.map(|r| r.content),
        feed_type,
    };

    Ok(feed)
}

/// Detects whether the feed is a podcast or article feed.
///
/// Rules per requirements:
/// 1. If feed.extensions contains namespace "itunes" (any key) OR feed.rating.urn == "itunes" => "podcast"
/// 2. Else sample first up to 5 entries; for each entry count as podcast indicator if:
///    (a) entry.extensions has namespace "itunes", OR
///    (b) any link rel=="enclosure" with media_type starting with audio/ or video/, OR
///    (c) any media.content content_type starts with audio/ or video/
///    Majority -> podcast else article
fn detect_feed_type(feed: &FeedRsFeed, itunes_ext: &ParsedITunesExtensions) -> String {
    // Check for iTunes namespace at feed level
    if itunes_ext.feed.has_itunes_namespace {
        return "podcast".to_string();
    }

    // Check for iTunes rating at feed level (feed-rs stores iTunes explicit as rating)
    if let Some(ref rating) = feed.rating {
        if rating.urn == "itunes" {
            return "podcast".to_string();
        }
    }

    // Check first 5 entries for podcast indicators
    let check_count = feed.entries.len().min(5);
    if check_count == 0 {
        return "article".to_string();
    }

    let mut podcast_count = 0;
    for (idx, entry) in feed.entries.iter().take(check_count).enumerate() {
        let item_ext = itunes_ext
            .items
            .get(&entry.id)
            .or_else(|| itunes_ext.items_by_index.get(idx));

        if has_podcast_indicators(entry, item_ext) {
            podcast_count += 1;
        }
    }

    // Majority rule: podcast_count * 2 > check_count
    if podcast_count * 2 > check_count {
        "podcast".to_string()
    } else {
        "article".to_string()
    }
}

/// Checks if an entry has podcast indicators.
///
/// Per requirements:
/// (a) entry.extensions has namespace "itunes" (we check via ItemITunesExt having any data), OR
/// (b) any link rel=="enclosure" with media_type starting with audio/ or video/, OR
/// (c) any media.content content_type starts with audio/ or video/
fn has_podcast_indicators(entry: &Entry, item_ext: Option<&ItemITunesExt>) -> bool {
    // (a) Check if entry has iTunes extension data
    if let Some(ext) = item_ext {
        if ext.duration.is_some()
            || ext.explicit.is_some()
            || ext.image_href.is_some()
            || ext.author.is_some()
        {
            return true;
        }
    }

    // (b) Check for link rel="enclosure" with audio/video type
    for link in &entry.links {
        if is_enclosure_link(link) {
            if let Some(ref media_type) = link.media_type {
                let mime = media_type.to_string();
                if mime.starts_with("audio/") || mime.starts_with("video/") {
                    return true;
                }
            }
        }
    }

    // (c) Check media.content for audio/video types
    for media in &entry.media {
        for content in &media.content {
            if let Some(ref content_type) = content.content_type {
                let mime = content_type.to_string();
                if mime.starts_with("audio/") || mime.starts_with("video/") {
                    return true;
                }
            }
        }
    }

    false
}

/// Checks if a link is an enclosure link (rel == "enclosure" or LinkRel::Enclosure).
fn is_enclosure_link(link: &Link) -> bool {
    if let Some(ref rel) = link.rel {
        return rel == "enclosure";
    }
    false
}

/// Extracts the home URL from feed links.
/// Prefers link with rel="alternate", otherwise uses first link href.
fn extract_home_url(links: &[Link]) -> String {
    // First try rel="alternate"
    for link in links {
        if link.rel.as_deref() == Some("alternate") {
            return link.href.clone();
        }
    }

    // Fall back to first link
    links.first().map(|l| l.href.clone()).unwrap_or_default()
}

/// Extracts the item URL from entry links.
/// Prefers link with rel="alternate", otherwise first non-enclosure link, then entry.id.
fn extract_item_url(entry: &Entry) -> String {
    // First try rel="alternate"
    for link in &entry.links {
        if link.rel.as_deref() == Some("alternate") {
            return link.href.clone();
        }
    }

    // Fall back to first non-enclosure link
    for link in &entry.links {
        if !is_enclosure_link(link) {
            return link.href.clone();
        }
    }

    // Fall back to entry id
    entry.id.clone()
}

/// Extracts feed-level author.
/// iTunes author overrides if no standard author.
fn extract_feed_author(feed: &FeedRsFeed, itunes_ext: &ParsedITunesExtensions) -> Option<Author> {
    // Try standard authors first
    if let Some(person) = feed.authors.first() {
        return Some(person_to_author(person));
    }

    // Fall back to iTunes author
    if let Some(ref author_name) = itunes_ext.feed.author {
        return Some(Author {
            name: Some(author_name.clone()),
            email: None,
            uri: None,
        });
    }

    None
}

/// Extracts feed-level image URL.
/// iTunes image (extension) has highest priority, then feed.logo, then feed.icon.
fn extract_feed_image(feed: &FeedRsFeed, itunes_ext: &ParsedITunesExtensions) -> Option<String> {
    // iTunes image from extension has priority
    if let Some(ref href) = itunes_ext.feed.image_href {
        return Some(href.clone());
    }

    // feed-rs stores iTunes image in feed.logo
    if let Some(ref logo) = feed.logo {
        return Some(logo.uri.clone());
    }

    // Try feed.icon
    if let Some(ref icon) = feed.icon {
        return Some(icon.uri.clone());
    }

    None
}

/// Maps a feed-rs Entry to our FeedItem model.
fn map_entry(
    entry: &Entry,
    feed_type: &str,
    feed_language: Option<&str>,
    item_ext: &ItemITunesExt,
) -> FeedItem {
    let item_url = extract_item_url(entry);

    // Extract summary (raw HTML then stripped plain text)
    let summary_html = entry
        .summary
        .as_ref()
        .map(|t| t.content.clone())
        .unwrap_or_default();
    let summary = strip_html(&summary_html);

    // Extract content (prefer body/src); keep raw for image extraction, store plain text per Go parity
    let content_raw = entry
        .content
        .as_ref()
        .and_then(|c| {
            c.body
                .clone()
                .or_else(|| c.src.as_ref().map(|l| l.href.clone()))
        })
        .unwrap_or_else(|| summary_html.clone());
    let content = strip_html(&content_raw);

    // Extract enclosures from links (rel=enclosure) and media.content, deduplicated
    let enclosures = extract_enclosures(entry);

    // Select primary media URL (audio priority)
    let primary_media_url = select_primary_media(&enclosures);

    // Get duration: prefer our parsed iTunes duration (correct for MM:SS), fall back to feed-rs
    let duration_seconds = extract_duration(entry, item_ext);

    // Get explicit flag from iTunes extension or media.rating
    let explicit_flag = extract_explicit_flag(entry, item_ext);

    // Select image/thumbnail with priority cascade
    let (image_url, thumbnail_url) = select_image_thumbnail(
        entry,
        &enclosures,
        &content_raw,
        &summary_html,
        &item_url,
        item_ext,
    );

    // Extract author (iTunes author if no standard author)
    let author = extract_entry_author(entry, item_ext);

    // Extract categories
    let categories: Vec<String> = entry.categories.iter().map(|c| c.term.clone()).collect();

    // Parse timestamps
    let published_ms = entry
        .published
        .map(|dt| dt.timestamp_millis() as u64)
        .unwrap_or(0);

    let updated_ms = entry
        .updated
        .map(|dt| dt.timestamp_millis() as u64)
        .or_else(|| entry.published.map(|dt| dt.timestamp_millis() as u64))
        .unwrap_or(0);

    // Language: entry language or feed language
    let language = entry
        .language
        .clone()
        .or_else(|| feed_language.map(String::from));

    FeedItem {
        title: entry
            .title
            .as_ref()
            .map(|t| t.content.clone())
            .unwrap_or_default(),
        url: item_url,
        image_url,
        summary,
        content,
        guid: entry.id.clone(),
        language,
        feed_type: feed_type.to_string(),
        published_ms,
        updated_ms,
        author,
        categories,
        enclosures,
        primary_media_url,
        thumbnail_url,
        explicit_flag,
        duration_seconds,
    }
}

/// Extracts enclosures from entry.
/// Per requirements:
/// - Include entry.links where rel=="enclosure"; map url=href, mime_type=media_type, length=length.unwrap_or(0)
/// - Also include media.content entries with url
/// - Deduplicate identical URL/mime pairs (keep first)
fn extract_enclosures(entry: &Entry) -> Vec<Enclosure> {
    let mut enclosures = Vec::new();
    let mut seen: HashSet<(String, Option<String>)> = HashSet::new();

    // First, extract from links with rel="enclosure"
    for link in &entry.links {
        if is_enclosure_link(link) {
            let mime_type = link.media_type.clone();
            let key = (link.href.clone(), mime_type.clone());
            if seen.insert(key) {
                enclosures.push(Enclosure {
                    url: link.href.clone(),
                    mime_type,
                    length: link.length.unwrap_or(0),
                });
            }
        }
    }

    // Also extract from media.content
    for media in &entry.media {
        for content in &media.content {
            if let Some(ref url) = content.url {
                let mime_type = content.content_type.as_ref().map(|m| m.to_string());
                let key = (url.to_string(), mime_type.clone());
                if seen.insert(key) {
                    enclosures.push(Enclosure {
                        url: url.to_string(),
                        mime_type,
                        length: content.size.unwrap_or(0),
                    });
                }
            }
        }
    }

    enclosures
}

/// Selects the primary media URL based on audio priority.
/// Priority: audio/mpeg > audio/mp3 > audio/mp4 > audio/aac > first enclosure
fn select_primary_media(enclosures: &[Enclosure]) -> Option<String> {
    if enclosures.is_empty() {
        return None;
    }

    let priority_order = ["audio/mpeg", "audio/mp3", "audio/mp4", "audio/aac"];

    for priority in &priority_order {
        for enc in enclosures {
            if let Some(ref mime) = enc.mime_type {
                if mime == *priority {
                    return Some(enc.url.clone());
                }
            }
        }
    }

    // Fall back to first enclosure
    Some(enclosures[0].url.clone())
}

/// Extracts duration in seconds.
/// Per requirements: use iTunes extension duration parsed with parse_duration_seconds when media.duration missing.
/// Our iTunes parser correctly handles MM:SS format unlike feed-rs.
fn extract_duration(entry: &Entry, item_ext: &ItemITunesExt) -> u32 {
    // First try our iTunes extension parser (correct for MM:SS)
    if item_ext.duration.is_some() {
        return parse_item_duration(item_ext);
    }

    // Fall back to feed-rs media.duration (but it has MM:SS bug)
    for media in &entry.media {
        if let Some(duration) = media.duration {
            return duration.as_secs() as u32;
        }
    }

    0
}

/// Extracts explicit flag.
/// Per requirements:
/// - true if explicit from extensions is "yes"/"true"/"explicit" (case-insensitive), OR
/// - any media.rating urn=="itunes" with value in {"true","yes","explicit"} (case-insensitive)
fn extract_explicit_flag(entry: &Entry, item_ext: &ItemITunesExt) -> bool {
    // Check iTunes extension
    if is_explicit(item_ext.explicit.as_deref()) {
        return true;
    }

    // Check media.rating
    for media in &entry.media {
        for content in &media.content {
            if let Some(ref rating) = content.rating {
                if rating.urn == "itunes" {
                    let value_lower = rating.value.to_lowercase();
                    if value_lower == "true" || value_lower == "yes" || value_lower == "explicit" {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Selects image and thumbnail URLs with priority cascade.
/// Per requirements, order:
/// (1) iTunes image (entry extension image href) - highest priority
/// (2) First image enclosure (mime starts with image/)
/// (3) media thumbnail (entry.media[].thumbnails.first())
/// (4) first <img> in content HTML (extract_first_image with base=item URL)
/// (5) first <img> in summary HTML
/// Both image_url and thumbnail_url set to the same selected value.
fn select_image_thumbnail(
    entry: &Entry,
    enclosures: &[Enclosure],
    content_html: &str,
    summary_html: &str,
    item_url: &str,
    item_ext: &ItemITunesExt,
) -> (Option<String>, Option<String>) {
    // (1) iTunes image from extension - highest priority
    if let Some(ref href) = item_ext.image_href {
        return (Some(href.clone()), Some(href.clone()));
    }

    // (2) First image enclosure
    for enc in enclosures {
        if let Some(ref mime) = enc.mime_type {
            if mime.starts_with("image/") {
                return (Some(enc.url.clone()), Some(enc.url.clone()));
            }
        }
    }

    // (3) Media thumbnails
    for media in &entry.media {
        if let Some(thumb) = media.thumbnails.first() {
            let url = thumb.image.uri.clone();
            return (Some(url.clone()), Some(url));
        }
    }

    // (4) First <img> from content HTML
    let base_url = if item_url.is_empty() {
        None
    } else {
        Some(item_url)
    };
    if let Some(img_url) = extract_first_image(content_html, base_url) {
        return (Some(img_url.clone()), Some(img_url));
    }

    // (5) First <img> from summary HTML
    if let Some(img_url) = extract_first_image(summary_html, base_url) {
        return (Some(img_url.clone()), Some(img_url));
    }

    (None, None)
}

/// Extracts entry-level author.
/// iTunes author from extension if no standard author or media credit.
fn extract_entry_author(entry: &Entry, item_ext: &ItemITunesExt) -> Option<Author> {
    // Try entry authors first
    if let Some(person) = entry.authors.first() {
        return Some(person_to_author(person));
    }

    // Try media credits
    for media in &entry.media {
        if let Some(credit) = media.credits.first() {
            return Some(Author {
                name: Some(credit.entity.clone()),
                email: None,
                uri: None,
            });
        }
    }

    // Fall back to iTunes author extension
    if let Some(ref author_name) = item_ext.author {
        return Some(Author {
            name: Some(author_name.clone()),
            email: None,
            uri: None,
        });
    }

    None
}

/// Converts a feed-rs Person to our Author model.
fn person_to_author(person: &Person) -> Author {
    Author {
        name: Some(person.name.clone()),
        email: person.email.clone(),
        uri: person.uri.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_feed_type_article() {
        // A plain RSS feed without iTunes or audio enclosures
        let rss = r#"<?xml version="1.0"?>
        <rss version="2.0">
            <channel>
                <title>Test Blog</title>
                <item><title>Article 1</title></item>
                <item><title>Article 2</title></item>
            </channel>
        </rss>"#;

        let parsed = feed_rs::parser::parse(rss.as_bytes()).unwrap();
        let itunes_ext = parse_itunes_extensions(rss.as_bytes());
        let feed_type = detect_feed_type(&parsed, &itunes_ext);
        assert_eq!(feed_type, "article");
    }

    #[test]
    fn test_detect_feed_type_podcast_by_namespace() {
        // RSS feed with iTunes namespace
        let rss = r#"<?xml version="1.0"?>
        <rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
            <channel>
                <title>Test Podcast</title>
                <item><title>Episode 1</title></item>
            </channel>
        </rss>"#;

        let parsed = feed_rs::parser::parse(rss.as_bytes()).unwrap();
        let itunes_ext = parse_itunes_extensions(rss.as_bytes());
        let feed_type = detect_feed_type(&parsed, &itunes_ext);
        assert_eq!(feed_type, "podcast");
    }

    #[test]
    fn test_extract_home_url() {
        let rss = r#"<?xml version="1.0"?>
        <rss version="2.0">
            <channel>
                <title>Test</title>
                <link>https://example.com</link>
            </channel>
        </rss>"#;

        let parsed = feed_rs::parser::parse(rss.as_bytes()).unwrap();
        let home_url = extract_home_url(&parsed.links);
        assert_eq!(home_url, "https://example.com/");
    }

    #[test]
    fn test_select_primary_media_audio_priority() {
        let enclosures = vec![
            Enclosure {
                url: "https://example.com/video.mp4".to_string(),
                mime_type: Some("video/mp4".to_string()),
                length: 1000,
            },
            Enclosure {
                url: "https://example.com/audio.mp3".to_string(),
                mime_type: Some("audio/mpeg".to_string()),
                length: 500,
            },
        ];

        let primary = select_primary_media(&enclosures);
        assert_eq!(primary, Some("https://example.com/audio.mp3".to_string()));
    }

    #[test]
    fn test_duration_parsing_mmss() {
        // Test that our duration parsing correctly handles MM:SS format
        let item_ext = ItemITunesExt {
            duration: Some("45:30".to_string()),
            ..Default::default()
        };
        let entry = Entry::default();
        let duration = extract_duration(&entry, &item_ext);
        assert_eq!(duration, 2730); // 45*60 + 30
    }

    #[test]
    fn test_explicit_flag_from_extension() {
        let item_ext = ItemITunesExt {
            explicit: Some("yes".to_string()),
            ..Default::default()
        };
        let entry = Entry::default();
        assert!(extract_explicit_flag(&entry, &item_ext));

        let item_ext_no = ItemITunesExt {
            explicit: Some("no".to_string()),
            ..Default::default()
        };
        assert!(!extract_explicit_flag(&entry, &item_ext_no));
    }
}
