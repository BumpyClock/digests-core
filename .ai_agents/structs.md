# Shared FFI Data Contracts

These structs define the stable C ABI surface for the shared parsing/extraction core. They are plain-old-data; all string fields are UTF-8 slices expressed as `const uint8_t* data` plus `size_t len`. Consumers must not mutate returned memory and must release arenas with `digests_free_arena` (or the specific free function for a handle) when done.

## Common Conventions
- `ptr + len`: strings and blobs; not null-terminated.
- `*_len`: number of elements in an array (not bytes).
- `uint64_t unix_ms`: milliseconds since Unix epoch.
- Memory: results live in an arena allocated by the core; free via the provided free function. Do not free individual pointers.
- Versioning: expose `DIGESTS_FFI_VERSION` (uint32_t) and enforce size checks on the consumer side where possible.

## Feed-Level Types
```c
typedef struct {
    const uint8_t *data;
    size_t len;
} DString;

typedef struct {
    DString url;          // enclosure URL
    DString type;         // MIME type, may be empty
    uint64_t length;      // bytes; 0 if unknown
} DEnclosure;

typedef struct {
    DString url;
    DString title;
} DLink;

typedef struct {
    DString name;
    DString email;
    DString uri;          // optional
} DAuthor;

typedef struct {
    DString title;
    DString url;          // absolute URL
    DString image_url;    // lead image/thumbnail if known
    DString summary;      // description
    DString content;      // full HTML/markdown when present
    DString guid;         // stable ID when provided
    DString language;     // RFC 5646, optional
    DString feed_type;    // "rss", "atom", "podcast"
    uint64_t published_ms;    // 0 if unknown
    uint64_t updated_ms;      // 0 if unknown
    DAuthor author;
    DString *categories;      // array of tags
    size_t categories_len;
    DEnclosure *enclosures;   // media enclosures
    size_t enclosures_len;
    DString primary_media_url; // chosen enclosure URL
    DString thumbnail_url;     // chosen thumbnail
    bool explicit_flag;        // iTunes explicit
    uint32_t duration_seconds; // podcast duration if available
} DFeedItem;

typedef struct {
    DString title;
    DString home_url;
    DString feed_url;
    DString description;
    DString language;
    DString image_url;
    DAuthor author;
    uint64_t published_ms;
    uint64_t updated_ms;
    DFeedItem *items;
    size_t items_len;
    DString generator;
    DString copyright;
} DFeed;
```

## Reader View / Article Extraction
```c
typedef struct {
    DString title;
    DString author;
    DString excerpt;
    DString content;          // normalized HTML or markdown
    DString url;              // canonical URL
    DString site_name;
    DString domain;
    DString language;         // ISO 639-1 or RFC 5646
    DString lead_image_url;
    DString favicon;
    DString theme_color;
    uint64_t published_ms;    // 0 if unknown
    uint64_t word_count;      // optional; 0 if unknown
    uint32_t total_pages;
    uint32_t rendered_pages;
    bool has_video_metadata;
    DString video_url;
} DReaderView;
```

## Metadata/OG Extraction
```c
typedef struct {
    DString title;
    DString description;
    DString site_name;
    DString type;             // og:type
    DString url;              // og:url canonical
    DString image_url;        // og:image
    DString image_alt;
    DString icon_url;         // favicon
    DString theme_color;      // CSS meta theme-color
    DString language;         // detected or meta lang
} DMetadata;
```

## Error Type
```c
typedef enum {
    D_OK = 0,
    D_ERR_PARSE = 1,
    D_ERR_FETCH = 2,
    D_ERR_TIMEOUT = 3,
    D_ERR_INVALID = 4,
    D_ERR_UNSUPPORTED = 5,
    D_ERR_INTERNAL = 255
} DErrorCode;

typedef struct {
    DErrorCode code;
    DString message;    // human-readable
} DError;
```

## Entry Points (C ABI sketch)
```c
typedef struct DFeedArena DFeedArena; // opaque; owns all allocations for a feed result

uint32_t digests_ffi_version(void);

// Feed parsing from bytes (already fetched)
DFeedArena* digests_parse_feed(const uint8_t *data, size_t len, DError *out_err);
const DFeed* digests_feed_result(const DFeedArena*);
void digests_free_feed(DFeedArena*);

// Reader view
typedef struct DReaderArena DReaderArena;
DReaderArena* digests_extract_reader(const uint8_t *url, size_t url_len,
                                     const uint8_t *html, size_t html_len,
                                     DError *out_err);
const DReaderView* digests_reader_result(const DReaderArena*);
void digests_free_reader(DReaderArena*);

// Metadata-only extraction
typedef struct DMetaArena DMetaArena;
DMetaArena* digests_extract_metadata(const uint8_t *html, size_t html_len,
                                     const uint8_t *base_url, size_t base_url_len,
                                     DError *out_err);
const DMetadata* digests_metadata_result(const DMetaArena*);
void digests_free_metadata(DMetaArena*);
```

## Notes for Language Bindings
- Swift: wrap arenas in classes; free in `deinit`. Convert `DString` lazily to `String` when needed.
- Kotlin/Android: JNI/Uniffi; map `DString` to `ByteArray`/`String` as late as possible; free arenas in `close()`.
- .NET: `StructLayout.Sequential`, `nint` for pointers; P/Invoke into `*_result` and free via arena.
- Go: thin cgo layer that copies into Go structs once; caller frees arena after copying.
