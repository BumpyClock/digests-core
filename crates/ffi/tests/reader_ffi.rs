// ABOUTME: Integration tests for the Hermes FFI reader and metadata functions.
// ABOUTME: Tests the C ABI functions for extracting reader views and metadata.

use std::ptr;
use std::slice;
use std::str;

use digests_ffi::{
    digests_extract_metadata, digests_extract_reader, digests_free_metadata, digests_free_reader,
    digests_metadata_result, digests_reader_result, DError, DErrorCode, DString,
};

/// Helper to convert a DString to a &str for assertions.
unsafe fn dstring_to_str(ds: &DString) -> &str {
    if ds.data.is_null() || ds.len == 0 {
        return "";
    }
    let slice = slice::from_raw_parts(ds.data, ds.len);
    str::from_utf8(slice).unwrap_or("")
}

#[test]
fn test_reader_success() {
    let html = r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <title>Test Article Title</title>
            <meta name="author" content="John Doe">
            <meta property="og:site_name" content="Test Site">
        </head>
        <body>
            <article>
                <h1>Test Article Title</h1>
                <p>This is the first paragraph of the article content.</p>
                <p>This is the second paragraph with more text to ensure we have some content.</p>
                <p>And a third paragraph to make sure the word count is reasonable.</p>
            </article>
        </body>
        </html>
    "#;
    let url = "https://example.com/article";

    unsafe {
        let mut err = DError {
            code: DErrorCode::Internal as u32,
            message: DString::empty(),
        };

        let arena =
            digests_extract_reader(url.as_ptr(), url.len(), html.as_ptr(), html.len(), &mut err);

        assert!(!arena.is_null(), "arena should not be null on success");
        assert_eq!(err.code, DErrorCode::Ok as u32, "error code should be OK");

        let view = digests_reader_result(arena);
        assert!(!view.is_null(), "view should not be null");

        let title = dstring_to_str(&(*view).title);
        assert!(!title.is_empty(), "title should not be empty");
        assert!(
            title.contains("Test Article Title"),
            "title should contain expected text"
        );

        let domain = dstring_to_str(&(*view).domain);
        assert_eq!(domain, "example.com", "domain should be example.com");

        let view_url = dstring_to_str(&(*view).url);
        assert!(
            view_url.contains("example.com"),
            "url should contain example.com"
        );

        // Content should have some text
        let content = dstring_to_str(&(*view).content);
        assert!(!content.is_empty(), "content should not be empty");

        digests_free_reader(arena);
    }
}

#[test]
fn test_reader_invalid_null_html() {
    let url = "https://example.com/test";

    unsafe {
        let mut err = DError {
            code: DErrorCode::Ok as u32,
            message: DString::empty(),
        };

        // Pass null HTML pointer
        let arena = digests_extract_reader(url.as_ptr(), url.len(), ptr::null(), 0, &mut err);

        assert!(arena.is_null(), "arena should be null on invalid input");
        assert_eq!(
            err.code,
            DErrorCode::Invalid as u32,
            "error code should be Invalid"
        );
    }
}

#[test]
fn test_reader_invalid_null_url() {
    let html = "<html><body>test</body></html>";

    unsafe {
        let mut err = DError {
            code: DErrorCode::Ok as u32,
            message: DString::empty(),
        };

        // Pass null URL pointer
        let arena = digests_extract_reader(ptr::null(), 0, html.as_ptr(), html.len(), &mut err);

        assert!(arena.is_null(), "arena should be null on invalid input");
        assert_eq!(
            err.code,
            DErrorCode::Invalid as u32,
            "error code should be Invalid"
        );
    }
}

#[test]
fn test_reader_empty_html() {
    let url = "https://example.com/test";
    let html = "";

    unsafe {
        let mut err = DError {
            code: DErrorCode::Ok as u32,
            message: DString::empty(),
        };

        let arena =
            digests_extract_reader(url.as_ptr(), url.len(), html.as_ptr(), html.len(), &mut err);

        assert!(arena.is_null(), "arena should be null on empty html");
        assert_eq!(
            err.code,
            DErrorCode::Invalid as u32,
            "error code should be Invalid for empty input"
        );
    }
}

#[test]
fn test_metadata_success() {
    let html = r##"
        <!DOCTYPE html>
        <html lang="en-US">
        <head>
            <title>Page Title</title>
            <meta property="og:title" content="OG Title">
            <meta property="og:description" content="OG Description">
            <meta property="og:site_name" content="Test Site">
            <meta property="og:type" content="article">
            <meta property="og:url" content="https://example.com/article">
            <meta property="og:image" content="/images/hero.jpg">
            <meta property="og:image:alt" content="Hero image">
            <meta name="theme-color" content="#ff0000">
            <link rel="icon" href="/favicon.ico">
        </head>
        <body></body>
        </html>
    "##;
    let base_url = "https://example.com/post";

    unsafe {
        let mut err = DError {
            code: DErrorCode::Internal as u32,
            message: DString::empty(),
        };

        let arena = digests_extract_metadata(
            html.as_ptr(),
            html.len(),
            base_url.as_ptr(),
            base_url.len(),
            &mut err,
        );

        assert!(!arena.is_null(), "arena should not be null on success");
        assert_eq!(err.code, DErrorCode::Ok as u32, "error code should be OK");

        let meta = digests_metadata_result(arena);
        assert!(!meta.is_null(), "metadata should not be null");

        let title = dstring_to_str(&(*meta).title);
        assert_eq!(title, "OG Title", "title should be OG Title");

        let description = dstring_to_str(&(*meta).description);
        assert_eq!(
            description, "OG Description",
            "description should be OG Description"
        );

        let site_name = dstring_to_str(&(*meta).site_name);
        assert_eq!(site_name, "Test Site", "site_name should be Test Site");

        let og_type = dstring_to_str(&(*meta).og_type);
        assert_eq!(og_type, "article", "og_type should be article");

        // Image URL should be resolved to absolute
        let image_url = dstring_to_str(&(*meta).image_url);
        assert_eq!(
            image_url, "https://example.com/images/hero.jpg",
            "image_url should be resolved to absolute"
        );

        let image_alt = dstring_to_str(&(*meta).image_alt);
        assert_eq!(image_alt, "Hero image", "image_alt should be Hero image");

        // Icon URL should be resolved
        let icon_url = dstring_to_str(&(*meta).icon_url);
        assert_eq!(
            icon_url, "https://example.com/favicon.ico",
            "icon_url should be resolved"
        );

        let theme_color = dstring_to_str(&(*meta).theme_color);
        assert_eq!(theme_color, "#ff0000", "theme_color should be #ff0000");

        let language = dstring_to_str(&(*meta).language);
        assert_eq!(language, "en", "language should be normalized to 'en'");

        digests_free_metadata(arena);
    }
}

#[test]
fn test_metadata_invalid_null_html() {
    let base_url = "https://example.com/";

    unsafe {
        let mut err = DError {
            code: DErrorCode::Ok as u32,
            message: DString::empty(),
        };

        let arena =
            digests_extract_metadata(ptr::null(), 0, base_url.as_ptr(), base_url.len(), &mut err);

        assert!(arena.is_null(), "arena should be null on invalid input");
        assert_eq!(
            err.code,
            DErrorCode::Invalid as u32,
            "error code should be Invalid"
        );
    }
}

#[test]
fn test_metadata_invalid_null_base_url() {
    let html = "<html><head><title>Test</title></head></html>";

    unsafe {
        let mut err = DError {
            code: DErrorCode::Ok as u32,
            message: DString::empty(),
        };

        let arena = digests_extract_metadata(html.as_ptr(), html.len(), ptr::null(), 0, &mut err);

        assert!(arena.is_null(), "arena should be null on invalid input");
        assert_eq!(
            err.code,
            DErrorCode::Invalid as u32,
            "error code should be Invalid"
        );
    }
}

#[test]
fn test_metadata_invalid_base_url_format() {
    let html = "<html><head><title>Test</title></head></html>";
    let base_url = "not-a-valid-url";

    unsafe {
        let mut err = DError {
            code: DErrorCode::Ok as u32,
            message: DString::empty(),
        };

        let arena = digests_extract_metadata(
            html.as_ptr(),
            html.len(),
            base_url.as_ptr(),
            base_url.len(),
            &mut err,
        );

        assert!(
            arena.is_null(),
            "arena should be null on invalid base_url format"
        );
        assert_eq!(
            err.code,
            DErrorCode::Invalid as u32,
            "error code should be Invalid for invalid URL"
        );
    }
}

#[test]
fn test_reader_result_null_arena() {
    unsafe {
        let view = digests_reader_result(ptr::null());
        assert!(view.is_null(), "view should be null for null arena");
    }
}

#[test]
fn test_metadata_result_null_arena() {
    unsafe {
        let meta = digests_metadata_result(ptr::null());
        assert!(meta.is_null(), "metadata should be null for null arena");
    }
}

#[test]
fn test_free_null_reader_arena() {
    // Should not crash when freeing null
    unsafe {
        digests_free_reader(ptr::null_mut());
    }
}

#[test]
fn test_free_null_metadata_arena() {
    // Should not crash when freeing null
    unsafe {
        digests_free_metadata(ptr::null_mut());
    }
}

#[test]
fn test_null_out_err() {
    // Verify functions work when out_err is null
    let html = "<html><body><p>Test content.</p></body></html>";
    let url = "https://example.com/test";

    unsafe {
        // Should not crash with null out_err
        let arena = digests_extract_reader(
            url.as_ptr(),
            url.len(),
            html.as_ptr(),
            html.len(),
            ptr::null_mut(),
        );

        // Should succeed
        assert!(!arena.is_null());
        digests_free_reader(arena);
    }
}
