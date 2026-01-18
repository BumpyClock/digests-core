# Hermes Article Extraction

The `hermes` crate provides robust article extraction from HTML pages, ported from the original Hermes algorithm.

## Features

- **High accuracy** extraction of main article content
- **Metadata extraction** (title, author, date, etc.)
- **Ad and navigation removal**
- **Confidence scoring** for extraction quality
- **Multi-language support**
- **Performance optimized** with heuristics

## Quick Start

```rust
use digests_hermes::{extract_reader_view, extract_metadata, ReaderViewOptions};

// Extract both content and metadata
let (reader_view, metadata) = extract_reader_view(
    "https://example.com/article",
    &html_content,
)?;

// Or extract just the metadata
let metadata = extract_metadata("https://example.com/article", &html_content)?;

println!("Title: {}", metadata.title);
println!("Author: {:?}", metadata.author);
println!("Published: {:?}", metadata.published_date);
println!("Excerpt: {}", metadata.excerpt);
```

## Data Structures

### DReaderView
The extracted article content:

```rust
pub struct DReaderView {
    pub title: String,
    pub content: String,        // Cleaned article body
    pub length: usize,          // Content length in characters
    pub excerpt: String,        // Brief summary
    pub site_name: Option<String>, // Site/publisher name
    pub author: Option<String>,
    pub published_date: Option<String>,
    pub language: Option<String>,
    pub reading_time: Option<usize>, // Estimated minutes to read
    pub confidence: f32,       // Extraction confidence (0.0-1.0)
}
```

### DMetadata
Structured article metadata:

```rust
pub struct DMetadata {
    pub title: String,
    pub author: Option<String>,
    pub published_date: Option<String>,
    pub excerpt: String,
    pub language: Option<String>,
    pub keywords: Vec<String>,
    pub description: Option<String>,
    pub site_name: Option<String>,
    pub url: String,
    pub image_url: Option<String>,
    pub favicon_url: Option<String>,
    pub canonical_url: Option<String>,
    pub open_graph_data: Option<BTreeMap<String, String>>,
    pub twitter_card_data: Option<BTreeMap<String, String>>,
}
```

## Extraction Options

Configure extraction behavior:

```rust
let options = ReaderViewOptions {
    max_content_length: 50000,    // Max content to extract
    min_content_length: 500,     // Minimum content length
    preserve_links: true,        // Keep links in extracted content
    preserve_images: true,      // Keep images
    extract_metadata: true,      // Extract metadata
    timeout_ms: 5000,           // Extraction timeout
};

let (reader_view, _) = extract_reader_view_with_options(
    "https://example.com",
    &html_content,
    options,
)?;
```

## Best Practices

### 1. Always Check Confidence
```rust
let (reader_view, _) = extract_reader_view(url, html)?;

if reader_view.confidence < 0.7 {
    println!("Low confidence extraction: {:.2}", reader_view.confidence);
    // Handle low quality extraction
}
```

### 2. Handle Missing Metadata
```rust
let (_, metadata) = extract_reader_view(url, html)?;

match metadata.author {
    Some(author) => println!("By {}", author),
    None => println!("Author not found"),
}
```

### 3. Use Appropriate Timeouts
```rust
let options = ReaderViewOptions {
    timeout_ms: 3000, // 3 second timeout for fast sites
    // ... other options
};
```

## Advanced Usage

### Custom Extraction
```rust
use digests_hermes::{ReaderExtractor, ExtractionConfig};

let extractor = ReaderExtractor::new(ExtractionConfig {
    min_paragraphs: 3,
    max_density: 1.5,
    // ... custom heuristics
});

let reader_view = extractor.extract(url, html)?;
```

### Batch Processing
```rust
use digests_hermes::extract_multiple;

let urls = vec![
    "https://example.com/article1",
    "https://example.com/article2",
    "https://example.com/article3",
];

let results: Vec<(String, Result<DReaderView, _>)> = urls
    .into_iter()
    .map(|url| {
        let html = fetch_html(url)?;
        let view = extract_reader_view(url, &html)?;
        Ok((url.to_string(), view))
    })
    .collect();
```

### Metadata-Only Extraction
For when you only need metadata and not full content:

```rust
let metadata = extract_metadata(url, html)?;

// Extract structured data from OpenGraph/Twitter cards
if let Some(og) = &metadata.open_graph_data {
    if let Some(title) = og.get("og:title") {
        println!("OG Title: {}", title);
    }
}
```

## Performance Optimization

### 1. Pre-filter HTML
```rust
// Remove known non-content areas before extraction
let cleaned_html = remove_script_tags(&html);
let cleaned_html = remove_ads(&cleaned_html);

let reader_view = extract_reader_view(url, &cleaned_html)?;
```

### 2. Limit Input Size
```rust
// For very large pages, limit input
let max_input = 1024 * 1024; // 1MB
let html_to_process = if html.len() > max_input {
    &html[..max_input]
} else {
    &html
};
```

### 3. Parallel Processing
```rust
use rayon::prelude::*;

let results: Vec<_> = urls
    .par_iter()
    .map(|url| {
        let html = fetch_html(url)?;
        extract_reader_view(url, &html)
    })
    .collect();
```

## Error Handling

### Common Error Types
```rust
use digests_hermes::ExtractionError;

match extract_reader_view(url, html) {
    Ok((view, _)) => println!("Extracted {} chars", view.length),
    Err(ExtractionError::InvalidHtml) => {
        eprintln!("Invalid HTML provided");
    }
    Err(ExtractionError::Timeout) => {
        eprintln!("Extraction timed out");
    }
    Err(ExtractionError::NoContentFound) => {
        eprintln!("No article content found");
    }
    Err(e) => eprintln!("Extraction error: {}", e),
}
```

## Quality Metrics

### Confidence Scoring
The extraction confidence is based on several factors:

- **Content density**: Amount of actual content vs HTML
- **Structure**: Proper article structure (headings, paragraphs)
- **Length**: Sufficient content length
- **Quality**: Clean text vs ads/navigation

### Debug Mode
Enable debug output to see extraction decisions:

```rust
std::env::set_var("DIGESTS_HERMES_DEBUG", "1");
let (view, _) = extract_reader_view(url, html)?;
println!("Confidence: {:.2}", view.confidence);
println!("Content length: {}", view.length);
```

## Testing

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_article() {
        let html = r#"
        <html>
            <head><title>Test Article</title></head>
            <body>
                <article>
                    <h1>Main Title</h1>
                    <p>This is the article content.</p>
                </article>
            </body>
        </html>
        "#;

        let (view, _) = extract_reader_view("https://example.com", html).unwrap();
        assert_eq!(view.title, "Test Article");
        assert!(view.content.contains("article content"));
    }
}
```

### Integration Tests
```rust
#[test]
fn test_real_site_extraction() {
    let response = reqwest::get("https://example.com/article").unwrap();
    let html = response.text().unwrap();

    let (view, _) = extract_reader_view("https://example.com/article", &html).unwrap();
    assert!(view.confidence > 0.5);
    assert!(view.length > 100);
}
```

## Troubleshooting

### Common Issues

1. **Low confidence**: Try removing ads/ads scripts before extraction
2. **Missing content**: Check if content is loaded dynamically (JavaScript)
3. **Wrong language**: Set language explicitly if auto-detection fails
4. **Timeout**: Increase timeout for complex pages

### Extraction Tips

1. **Pre-process HTML**: Remove known bad elements (ads, navigation)
2. **Handle dynamic content**: For JavaScript-heavy sites, consider using headless browsers
3. **Post-process**: Clean extracted content further if needed
4. **Cache results**: Avoid re-extracting the same URL repeatedly

### Configuration Reference

| Option | Default | Description |
|--------|---------|-------------|
| `max_content_length` | 50000 | Maximum content to extract (chars) |
| `min_content_length` | 500 | Minimum content threshold |
| `preserve_links` | true | Keep links in extracted content |
| `preserve_images` | true | Keep images in content |
| `extract_metadata` | true | Extract structured metadata |
| `timeout_ms` | 5000 | Extraction timeout in milliseconds |