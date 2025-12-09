// ABOUTME: Synchronous adapter for the async Hermes reader API.
// ABOUTME: Provides blocking extract_reader_sync for content extraction.

use tokio::runtime::Runtime;

use crate::error::ParseError;
use crate::options::ContentType;
use crate::reader_result::ReaderResult;
use crate::Client;

/// Extract reader content synchronously by spinning up a local tokio runtime.
///
/// # Arguments
/// * `url` - The URL of the page (used for domain extraction and relative URL resolution)
/// * `html` - The raw HTML content to parse
///
/// # Returns
/// A `ReaderResult` with extracted article data, or a `ParseError` on failure.
pub fn extract_reader_sync(url: &str, html: &str) -> Result<ReaderResult, ParseError> {
    let rt = Runtime::new().map_err(|e| {
        ParseError::extract(
            url,
            "extract_reader_sync",
            Some(anyhow::anyhow!("Failed to create runtime: {}", e)),
        )
    })?;

    let client = Client::builder().content_type(ContentType::Html).build();
    let url_owned = url.to_string();
    let html_owned = html.to_string();

    let result = rt.block_on(async { client.parse_html(&html_owned, &url_owned).await })?;

    Ok(ReaderResult::from_parse_result(&result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_reader_sync_simple() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test Page</title></head>
            <body><p>Hello World content here.</p></body>
            </html>
        "#;

        let result = extract_reader_sync("https://example.com/page", html);
        assert!(result.is_ok());
        let rr = result.unwrap();
        assert_eq!(rr.title, "Test Page");
        assert_eq!(rr.domain, "example.com");
    }
}
