// ABOUTME: Integration tests for feed parsing functionality.
// ABOUTME: Tests article/podcast detection, iTunes metadata extraction, and time format coverage.

use digests_feed::parse_feed_bytes;

/// Tests basic article feed parsing (RSS without iTunes/audio).
/// Per requirements:
/// - inline RSS without iTunes, two items
/// - first item content contains <img src="/img/a.jpg">, base link https://example.com/post1
/// - Assert feed.feed_type == "article"
/// - item[0].thumbnail_url resolved to https://example.com/img/a.jpg
/// - duration_seconds == 0
/// - explicit_flag == false
#[test]
fn test_article_feed_basic() {
    let rss = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
    <channel>
        <title>Tech Blog</title>
        <link>https://example.com</link>
        <description>A tech blog about programming</description>
        <item>
            <title>First Article</title>
            <link>https://example.com/post1</link>
            <guid>article-1</guid>
            <pubDate>Mon, 15 Jan 2024 10:00:00 +0000</pubDate>
            <description>This is a summary of the first article.</description>
            <content:encoded xmlns:content="http://purl.org/rss/1.0/modules/content/">
                <![CDATA[
                <p>This is the full content of the article.</p>
                <img src="/img/a.jpg" alt="Article image">
                <p>More content here.</p>
                ]]>
            </content:encoded>
        </item>
        <item>
            <title>Second Article</title>
            <link>https://example.com/post2</link>
            <guid>article-2</guid>
            <pubDate>Tue, 16 Jan 2024 11:00:00 +0000</pubDate>
            <description>Summary of the second article.</description>
        </item>
    </channel>
</rss>"#;

    let feed = parse_feed_bytes(rss.as_bytes(), "https://example.com/feed.xml").unwrap();

    // Feed type detection: no iTunes, no audio enclosures -> article
    assert_eq!(feed.feed_type, "article", "feed_type should be 'article'");

    // Check first item
    assert!(feed.items.len() >= 2, "should have at least 2 items");
    let item = &feed.items[0];

    // Thumbnail should be resolved from content <img src="/img/a.jpg"> with base https://example.com/post1
    assert_eq!(
        item.thumbnail_url,
        Some("https://example.com/img/a.jpg".to_string()),
        "thumbnail_url should be resolved to https://example.com/img/a.jpg"
    );

    // No podcast metadata
    assert_eq!(
        item.duration_seconds, 0,
        "article should have duration_seconds == 0"
    );
    assert!(
        !item.explicit_flag,
        "article should have explicit_flag == false"
    );
}

/// Tests podcast feed parsing with iTunes tags and audio enclosures.
/// Per requirements:
/// - inline RSS with itunes namespace
/// - channel itunes:image href="https://podcast/img.jpg"
/// - item has enclosure url="https://cdn/show.mp3" type="audio/mpeg" length="12345"
/// - item has itunes:duration "01:02:03" and itunes:explicit "yes" and itunes:image href="https://cdn/episode.jpg"
/// - Assert feed.feed_type == "podcast"
/// - feed.image_url == Some("https://podcast/img.jpg")
/// - item.primary_media_url == Some("https://cdn/show.mp3")
/// - item.duration_seconds == 3723
/// - item.explicit_flag == true
/// - item.thumbnail_url == Some("https://cdn/episode.jpg")
#[test]
fn test_podcast_feed_basic() {
    let rss = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
    <channel>
        <title>Tech Podcast</title>
        <link>https://podcast.example.com</link>
        <description>A podcast about technology</description>
        <itunes:image href="https://podcast/img.jpg"/>
        <item>
            <title>Episode 1</title>
            <link>https://podcast.example.com/ep1</link>
            <guid>episode-1</guid>
            <pubDate>Mon, 15 Jan 2024 10:00:00 +0000</pubDate>
            <description>Welcome to the podcast!</description>
            <enclosure url="https://cdn/show.mp3" type="audio/mpeg" length="12345"/>
            <itunes:duration>01:02:03</itunes:duration>
            <itunes:explicit>yes</itunes:explicit>
            <itunes:image href="https://cdn/episode.jpg"/>
        </item>
    </channel>
</rss>"#;

    let feed = parse_feed_bytes(rss.as_bytes(), "https://podcast.example.com/feed.xml").unwrap();

    // Feed type detection: has iTunes namespace -> podcast
    assert_eq!(feed.feed_type, "podcast", "feed_type should be 'podcast'");

    // Feed-level image from iTunes
    assert_eq!(
        feed.image_url,
        Some("https://podcast/img.jpg".to_string()),
        "feed.image_url should be https://podcast/img.jpg"
    );

    // Check episode
    assert!(!feed.items.is_empty(), "should have at least 1 item");
    let item = &feed.items[0];

    // Primary media URL should be the mp3
    assert_eq!(
        item.primary_media_url,
        Some("https://cdn/show.mp3".to_string()),
        "primary_media_url should be https://cdn/show.mp3"
    );

    // Duration parsed from "01:02:03" = 1*3600 + 2*60 + 3 = 3723
    assert_eq!(
        item.duration_seconds, 3723,
        "duration_seconds should be 3723 (parsed from 01:02:03)"
    );

    // Explicit flag from itunes:explicit "yes"
    assert!(
        item.explicit_flag,
        "explicit_flag should be true (itunes:explicit is 'yes')"
    );

    // Thumbnail from iTunes image on entry
    assert_eq!(
        item.thumbnail_url,
        Some("https://cdn/episode.jpg".to_string()),
        "thumbnail_url should be https://cdn/episode.jpg"
    );
}

/// Tests that pubDate with named timezone (MST) is parsed correctly.
/// Per requirements:
/// - feed pubDate "Mon, 02 Jan 2006 15:04:05 MST" parsed
/// - feed.published_ms != 0 (uses parse_flexible_time formats)
/// - Use minimal RSS string; ensure parser doesn't panic
#[test]
fn test_time_format_mst() {
    let rss = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
    <channel>
        <title>Time Test Feed</title>
        <link>https://example.com</link>
        <item>
            <title>Test Item</title>
            <link>https://example.com/item</link>
            <guid>test-item</guid>
            <pubDate>Mon, 02 Jan 2006 15:04:05 MST</pubDate>
            <description>Testing named timezone parsing.</description>
        </item>
    </channel>
</rss>"#;

    let feed = parse_feed_bytes(rss.as_bytes(), "https://example.com/feed.xml").unwrap();

    assert!(!feed.items.is_empty(), "should have at least 1 item");
    let item = &feed.items[0];

    // published_ms should be nonzero if the MST timezone was parsed
    assert!(
        item.published_ms != 0,
        "published_ms should be nonzero when parsing 'Mon, 02 Jan 2006 15:04:05 MST'"
    );
}
