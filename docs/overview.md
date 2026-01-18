# Project Overview

## Architecture

digests-core is a Rust workspace that provides shared parsing primitives with a focus on:

1. **Feed parsing** - Convert RSS/Atom/podcast feeds to structured data
2. **Article extraction** - Extract clean article content from HTML pages
3. **Cross-platform ABI** - Provide C bindings for embedding in other languages

## Components

### `crates/feed`
**Purpose**: Parse RSS, Atom, and podcast feeds
**Output**: `DFeed` structure with standardized feed representation

**Key Features**:
- Support for RSS 0.91, 0.92, 2.0
- Atom 1.0 feed support
- Podcast extensions (RSS with iTunes namespace)
- Robust error handling for malformed feeds
- URL normalization and validation

**Example**:
```rust
use digests_feed::parse_feed;

let feed = parse_feed(r#"
<rss version="2.0">
  <channel>
    <title>Example Feed</title>
    <link>https://example.com</link>
    <description>Sample feed content</description>
  </channel>
</rss>
"#);
```

### `crates/hermes`
**Purpose**: Extract article content and metadata from HTML pages
**Output**: `DReaderView` (cleaned content) and `DMetadata` (structured metadata)

**Key Features**:
- Port of the Hermes article extraction algorithm
- Handles complex modern HTML layouts
- Extracts author, title, publication date, etc.
- Removes ads, navigation, and irrelevant content
- Provides confidence scores for extraction quality

**Example**:
```rust
use digests_hermes::extract_reader_view;

let reader_view = extract_reader_view(
    &url,
    &html_content,
)?;
```

### `crates/ffi`
**Purpose**: C ABI interface for the parsers
**Output**: Arena-allocated results that can be safely transferred across FFI boundary

**Key Functions**:
- `digests_extract_reader()` - Extract article from URL + HTML
- `digests_extract_metadata()` - Extract metadata from HTML
- `digests_free_reader()` / `digests_free_metadata()` - Cleanup arenas

### `crates/cli`
**Purpose**: Command-line interface for development and testing
**Features**:
- Parse feeds from URLs or files
- Extract articles from HTML
- Output in JSON format (pretty-printed or compact)
- Support for stdin input

## Data Flow

1. **Input**: Feed XML or HTML page content
2. **Parsing**: Feed parser → DFeed or Hermes extractor → DReaderView + DMetadata
3. **FFI Conversion**: Rust structs → C-compatible structs
4. **Output**: Structured data ready for consumption by other languages

## Design Principles

1. **Stability**: Maintain backward compatibility in the C ABI
2. **Performance**: Zero-copy where possible, efficient memory management
3. **Robustness**: Graceful handling of malformed input
4. **Simplicity**: Clear interfaces and minimal dependencies
5. **Cross-platform**: Work reliably on Linux, macOS, and Windows