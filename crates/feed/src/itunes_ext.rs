// ABOUTME: Raw XML parsing for iTunes podcast extensions not exposed by feed-rs.
// ABOUTME: Extracts duration, explicit, image, and author from itunes namespace elements.

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use std::collections::HashMap;

use crate::duration_parse::parse_duration_seconds;

/// iTunes metadata extracted from raw XML at the feed (channel) level.
#[derive(Debug, Default, Clone)]
pub struct FeedITunesExt {
    /// True if feed has itunes namespace declaration.
    pub has_itunes_namespace: bool,
    /// Feed-level itunes:image href attribute.
    pub image_href: Option<String>,
    /// Feed-level itunes:author text content.
    pub author: Option<String>,
    /// Feed-level itunes:explicit text content.
    pub explicit: Option<String>,
}

/// iTunes metadata extracted from raw XML at the item level.
#[derive(Debug, Default, Clone)]
pub struct ItemITunesExt {
    /// Item-level itunes:image href attribute.
    pub image_href: Option<String>,
    /// Item-level itunes:author text content.
    pub author: Option<String>,
    /// Item-level itunes:duration text content (raw string).
    pub duration: Option<String>,
    /// Item-level itunes:explicit text content.
    pub explicit: Option<String>,
}

/// Parsed iTunes extensions for a complete feed.
#[derive(Debug, Default, Clone)]
pub struct ParsedITunesExtensions {
    pub feed: FeedITunesExt,
    /// Map from item guid (or index if no guid) to item extensions.
    pub items: HashMap<String, ItemITunesExt>,
    /// Items by index for fallback lookup.
    pub items_by_index: Vec<ItemITunesExt>,
}

/// Parses iTunes extensions from raw RSS/Atom XML bytes.
/// This extracts data that feed-rs doesn't properly expose.
pub fn parse_itunes_extensions(data: &[u8]) -> ParsedITunesExtensions {
    let mut result = ParsedITunesExtensions::default();
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();

    // Track current position in XML structure
    let mut in_channel = false;
    let mut in_item = false;
    let mut current_item_guid: Option<String> = None;
    let mut current_item_ext = ItemITunesExt::default();
    let mut current_element: Option<String> = None;
    let mut item_index = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local_name = name.split(':').last().unwrap_or(&name);

                // Check for itunes namespace declaration at root
                if name == "rss" || name == "feed" {
                    result.feed.has_itunes_namespace = has_itunes_namespace(e);
                }

                match local_name {
                    "channel" => in_channel = true,
                    "item" | "entry" => {
                        in_item = true;
                        current_item_guid = None;
                        current_item_ext = ItemITunesExt::default();
                    }
                    "guid" | "id" if in_item => {
                        current_element = Some("guid".to_string());
                    }
                    _ => {}
                }

                // Handle itunes:* elements
                if name.starts_with("itunes:") {
                    let itunes_name = &name[7..]; // Strip "itunes:" prefix
                    match itunes_name {
                        "image" => {
                            // itunes:image uses href attribute
                            if let Some(href) = get_attribute(e, "href") {
                                if in_item {
                                    current_item_ext.image_href = Some(href);
                                } else if in_channel {
                                    result.feed.image_href = Some(href);
                                }
                            }
                        }
                        "author" | "duration" | "explicit" => {
                            current_element = Some(itunes_name.to_string());
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if let Some(ref elem) = current_element {
                    let text = e.decode().map(|s| s.into_owned()).unwrap_or_default();
                    if !text.is_empty() {
                        match elem.as_str() {
                            "guid" if in_item => {
                                current_item_guid = Some(text);
                            }
                            "author" => {
                                if in_item {
                                    current_item_ext.author = Some(text);
                                } else if in_channel {
                                    result.feed.author = Some(text);
                                }
                            }
                            "duration" if in_item => {
                                current_item_ext.duration = Some(text);
                            }
                            "explicit" => {
                                if in_item {
                                    current_item_ext.explicit = Some(text);
                                } else if in_channel {
                                    result.feed.explicit = Some(text);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local_name = name.split(':').last().unwrap_or(&name);

                match local_name {
                    "channel" => in_channel = false,
                    "item" | "entry" => {
                        // Store item extensions
                        let key = current_item_guid
                            .clone()
                            .unwrap_or_else(|| format!("__index_{}", item_index));
                        result.items.insert(key, current_item_ext.clone());
                        result.items_by_index.push(current_item_ext.clone());
                        in_item = false;
                        item_index += 1;
                    }
                    _ => {}
                }

                // Clear current element after processing
                if name.starts_with("itunes:") || local_name == "guid" || local_name == "id" {
                    current_element = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    result
}

/// Checks if an RSS/feed element has the iTunes namespace declared.
fn has_itunes_namespace(e: &BytesStart) -> bool {
    for attr in e.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        let value = String::from_utf8_lossy(&attr.value);
        if key.starts_with("xmlns") && value.contains("itunes.com") {
            return true;
        }
    }
    false
}

/// Gets an attribute value from an XML element.
fn get_attribute(e: &BytesStart, name: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if key == name {
            return Some(String::from_utf8_lossy(&attr.value).to_string());
        }
    }
    None
}

/// Parses duration from ItemITunesExt, returning seconds.
/// Falls back to 0 if no duration or parse fails.
pub fn parse_item_duration(ext: &ItemITunesExt) -> u32 {
    ext.duration
        .as_ref()
        .and_then(|d| parse_duration_seconds(d))
        .unwrap_or(0)
}

/// Checks if explicit flag is set based on iTunes extension value.
/// Returns true for case-insensitive: "yes", "true", "explicit".
pub fn is_explicit(value: Option<&str>) -> bool {
    value
        .map(|v| {
            let lower = v.to_lowercase();
            lower == "yes" || lower == "true" || lower == "explicit"
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_itunes_extensions_basic() {
        let rss = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
    <channel>
        <title>Test Podcast</title>
        <itunes:image href="https://podcast/feed-img.jpg"/>
        <itunes:author>Feed Author</itunes:author>
        <itunes:explicit>yes</itunes:explicit>
        <item>
            <guid>ep-1</guid>
            <title>Episode 1</title>
            <itunes:duration>45:30</itunes:duration>
            <itunes:explicit>yes</itunes:explicit>
            <itunes:image href="https://podcast/ep1-img.jpg"/>
            <itunes:author>Episode Author</itunes:author>
        </item>
        <item>
            <guid>ep-2</guid>
            <title>Episode 2</title>
            <itunes:duration>01:02:03</itunes:duration>
            <itunes:explicit>no</itunes:explicit>
        </item>
    </channel>
</rss>"#;

        let ext = parse_itunes_extensions(rss.as_bytes());

        // Feed level
        assert!(ext.feed.has_itunes_namespace);
        assert_eq!(
            ext.feed.image_href,
            Some("https://podcast/feed-img.jpg".to_string())
        );
        assert_eq!(ext.feed.author, Some("Feed Author".to_string()));
        assert_eq!(ext.feed.explicit, Some("yes".to_string()));

        // Item level - by guid
        let item1 = ext.items.get("ep-1").unwrap();
        assert_eq!(item1.duration, Some("45:30".to_string()));
        assert_eq!(item1.explicit, Some("yes".to_string()));
        assert_eq!(
            item1.image_href,
            Some("https://podcast/ep1-img.jpg".to_string())
        );
        assert_eq!(item1.author, Some("Episode Author".to_string()));

        let item2 = ext.items.get("ep-2").unwrap();
        assert_eq!(item2.duration, Some("01:02:03".to_string()));
        assert_eq!(item2.explicit, Some("no".to_string()));

        // Duration parsing
        assert_eq!(parse_item_duration(item1), 2730); // 45*60 + 30
        assert_eq!(parse_item_duration(item2), 3723); // 1*3600 + 2*60 + 3
    }

    #[test]
    fn test_is_explicit() {
        assert!(is_explicit(Some("yes")));
        assert!(is_explicit(Some("YES")));
        assert!(is_explicit(Some("true")));
        assert!(is_explicit(Some("TRUE")));
        assert!(is_explicit(Some("explicit")));
        assert!(is_explicit(Some("EXPLICIT")));
        assert!(!is_explicit(Some("no")));
        assert!(!is_explicit(Some("false")));
        assert!(!is_explicit(Some("clean")));
        assert!(!is_explicit(None));
    }

    #[test]
    fn test_no_itunes_namespace() {
        let rss = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
    <channel>
        <title>Article Blog</title>
        <item>
            <guid>art-1</guid>
            <title>Article 1</title>
        </item>
    </channel>
</rss>"#;

        let ext = parse_itunes_extensions(rss.as_bytes());
        assert!(!ext.feed.has_itunes_namespace);
        assert!(ext.feed.image_href.is_none());
        assert!(ext.feed.author.is_none());
    }

    #[test]
    fn test_items_by_index() {
        let rss = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
    <channel>
        <item>
            <title>Episode 1</title>
            <itunes:duration>10:00</itunes:duration>
        </item>
        <item>
            <title>Episode 2</title>
            <itunes:duration>20:00</itunes:duration>
        </item>
    </channel>
</rss>"#;

        let ext = parse_itunes_extensions(rss.as_bytes());
        assert_eq!(ext.items_by_index.len(), 2);
        assert_eq!(ext.items_by_index[0].duration, Some("10:00".to_string()));
        assert_eq!(ext.items_by_index[1].duration, Some("20:00".to_string()));
    }
}
