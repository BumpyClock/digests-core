// ABOUTME: C FFI bindings for the digests parsing core.
// ABOUTME: Exposes arena-allocated reader and metadata extraction results to Swift/Kotlin consumers.

use std::panic;
use std::ptr;

use bumpalo::Bump;
use digests_feed::{
    apply_metadata_to_feed, enrich_items_with_metadata, parse_feed_bytes, Author as FAuthor,
    Enclosure as FEnclosure, Feed as FFeed, FeedItem as FFeedItem,
};
use digests_hermes::{
    extract_metadata_only, extract_reader_sync, ErrorCode, Metadata, ReaderResult,
};
use reqwest::blocking::Client as HttpClient;
use url::Url;

/// FFI version constant for ABI compatibility checking.
pub const DIGESTS_FFI_VERSION: u32 = 1;

/// Returns the FFI ABI version number.
/// Consumers should check this matches their expected version.
#[no_mangle]
pub extern "C" fn digests_ffi_version() -> u32 {
    DIGESTS_FFI_VERSION
}

// ----------------------------------------------------------------------------
// Error handling
// ----------------------------------------------------------------------------

/// Error codes matching the C ABI DErrorCode enum.
#[repr(u32)]
pub enum DErrorCode {
    Ok = 0,
    Parse = 1,
    Fetch = 2,
    Timeout = 3,
    Invalid = 4,
    Unsupported = 5,
    Internal = 255,
}

/// UTF-8 string slice for FFI. Not null-terminated.
/// Consumer must not mutate or free; memory owned by arena.
#[derive(Copy, Clone)]
#[repr(C)]
pub struct DString {
    pub data: *const u8,
    pub len: usize,
}

impl DString {
    /// Creates an empty DString with null pointer and zero length.
    pub const fn empty() -> Self {
        DString {
            data: ptr::null(),
            len: 0,
        }
    }
}

impl Default for DString {
    fn default() -> Self {
        Self::empty()
    }
}

/// FFI error struct matching C ABI DError.
#[repr(C)]
pub struct DError {
    pub code: u32,
    pub message: DString,
}

impl DError {
    /// Creates a success (D_OK) error with empty message.
    pub const fn ok() -> Self {
        DError {
            code: DErrorCode::Ok as u32,
            message: DString::empty(),
        }
    }
}

// ----------------------------------------------------------------------------
// DReaderView - matches C ABI struct
// ----------------------------------------------------------------------------

#[repr(C)]
pub struct DReaderView {
    pub title: DString,
    pub author: DString,
    pub excerpt: DString,
    pub content: DString,
    pub url: DString,
    pub site_name: DString,
    pub domain: DString,
    pub language: DString,
    pub lead_image_url: DString,
    pub favicon: DString,
    pub theme_color: DString,
    pub published_ms: u64,
    pub word_count: u64,
    pub total_pages: u32,
    pub rendered_pages: u32,
    pub has_video_metadata: bool,
    pub video_url: DString,
}

// ----------------------------------------------------------------------------
// DMetadata - matches C ABI struct
// ----------------------------------------------------------------------------

#[repr(C)]
pub struct DMetadata {
    pub title: DString,
    pub description: DString,
    pub site_name: DString,
    pub og_type: DString,
    pub url: DString,
    pub image_url: DString,
    pub image_alt: DString,
    pub icon_url: DString,
    pub theme_color: DString,
    pub language: DString,
}

// ----------------------------------------------------------------------------
// DFeed / DFeedItem / DEnclosure / DAuthor
// ----------------------------------------------------------------------------

#[derive(Copy, Clone)]
#[repr(C)]
pub struct DEnclosure {
    pub url: DString,
    pub r#type: DString,
    pub length: u64,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct DAuthor {
    pub name: DString,
    pub email: DString,
    pub uri: DString,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct DFeedItem {
    pub title: DString,
    pub url: DString,
    pub image_url: DString,
    pub summary: DString,
    pub content: DString,
    pub guid: DString,
    pub language: DString,
    pub feed_type: DString,
    pub published_ms: u64,
    pub updated_ms: u64,
    pub author: DAuthor,
    pub categories: *const DString,
    pub categories_len: usize,
    pub enclosures: *const DEnclosure,
    pub enclosures_len: usize,
    pub primary_media_url: DString,
    pub thumbnail_url: DString,
    pub explicit_flag: bool,
    pub duration_seconds: u32,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct DFeed {
    pub title: DString,
    pub home_url: DString,
    pub feed_url: DString,
    pub description: DString,
    pub language: DString,
    pub image_url: DString,
    pub author: DAuthor,
    pub published_ms: u64,
    pub updated_ms: u64,
    pub items: *const DFeedItem,
    pub items_len: usize,
    pub generator: DString,
    pub copyright: DString,
    pub feed_type: DString,
}

// ----------------------------------------------------------------------------
// Arena types
// ----------------------------------------------------------------------------

/// Arena holding reader extraction results.
/// All allocations for the view live in the bump allocator.
pub struct DReaderArena {
    #[allow(dead_code)]
    bump: Bump,
    view: *const DReaderView,
}

/// Arena holding metadata extraction results.
/// All allocations for the metadata live in the bump allocator.
pub struct DMetaArena {
    #[allow(dead_code)]
    bump: Bump,
    metadata: *const DMetadata,
}

/// Arena holding feed parsing results.
/// All allocations for feed + items live in the bump allocator.
pub struct DFeedArena {
    #[allow(dead_code)]
    bump: Bump,
    feed: *const DFeed,
}

// ----------------------------------------------------------------------------
// HTTP helper for enrichment
// ----------------------------------------------------------------------------

fn fetch_html(client: &HttpClient, url: &str) -> Result<String, reqwest::Error> {
    let resp = client.get(url).send()?.error_for_status()?;
    resp.text()
}

fn pick_site_url(feed: &FFeed) -> Option<String> {
    if !feed.home_url.is_empty() {
        return Some(feed.home_url.clone());
    }
    if !feed.feed_url.is_empty() {
        if let Ok(parsed) = Url::parse(&feed.feed_url) {
            if let Some(host) = parsed.host_str() {
                return Some(format!("{}://{}", parsed.scheme(), host));
            }
        }
        return Some(feed.feed_url.clone());
    }
    None
}

// ----------------------------------------------------------------------------
// Helper functions
// ----------------------------------------------------------------------------

/// Copies a string into the arena and returns a DString pointing to it.
fn copy_str_to_arena(bump: &Bump, s: &str) -> DString {
    if s.is_empty() {
        return DString::empty();
    }
    let bytes = bump.alloc_slice_copy(s.as_bytes());
    DString {
        data: bytes.as_ptr(),
        len: bytes.len(),
    }
}

/// Creates a DReaderView in the arena from a ReaderResult.
fn make_reader_view(bump: &Bump, rr: &ReaderResult) -> *const DReaderView {
    let view = bump.alloc(DReaderView {
        title: copy_str_to_arena(bump, &rr.title),
        author: copy_str_to_arena(bump, &rr.author),
        excerpt: copy_str_to_arena(bump, &rr.excerpt),
        content: copy_str_to_arena(bump, &rr.content),
        url: copy_str_to_arena(bump, &rr.url),
        site_name: copy_str_to_arena(bump, &rr.site_name),
        domain: copy_str_to_arena(bump, &rr.domain),
        language: copy_str_to_arena(bump, &rr.language),
        lead_image_url: copy_str_to_arena(bump, &rr.lead_image_url),
        favicon: copy_str_to_arena(bump, &rr.favicon),
        theme_color: copy_str_to_arena(bump, &rr.theme_color),
        published_ms: rr.published_ms,
        word_count: rr.word_count,
        total_pages: rr.total_pages,
        rendered_pages: rr.rendered_pages,
        has_video_metadata: rr.has_video_metadata,
        video_url: copy_str_to_arena(bump, &rr.video_url),
    });
    view as *const DReaderView
}

/// Creates a DMetadata in the arena from a Metadata.
fn make_metadata_view(bump: &Bump, meta: &Metadata) -> *const DMetadata {
    let dm = bump.alloc(DMetadata {
        title: copy_str_to_arena(bump, &meta.title),
        description: copy_str_to_arena(bump, &meta.description),
        site_name: copy_str_to_arena(bump, &meta.site_name),
        og_type: copy_str_to_arena(bump, &meta.og_type),
        url: copy_str_to_arena(bump, &meta.url),
        image_url: copy_str_to_arena(bump, &meta.image_url),
        image_alt: copy_str_to_arena(bump, &meta.image_alt),
        icon_url: copy_str_to_arena(bump, &meta.icon_url),
        theme_color: copy_str_to_arena(bump, &meta.theme_color),
        language: copy_str_to_arena(bump, &meta.language),
    });
    dm as *const DMetadata
}

/// Creates a DAuthor from FAuthor.
fn make_author(bump: &Bump, a: &FAuthor) -> DAuthor {
    DAuthor {
        name: copy_str_to_arena(bump, a.name.as_deref().unwrap_or("")),
        email: copy_str_to_arena(bump, a.email.as_deref().unwrap_or("")),
        uri: copy_str_to_arena(bump, a.uri.as_deref().unwrap_or("")),
    }
}

/// Creates a DEnclosure slice from feed enclosures.
fn make_enclosures<'a>(bump: &'a Bump, encs: &[FEnclosure]) -> (&'a [DEnclosure], usize) {
    let out_iter = encs.iter().map(|e| DEnclosure {
        url: copy_str_to_arena(bump, &e.url),
        r#type: copy_str_to_arena(bump, e.mime_type.as_deref().unwrap_or("")),
        length: e.length,
    });
    let slice = bump.alloc_slice_fill_iter(out_iter);
    (slice, slice.len())
}

/// Creates a DFeedItem slice from feed items.
fn make_feed_items<'a>(bump: &'a Bump, items: &[FFeedItem]) -> (&'a [DFeedItem], usize) {
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        // Categories
        let cat_iter = it.categories.iter().map(|c| copy_str_to_arena(bump, c));
        let cat_slice = bump.alloc_slice_fill_iter(cat_iter);

        // Enclosures
        let (enc_slice, enc_len) = make_enclosures(bump, &it.enclosures);

        out.push(DFeedItem {
            title: copy_str_to_arena(bump, &it.title),
            url: copy_str_to_arena(bump, &it.url),
            image_url: copy_str_to_arena(bump, it.image_url.as_deref().unwrap_or("")),
            summary: copy_str_to_arena(bump, &it.summary),
            content: copy_str_to_arena(bump, &it.content),
            guid: copy_str_to_arena(bump, &it.guid),
            language: copy_str_to_arena(bump, it.language.as_deref().unwrap_or("")),
            feed_type: copy_str_to_arena(bump, &it.feed_type),
            published_ms: it.published_ms,
            updated_ms: it.updated_ms,
            author: make_author(bump, &it.author.clone().unwrap_or_default()),
            categories: cat_slice.as_ptr(),
            categories_len: cat_slice.len(),
            enclosures: enc_slice.as_ptr(),
            enclosures_len: enc_len,
            primary_media_url: copy_str_to_arena(
                bump,
                it.primary_media_url.as_deref().unwrap_or(""),
            ),
            thumbnail_url: copy_str_to_arena(bump, it.thumbnail_url.as_deref().unwrap_or("")),
            explicit_flag: it.explicit_flag,
            duration_seconds: it.duration_seconds,
        });
    }
    let slice = bump.alloc_slice_fill_iter(out.into_iter());
    (slice, slice.len())
}

/// Creates a DFeed in the arena from a Feed.
fn make_feed_view(bump: &Bump, feed: &FFeed) -> *const DFeed {
    let (items_slice, items_len) = make_feed_items(bump, &feed.items);
    let df = bump.alloc(DFeed {
        title: copy_str_to_arena(bump, &feed.title),
        home_url: copy_str_to_arena(bump, &feed.home_url),
        feed_url: copy_str_to_arena(bump, &feed.feed_url),
        description: copy_str_to_arena(bump, &feed.description),
        language: copy_str_to_arena(bump, feed.language.as_deref().unwrap_or("")),
        image_url: copy_str_to_arena(bump, feed.image_url.as_deref().unwrap_or("")),
        author: make_author(bump, &feed.author.clone().unwrap_or_default()),
        published_ms: feed.published_ms,
        updated_ms: feed.updated_ms,
        items: items_slice.as_ptr(),
        items_len,
        generator: copy_str_to_arena(bump, feed.generator.as_deref().unwrap_or("")),
        copyright: copy_str_to_arena(bump, feed.copyright.as_deref().unwrap_or("")),
        feed_type: copy_str_to_arena(bump, &feed.feed_type),
    });
    df as *const DFeed
}
/// Maps a ParseError code to a DErrorCode.
fn map_error_code(code: ErrorCode) -> u32 {
    match code {
        ErrorCode::InvalidUrl => DErrorCode::Invalid as u32,
        ErrorCode::Fetch => DErrorCode::Fetch as u32,
        ErrorCode::Timeout => DErrorCode::Timeout as u32,
        ErrorCode::Ssrf => DErrorCode::Invalid as u32,
        ErrorCode::Extract => DErrorCode::Parse as u32,
        ErrorCode::Context => DErrorCode::Internal as u32,
    }
}

/// Sets the out_err with the given code and message.
/// The message is allocated in the provided bump arena.
/// If out_err is null, this is a no-op.
unsafe fn set_error(out_err: *mut DError, bump: &Bump, code: u32, message: &str) {
    if !out_err.is_null() {
        (*out_err).code = code;
        (*out_err).message = copy_str_to_arena(bump, message);
    }
}

/// Sets out_err to success (D_OK with empty message).
/// If out_err is null, this is a no-op.
unsafe fn set_success(out_err: *mut DError) {
    if !out_err.is_null() {
        (*out_err).code = DErrorCode::Ok as u32;
        (*out_err).message = DString::empty();
    }
}

// ----------------------------------------------------------------------------
// Reader FFI functions
// ----------------------------------------------------------------------------

/// Blocking reader extraction. Returns arena-allocated ReaderResult.
///
/// # Arguments
/// * `url` - URL bytes (UTF-8)
/// * `url_len` - Length of URL in bytes
/// * `html` - HTML content bytes (UTF-8)
/// * `html_len` - Length of HTML in bytes
/// * `out_err` - Output error struct (may be null)
///
/// # Returns
/// Pointer to DReaderArena on success, null on failure.
/// On failure, out_err (if non-null) contains error details.
///
/// # Safety
/// Caller must free the returned arena via digests_free_reader.
#[no_mangle]
pub unsafe extern "C" fn digests_extract_reader(
    url: *const u8,
    url_len: usize,
    html: *const u8,
    html_len: usize,
    out_err: *mut DError,
) -> *mut DReaderArena {
    // Create a temporary bump for error messages if we fail early
    let err_bump = Bump::new();

    // Validate inputs
    if url.is_null() || url_len == 0 {
        set_error(
            out_err,
            &err_bump,
            DErrorCode::Invalid as u32,
            "url is null or empty",
        );
        return ptr::null_mut();
    }
    if html.is_null() || html_len == 0 {
        set_error(
            out_err,
            &err_bump,
            DErrorCode::Invalid as u32,
            "html is null or empty",
        );
        return ptr::null_mut();
    }

    // Convert to &str
    let url_slice = std::slice::from_raw_parts(url, url_len);
    let url_str = match std::str::from_utf8(url_slice) {
        Ok(s) => s,
        Err(_) => {
            set_error(
                out_err,
                &err_bump,
                DErrorCode::Invalid as u32,
                "url is not valid UTF-8",
            );
            return ptr::null_mut();
        }
    };

    let html_slice = std::slice::from_raw_parts(html, html_len);
    let html_str = match std::str::from_utf8(html_slice) {
        Ok(s) => s,
        Err(_) => {
            set_error(
                out_err,
                &err_bump,
                DErrorCode::Invalid as u32,
                "html is not valid UTF-8",
            );
            return ptr::null_mut();
        }
    };

    // Catch panics to avoid unwinding across FFI boundary
    let result = panic::catch_unwind(|| extract_reader_sync(url_str, html_str));

    match result {
        Ok(Ok(reader_result)) => {
            // Success - create arena and view
            let bump = Bump::new();
            let view = make_reader_view(&bump, &reader_result);
            let arena = Box::new(DReaderArena { bump, view });
            set_success(out_err);
            Box::into_raw(arena)
        }
        Ok(Err(parse_err)) => {
            // ParseError from hermes
            let code = map_error_code(parse_err.code);
            let msg = parse_err.to_string();
            set_error(out_err, &err_bump, code, &msg);
            ptr::null_mut()
        }
        Err(_) => {
            // Panic caught
            set_error(
                out_err,
                &err_bump,
                DErrorCode::Internal as u32,
                "internal panic during extraction",
            );
            ptr::null_mut()
        }
    }
}

/// Returns a pointer to the DReaderView inside the arena.
///
/// # Safety
/// The arena pointer must be valid and non-null.
/// The returned pointer is valid until digests_free_reader is called.
#[no_mangle]
pub unsafe extern "C" fn digests_reader_result(arena: *const DReaderArena) -> *const DReaderView {
    if arena.is_null() {
        return ptr::null();
    }
    (*arena).view
}

/// Frees the reader arena and all associated allocations.
///
/// # Safety
/// The arena pointer must be valid and must have been returned by digests_extract_reader.
/// After this call, the arena pointer is invalid.
#[no_mangle]
pub unsafe extern "C" fn digests_free_reader(arena: *mut DReaderArena) {
    if !arena.is_null() {
        drop(Box::from_raw(arena));
    }
}

// ----------------------------------------------------------------------------
// Metadata FFI functions
// ----------------------------------------------------------------------------

/// Extracts metadata from HTML. Returns arena-allocated Metadata.
///
/// # Arguments
/// * `html` - HTML content bytes (UTF-8)
/// * `html_len` - Length of HTML in bytes
/// * `base_url` - Base URL bytes for resolving relative URLs (UTF-8)
/// * `base_url_len` - Length of base URL in bytes
/// * `out_err` - Output error struct (may be null)
///
/// # Returns
/// Pointer to DMetaArena on success, null on failure.
/// On failure, out_err (if non-null) contains error details.
///
/// # Safety
/// Caller must free the returned arena via digests_free_metadata.
#[no_mangle]
pub unsafe extern "C" fn digests_extract_metadata(
    html: *const u8,
    html_len: usize,
    base_url: *const u8,
    base_url_len: usize,
    out_err: *mut DError,
) -> *mut DMetaArena {
    // Create a temporary bump for error messages if we fail early
    let err_bump = Bump::new();

    // Validate inputs
    if html.is_null() || html_len == 0 {
        set_error(
            out_err,
            &err_bump,
            DErrorCode::Invalid as u32,
            "html is null or empty",
        );
        return ptr::null_mut();
    }
    if base_url.is_null() || base_url_len == 0 {
        set_error(
            out_err,
            &err_bump,
            DErrorCode::Invalid as u32,
            "base_url is null or empty",
        );
        return ptr::null_mut();
    }

    // Convert to &str
    let html_slice = std::slice::from_raw_parts(html, html_len);
    let html_str = match std::str::from_utf8(html_slice) {
        Ok(s) => s,
        Err(_) => {
            set_error(
                out_err,
                &err_bump,
                DErrorCode::Invalid as u32,
                "html is not valid UTF-8",
            );
            return ptr::null_mut();
        }
    };

    let base_url_slice = std::slice::from_raw_parts(base_url, base_url_len);
    let base_url_str = match std::str::from_utf8(base_url_slice) {
        Ok(s) => s,
        Err(_) => {
            set_error(
                out_err,
                &err_bump,
                DErrorCode::Invalid as u32,
                "base_url is not valid UTF-8",
            );
            return ptr::null_mut();
        }
    };

    // Catch panics to avoid unwinding across FFI boundary
    let result = panic::catch_unwind(|| extract_metadata_only(html_str, base_url_str));

    match result {
        Ok(Ok(metadata)) => {
            // Success - create arena and view
            let bump = Bump::new();
            let meta_ptr = make_metadata_view(&bump, &metadata);
            let arena = Box::new(DMetaArena {
                bump,
                metadata: meta_ptr,
            });
            set_success(out_err);
            Box::into_raw(arena)
        }
        Ok(Err(parse_err)) => {
            // ParseError from hermes
            let code = map_error_code(parse_err.code);
            let msg = parse_err.to_string();
            set_error(out_err, &err_bump, code, &msg);
            ptr::null_mut()
        }
        Err(_) => {
            // Panic caught
            set_error(
                out_err,
                &err_bump,
                DErrorCode::Internal as u32,
                "internal panic during metadata extraction",
            );
            ptr::null_mut()
        }
    }
}

/// Returns a pointer to the DMetadata inside the arena.
///
/// # Safety
/// The arena pointer must be valid and non-null.
/// The returned pointer is valid until digests_free_metadata is called.
#[no_mangle]
pub unsafe extern "C" fn digests_metadata_result(arena: *const DMetaArena) -> *const DMetadata {
    if arena.is_null() {
        return ptr::null();
    }
    (*arena).metadata
}

/// Frees the metadata arena and all associated allocations.
///
/// # Safety
/// The arena pointer must be valid and must have been returned by digests_extract_metadata.
/// After this call, the arena pointer is invalid.
#[no_mangle]
pub unsafe extern "C" fn digests_free_metadata(arena: *mut DMetaArena) {
    if !arena.is_null() {
        drop(Box::from_raw(arena));
    }
}

// ----------------------------------------------------------------------------
// Feed parsing + enrichment FFI
// ----------------------------------------------------------------------------

/// Parses feed bytes, enriches feed-level metadata by fetching site HTML, and returns arena.
#[no_mangle]
pub unsafe extern "C" fn digests_parse_feed(
    feed_url_ptr: *const u8,
    feed_url_len: usize,
    data_ptr: *const u8,
    data_len: usize,
    out_err: *mut DError,
) -> *mut DFeedArena {
    let err_bump = Bump::new();

    if feed_url_ptr.is_null() || data_ptr.is_null() || feed_url_len == 0 || data_len == 0 {
        set_error(
            out_err,
            &err_bump,
            DErrorCode::Invalid as u32,
            "invalid input",
        );
        return ptr::null_mut();
    }

    let feed_url_bytes = std::slice::from_raw_parts(feed_url_ptr, feed_url_len);
    let data_bytes = std::slice::from_raw_parts(data_ptr, data_len);

    let feed_url = match std::str::from_utf8(feed_url_bytes) {
        Ok(s) => s,
        Err(_) => {
            set_error(
                out_err,
                &err_bump,
                DErrorCode::Invalid as u32,
                "feed_url not utf-8",
            );
            return ptr::null_mut();
        }
    };

    let feed_result = panic::catch_unwind(|| parse_feed_bytes(data_bytes, feed_url));

    let mut feed = match feed_result {
        Ok(Ok(f)) => f,
        Ok(Err(e)) => {
            set_error(out_err, &err_bump, DErrorCode::Parse as u32, &e.to_string());
            return ptr::null_mut();
        }
        Err(_) => {
            set_error(
                out_err,
                &err_bump,
                DErrorCode::Internal as u32,
                "panic during feed parse",
            );
            return ptr::null_mut();
        }
    };

    // Enrichment: feed-level + item-level metadata using a shared blocking client
    if let Ok(http_client) = HttpClient::builder().user_agent("digests-core/ffi").build() {
        // Feed-level metadata from site/homepage
        if let Some(site_url) = pick_site_url(&feed) {
            if let Ok(site_html) = fetch_html(&http_client, &site_url) {
                if let Ok(meta) = extract_metadata_only(&site_html, &site_url) {
                    apply_metadata_to_feed(&mut feed, &meta);
                }
            }
        }

        // Item-level metadata thumbnails (only when missing)
        enrich_items_with_metadata(&mut feed, |url| {
            fetch_html(&http_client, url)
                .ok()
                .and_then(|html| extract_metadata_only(&html, url).ok())
        });
    }

    let arena_bump = Bump::new();
    let feed_ptr = make_feed_view(&arena_bump, &feed);
    let arena = DFeedArena {
        bump: arena_bump,
        feed: feed_ptr,
    };
    set_success(out_err);
    Box::into_raw(Box::new(arena))
}

/// Returns the feed view for a given feed arena.
#[no_mangle]
pub unsafe extern "C" fn digests_feed_result(arena: *const DFeedArena) -> *const DFeed {
    if arena.is_null() {
        return ptr::null();
    }
    (*arena).feed
}

/// Frees a feed arena.
#[no_mangle]
pub unsafe extern "C" fn digests_free_feed(arena: *mut DFeedArena) {
    if !arena.is_null() {
        drop(Box::from_raw(arena));
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_version() {
        assert_eq!(digests_ffi_version(), 1);
    }

    #[test]
    fn test_dstring_empty() {
        let s = DString::empty();
        assert!(s.data.is_null());
        assert_eq!(s.len, 0);
    }

    #[test]
    fn test_copy_str_to_arena() {
        let bump = Bump::new();
        let s = "hello world";
        let ds = copy_str_to_arena(&bump, s);
        assert!(!ds.data.is_null());
        assert_eq!(ds.len, 11);
        unsafe {
            let slice = std::slice::from_raw_parts(ds.data, ds.len);
            assert_eq!(std::str::from_utf8(slice).unwrap(), "hello world");
        }
    }

    #[test]
    fn test_copy_empty_str_to_arena() {
        let bump = Bump::new();
        let ds = copy_str_to_arena(&bump, "");
        assert!(ds.data.is_null());
        assert_eq!(ds.len, 0);
    }
}
