// ABOUTME: Integration tests for the synchronous reader adapters and metadata extractor.
// ABOUTME: Tests extract_reader_sync and extract_metadata_only.

use digests_hermes::{extract_metadata_only, extract_reader_sync};

#[test]
fn test_extract_reader_sync_basic() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>T</title>
        </head>
        <body>
            <p>Hello</p>
        </body>
        </html>
    "#;

    let result = extract_reader_sync("https://example.com/page", html);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

    let rr = result.unwrap();
    assert_eq!(rr.title, "T", "Title should be 'T'");
    assert!(
        rr.content.contains("Hello"),
        "Content should contain 'Hello', got: {}",
        rr.content
    );
    assert_eq!(rr.domain, "example.com", "Domain should be example.com");
}

#[test]
fn test_extract_reader_sync_published() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Article with Date</title>
            <meta property="article:published_time" content="2020-01-02T03:04:05Z">
        </head>
        <body>
            <article>
                <p>Some article content here for the reader to extract.</p>
            </article>
        </body>
        </html>
    "#;

    let result = extract_reader_sync("https://example.com/article", html);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

    let rr = result.unwrap();
    assert!(
        rr.published_ms > 0,
        "published_ms should be > 0, got: {}",
        rr.published_ms
    );
    // 2020-01-02T03:04:05Z in milliseconds
    // Unix timestamp: 1577934245 seconds = 1577934245000 ms
    assert!(
        rr.published_ms >= 1577934245000,
        "published_ms should be at least 2020-01-02, got: {}",
        rr.published_ms
    );
}

#[test]
fn test_extract_metadata_only() {
    let html = r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <title>Test Page</title>
            <meta property="og:title" content="OG Title">
            <meta property="og:description" content="OG Description">
            <meta property="og:image" content="/images/hero.jpg">
            <link rel="icon" href="favicon.ico">
        </head>
        <body></body>
        </html>
    "#;

    // Use trailing slash so relative URLs resolve against the directory
    let result = extract_metadata_only(html, "https://example.com/post/");
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

    let meta = result.unwrap();

    // Check that image is resolved to absolute URL (absolute path /images/...)
    assert_eq!(
        meta.image_url, "https://example.com/images/hero.jpg",
        "Image URL should be absolute"
    );

    // Check that icon is resolved to absolute URL (relative to base directory)
    assert_eq!(
        meta.icon_url, "https://example.com/post/favicon.ico",
        "Icon URL should be absolute"
    );

    // Check language
    assert_eq!(meta.language, "en", "Language should be 'en'");

    // Verify other fields
    assert_eq!(meta.title, "OG Title");
    assert_eq!(meta.description, "OG Description");
}

#[test]
fn test_extract_metadata_only_relative_nested() {
    let html = r#"
        <!DOCTYPE html>
        <html lang="fr-CA">
        <head>
            <meta property="og:image" content="../assets/img.png">
            <link rel="icon" href="../../icons/fav.ico">
        </head>
        <body></body>
        </html>
    "#;

    let result = extract_metadata_only(html, "https://example.com/blog/posts/article/");
    assert!(result.is_ok());

    let meta = result.unwrap();
    assert_eq!(
        meta.image_url, "https://example.com/blog/posts/assets/img.png",
        "Relative image URL should resolve correctly"
    );
    assert_eq!(
        meta.icon_url, "https://example.com/blog/icons/fav.ico",
        "Relative icon URL should resolve correctly"
    );
    assert_eq!(
        meta.language, "fr",
        "Language should normalize to primary tag"
    );
}

#[test]
fn test_extract_reader_sync_with_more_content() {
    let html = r##"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <title>Full Article</title>
            <meta property="og:site_name" content="Test Site">
            <meta name="theme-color" content="#123456">
            <link rel="icon" href="/favicon.png">
        </head>
        <body>
            <article>
                <h1>Full Article</h1>
                <p>This is a longer article with more substantial content.</p>
                <p>It has multiple paragraphs to ensure proper extraction.</p>
                <p>The reader should capture all of this text content.</p>
            </article>
        </body>
        </html>
    "##;

    let result = extract_reader_sync("https://testsite.org/articles/full", html);
    assert!(result.is_ok());

    let rr = result.unwrap();
    assert_eq!(rr.title, "Full Article");
    assert_eq!(rr.domain, "testsite.org");
    assert_eq!(rr.site_name, "Test Site");
    assert_eq!(rr.theme_color, "#123456");
    assert!(rr.word_count > 0, "Should have word count > 0");
}
