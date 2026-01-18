# Troubleshooting

This guide covers common issues and solutions when working with digests-core.

## Build Issues

### Linker Errors on Linux

**Problem**: `ld: cannot find -lssl` or similar SSL library errors

**Solution**: Install OpenSSL development headers
```bash
# Ubuntu/Debian
sudo apt-get install libssl-dev pkg-config

# Fedora/CentOS
sudo dnf install openssl-devel pkgconfig

# Arch Linux
sudo pacman -s openssl
```

**Verify installation**:
```bash
pkg-config --libs openssl  # Should show -lssl -lcrypto
```

### Windows Build Errors

**Problem**: `LINK : fatal error LNK1181: cannot open input file 'libcmt.lib'`

**Solution**: Install Visual Studio Build Tools
```bash
# Download and install Visual Studio Build Tools
# Select "Desktop development with C++" workload
```

**Alternative with vcpkg**:
```bash
vcpkg install openssl:x64-windows
cargo build -p digests-ffi --release
```

### macOS SSL Issues

**Problem**: `dyld: Library not loaded: /usr/local/opt/openssl/lib/libssl.1.1.dylib`

**Solution**: Install OpenSSL via Homebrew
```bash
brew install openssl
export OPENSSL_DIR=$(brew --prefix openssl)
export RUSTFLAGS="-L native=$OPENSSL_DIR/lib"
export PKG_CONFIG_PATH="$OPENSSL_DIR/lib/pkgconfig"
```

## FFI Issues

### Loading the Library

**Problem**: `dlopen: cannot open shared object file: No such file or directory`

**Solution**: Ensure library is built and provide correct path
```bash
# Build the library first
cargo build -p digests-ffi --release

# Check library exists
ls target/release/libdigests_ffi.*  # Linux/macOS
ls target/release/digests_ffi.dll   # Windows

# Use full path when loading
./target/release/libdigests_ffi.so  # Linux
./target/release/libdigests_ffi.dylib  # macOS
./target/release/digests_ffi.dll  # Windows
```

### Memory Leaks

**Problem**: Memory usage grows with repeated extractions

**Solution**: Always call free functions
```c
// Always free the arena
DReaderArena* arena = digests_extract_reader(...);
// ... use arena ...
digests_free_reader(arena);  // Required!

// For metadata
DMetaArena* meta_arena = digests_extract_metadata(...);
// ... use metadata ...
digests_free_metadata(meta_arena);
```

### NULL Pointer Errors

**Problem**: Segmentation fault accessing NULL pointers

**Solution**: Check all return values
```c
char* err = NULL;
DReaderArena* arena = digests_extract_reader(url, url_len, html, html_len, &err);

if (err) {
    // Handle error
    printf("Error: %s\n", err);
    free(err);
    return;
}

if (!arena) {
    // Handle NULL arena
    printf("Extraction failed\n");
    return;
}

// Safe to use arena
const DReaderView* view = digests_reader_result(arena);
```

## Feed Parsing Issues

### Invalid Feed Format

**Problem**: `ParseError::InvalidFeedFormat` error

**Solution**: Check feed format and validate XML
```rust
use digests_feed::{parse_feed, detect_feed_type};

// Detect feed type first
let feed_type = detect_feed_type(&feed_content);
match feed_type {
    FeedType::RSS | FeedType::Atom => {
        // Try parsing
        let feed = parse_feed(&feed_content)?;
        // Success!
    }
    FeedType::Unknown => {
        println!("Unknown feed format");
        // Handle unknown format
    }
}
```

### URL Validation Errors

**Problem**: `ParseError::InvalidUrl` for URLs in feed

**Solution**: Disable URL validation or fix URLs
```rust
// Disable URL validation (for performance)
std::env::set_var("DIGESTS_FEED_VALIDATE_URLS", "false");

// Or fix URLs before parsing
let fixed_content = feed_content
    .replace("http://", "https://")
    .replace("www.example.com", "correct-domain.com");
```

### Malformed XML

**Problem**: `ParseError::XmlError` with XML parsing failures

**Solution**: Clean XML before parsing
```rust
use std::io::Error;

fn clean_xml(xml: &str) -> Result<String, Error> {
    // Remove problematic XML entities
    let cleaned = xml
        .replace("&nbsp;", " ")
        .replace("&ndash;", "-")
        .replace("&mdash;", "--")
        .replace("&hellip;", "...");

    Ok(cleaned)
}

let cleaned_xml = clean_xml(&malformed_xml)?;
let feed = parse_feed(&cleaned_xml)?;
```

## Article Extraction Issues

### Low Confidence Scores

**Problem**: Extraction confidence < 0.7

**Solution**: Improve input HTML or adjust extraction settings
```rust
use digests_hermes::{extract_reader_view, ReaderViewOptions};

// Try with more lenient options
let options = ReaderViewOptions {
    min_content_length: 200,     // Reduce minimum length
    max_density: 2.0,          // Allow higher density
    preserve_links: true,
    extract_metadata: true,
    timeout_ms: 10000,        // Longer timeout
};

let (view, _) = extract_reader_view_with_options("https://example.com", html, options)?;

if view.confidence < 0.5 {
    // Try alternative extraction strategy
    let cleaned_html = remove_ads(&html);
    let (clean_view, _) = extract_reader_view("https://example.com", &cleaned_html)?;
}
```

### Missing Content

**Problem**: Extracted content is empty or very short

**Solution**: Check for dynamic content or malformed HTML
```rust
// Check if content is JavaScript-dependent
if html.contains("DOMContentLoaded") || html.contains("window.onload") {
    println!("Warning: Dynamic content detected");
}

// Try extracting anyway but expect lower quality
let (view, _) = extract_reader_view(url, html)?;
if view.content.len() < 100 {
    println!("Short content: {} chars", view.content.len());
}
```

### Timeout Errors

**Problem**: Extraction times out on complex pages

**Solution**: Reduce timeout or simplify input
```rust
// Lower timeout for faster processing
let options = ReaderViewOptions {
    timeout_ms: 2000,  // 2 seconds
    // ... other options
};

// Or pre-process HTML to remove heavy elements
let simple_html = remove_heavy_elements(&html);
let (view, _) = extract_reader_view(url, &simple_html)?;
```

## CLI Issues

### Network Errors

**Problem**: `Network error: Connection timeout` or similar

**Solution**: Check network and adjust timeouts
```bash
# Increase timeout
digests-cli extract --timeout 60 https://slow-site.com

# Use custom user agent
digests-cli extract --user-agent "Mozilla/5.0" https://site.com

# Skip SSL verification (development only)
digests-cli extract --insecure https://self-signed-site.com
```

### Large Output Files

**Problem**: JSON output is too large for console

**Solution**: Use compact output or redirect to file
```bash
# Compact output
digests-cli parse --compact https://large-feed.com

# Save to file
digests-cli parse https://large-feed.com > output.json

# Process with jq
digests-cli parse https://large-feed.com | jq '.items[0].title'
```

### Authentication Issues

**Problem**: `401 Unauthorized` errors

**Solution**: Add authentication headers
```bash
# Basic auth
digests-cli parse https://protected-feed.com \
  --headers "Authorization: Basic $(echo -n 'user:pass' | base64)"

# Bearer token
digests-cli parse https://api.example.com/feed \
  --headers "Authorization: Bearer token123"
```

## Performance Issues

### Slow Compilation

**Problem**: Cargo builds take too long

**Solution**: Enable incremental compilation and parallel jobs
```bash
# Enable incremental builds (default)
export CARGO_INCREMENTAL=1

# Use parallel jobs
cargo build -j 4

# For development builds (faster)
cargo build
```

### Memory Usage

**Problem**: High memory usage during extraction

**Solution**: Process in chunks or limit input size
```rust
// Limit input size
const MAX_INPUT_SIZE: usize = 1024 * 1024; // 1MB
let html_to_process = if html.len() > MAX_INPUT_SIZE {
    &html[..MAX_INPUT_SIZE]
} else {
    &html
};

let (view, _) = extract_reader_view(url, html_to_process)?;
```

## Platform-Specific Issues

### Windows Path Issues

**Problem**: File path separator issues

**Solution**: Use proper path handling
```c
// Windows: use backslashes or forward slashes
const char* path = "C:\\path\\to\\file.html";
// or
const char* path = "C:/path/to/file.html";

// Normalize paths
char* normalized_path = normalize_windows_path(path);
```

### macOS Dynamic Library Loading

**Problem**: Library not found at runtime

**Solution**: Set library path or install to system
```bash
# Add library to path
export DYLD_LIBRARY_PATH=.:$DYLD_LIBRARY_PATH

# Or install to /usr/local/lib
sudo cp target/release/libdigests_ffi.dylib /usr/local/lib/
```

### Linux Permission Issues

**Problem**: `Permission denied` when accessing files

**Solution**: Check file permissions
```bash
# Check file permissions
ls -la target/release/libdigests_ffi.so

# Fix permissions if needed
chmod 644 target/release/libdigests_ffi.so
```

## Debug Mode

### Enable Debug Output

Set environment variables for detailed logging:
```bash
# Feed parsing debug
export DIGESTS_FEED_DEBUG=1

# Hermes extraction debug
export DIGESTS_HERMES_DEBUG=1

# FFI debug
export DIGESTS_FFI_DEBUG=1

# CLI debug
export DIGESTS_CLI_DEBUG=1

# General debug
export DIGESTS_DEBUG=1
```

### Example Debug Output
```bash
$ DIGESTS_HERMES_DEBUG=1 digests-cli extract https://example.com/article
[DEBUG] Extracting from https://example.com/article
[DEBUG] HTML size: 45234 bytes
[DEBUG] Found main content area
[DEBUG] Extracted content: 12345 characters
[DEBUG] Confidence score: 0.87
```

## Getting Help

### Check Known Issues
Search existing GitHub issues:
- [Issues](https://github.com/BumpyClock/digests-core/issues)

### Create a Good Bug Report
When reporting issues, include:
1. **Environment**: OS, Rust version, target architecture
2. **Steps to reproduce**: Minimal code example
3. **Expected behavior**: What should happen
4. **Actual behavior**: What happens instead
5. **Error messages**: Full error output
6. **Relevant code**: Code snippet that causes the issue

### Example Bug Report
```markdown
## Environment
- OS: Ubuntu 22.04
- Rust: 1.75.0
- Target: x86_64-unknown-linux-gnu

## Steps to reproduce
1. Build the project: `cargo build`
2. Run CLI: `./target/release/digests-cli extract https://example.com`
3. Observe error

## Expected
Article content should be extracted and printed as JSON

## Actual
Error: Network error: Connection timeout

## Additional info
- Works with other URLs
- Timeout happens consistently
- Example URL: https://example.com (requires auth)
```