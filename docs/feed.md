# Feed Parsing

The `feed` crate provides robust parsing for RSS, Atom, and podcast feeds.

## Features

- **RSS 0.91, 0.92, 2.0** full support
- **Atom 1.0** complete implementation
- **Podcast extensions** (RSS with iTunes namespace)
- **RSS Media** namespace support
- **Robust error handling** for malformed feeds
- **URL normalization** and validation
- **Lazy parsing** for performance

## Quick Start

```rust
use digests_feed::{parse_feed, DFeed};
use std::fs;

// Parse from string
let feed_content = fs::read_to_string("feed.xml")?;
let feed = parse_feed(&feed_content)?;

// Access feed data
println!("Title: {}", feed.title);
println!("Link: {}", feed.link);
println!("Description: {}", feed.description);

// Iterate over items
for item in &feed.items {
    println!("- {}", item.title);
}
```

## Data Structures

### DFeed
```rust
pub struct DFeed {
    pub title: String,
    pub link: String,
    pub description: String,
    pub language: Option<String>,
    pub last_build_date: Option<String>,
    pub items: Vec<DItem>,
    // ... other fields
}
```

### DItem
```rust
pub struct DItem {
    pub title: String,
    pub link: Option<String>,
    pub description: Option<String>,
    pub content: Option<String>,
    pub author: Option<String>,
    pub pub_date: Option<String>,
    pub guid: Option<String>,
    pub categories: Vec<String>,
    pub enclosure: Option<DEnclosure>,
    // ... other fields
}
```

### DEnclosure
For media attachments (audio, video, etc.):
```rust
pub struct DEnclosure {
    pub url: String,
    pub mime_type: String,
    pub length: Option<u64>,
}
```

## Supported Feed Formats

### RSS 2.0
```xml
<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Example Feed</title>
    <link>https://example.com</link>
    <description>Sample RSS feed</description>

    <item>
      <title>First Post</title>
      <link>https://example.com/post1</link>
      <description>Content summary</description>
      <pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate>
      <guid>https://example.com/post1</guid>
    </item>
  </channel>
</rss>
```

### Atom 1.0
```xml
<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Example Atom Feed</title>
  <link href="https://example.com"/>
  <updated>2024-01-01T00:00:00Z</updated>

  <entry>
    <title>First Post</title>
    <link href="https://example.com/post1"/>
    <id>urn:uuid:12345</id>
    <updated>2024-01-01T00:00:00Z</updated>
    <summary>Content summary</summary>
  </entry>
</feed>
```

### Podcast RSS (iTunes extensions)
```xml
<channel>
  <title>Example Podcast</title>
  <itunes:author>Podcast Host</itunes:author>
  <itunes:summary>This is a podcast</itunes:summary>
  <itunes:image href="https://example.com/artwork.jpg"/>

  <item>
    <title>Episode 1</title>
    <itunes:author>Guest Name</itunes:author>
    <itunes:duration>45:30</itunes:duration>
    <enclosure url="https://example.com/episode1.mp3" type="audio/mpeg" length="12345678"/>
  </item>
</channel>
```

## Error Handling

The parser returns `Result<DFeed, ParseError>` where `ParseError` can be:

```rust
pub enum ParseError {
    XmlError(String),
    InvalidFeedFormat(String),
    MissingRequiredField(String),
    InvalidUrl(String),
    // ... other error types
}
```

### Common Error Patterns

#### Malformed XML
```rust
match parse_feed(feed_content) {
    Err(ParseError::XmlError(msg)) => {
        eprintln!("XML parsing failed: {}", msg);
        // Handle malformed XML
    }
    Err(ParseError::InvalidFeedFormat(msg)) => {
        eprintln!("Invalid feed format: {}", msg);
        // Handle unsupported feed type
    }
    Ok(feed) => {
        // Process successful parse
    }
}
```

#### Missing Required Fields
```rust
let feed = parse_feed(feed_content)?;
if feed.title.is_empty() {
    return Err(ParseError::MissingRequiredField("title".into()));
}
```

## Advanced Usage

### Custom URL Validation
```rust
use digests_feed::{parse_feed, validate_url};

fn parse_with_custom_validation(content: &str) -> Result<DFeed, ParseError> {
    let feed = parse_feed(content)?;

    // Validate all URLs in the feed
    for item in &feed.items {
        if let Some(ref link) = item.link {
            if !validate_url(link) {
                return Err(ParseError::InvalidUrl(link.clone()));
            }
        }
    }

    Ok(feed)
}
```

### Feed Type Detection
```rust
use digests_feed::{detect_feed_type, FeedType};

let feed_type = detect_feed_type(feed_content);
match feed_type {
    FeedType::RSS => println!("RSS feed detected"),
    FeedType::Atom => println!("Atom feed detected"),
    FeedType::Unknown => println!("Unknown feed type"),
}
```

### Lazy Parsing
For large feeds, use lazy parsing to process items one by one:

```rust
use digests_feed::parse_feed_lazy;

let feed = parse_feed_lazy(feed_content)?;
for item in feed.items {
    // Process each item without loading everything into memory
    process_item(item);
}
```

## Performance Considerations

1. **Memory Usage**: The parser loads the entire feed into memory. For very large feeds, consider streaming parsers.

2. **Validation**: URL validation is performed by default. Disable for performance if not needed:
   ```rust
   // Set environment variable before parsing
   std::env::set_var("DIGESTS_FEED_VALIDATE_URLS", "false");
   let feed = parse_feed(content)?;
   ```

3. **Caching**: Consider caching parsed feeds for frequently updated sources.

## Podcast-Specific Features

The parser handles podcast extensions automatically:

```rust
// Check for podcast-specific fields
if let Some(ref image) = feed.itunes_image {
    println!("Podcast artwork: {}", image);
}

for item in &feed.items {
    if let Some(ref duration) = item.itunes_duration {
        println!("Episode duration: {}", duration);
    }
    if let Some(ref enclosure) = item.enclosure {
        println!("Media file: {} ({})", enclosure.url, enclosure.mime_type);
    }
}
```

## Testing

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_rss_feed() {
        let content = fs::read_to_string("tests/fixtures/sample_rss.xml").unwrap();
        let feed = parse_feed(&content).unwrap();
        assert_eq!(feed.title, "Example RSS Feed");
    }
}
```

### Integration Tests
```rust
#[test]
fn test_real_feed_parsing() {
    let response = reqwest::get("https://example.com/feed.xml").unwrap();
    let content = response.text().unwrap();
    let feed = parse_feed(&content).unwrap();
    assert!(!feed.items.is_empty());
}
```

## Troubleshooting

### Common Issues

1. **CDATA sections**: Automatically handled by the XML parser
2. **HTML content in descriptions**: Preserved as-is
3. **Relative URLs**: Not automatically resolved (handled by caller)
4. **Date formats**: Flexible parsing accepts most common formats

### Debug Mode
Enable debug output:
```rust
std::env::set_var("DIGESTS_FEED_DEBUG", "1");
let feed = parse_feed(content)?;
```