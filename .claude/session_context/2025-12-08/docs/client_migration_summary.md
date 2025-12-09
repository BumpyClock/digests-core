# Client.rs Migration from scraper to dom_query

## Summary

Successfully migrated `/Users/adityasharma/Projects/digests-core/crates/hermes/src/client.rs` from the read-only `scraper` crate to the mutable `dom_query` crate. This completes Wave 1 of the migration, which also covered the dom module (brs.rs, cleaners.rs, scoring.rs). The client.rs migration enables DOM mutation capabilities while preserving all existing functionality.

## Files Modified

### Primary Migration File

**`/Users/adityasharma/Projects/digests-core/crates/hermes/src/client.rs`**
- Removed `scraper::Html` and `scraper::Selector` imports
- Changed all internal helper function signatures from `&Html` to `&Document`
- Updated all document parsing from `Html::parse_document()` to `Document::from()`
- Migrated two functions that used `Selector::parse()` to use inline selectors with dom_query
- Updated all extraction helper functions to work with `Document` instead of `Html`

## Key Changes

### 1. Import Changes

**Before:**
```rust
use scraper::{Html, Selector};
```

**After:**
```rust
use dom_query::Document;
```

### 2. Function Signature Updates

All extraction helper functions were updated to accept `&Document` instead of `&Html`:

- `extract_body_inner_html(doc: &Document)`
- `extract_article_body_from_ld_json(doc: &Document)`
- `extract_author(doc: &Document, ...)`
- `extract_date_published(doc: &Document, ...)`
- `extract_lead_image_url(doc: &Document, ...)`
- `extract_site_name(doc: &Document)`
- `extract_site_title(doc: &Document)`
- `extract_site_image(doc: &Document)`
- `extract_description_heuristic(doc: &Document)`
- `extract_language(doc: &Document)`
- `extract_theme_color(doc: &Document)`
- `extract_favicon(doc: &Document)`
- `extract_dek(doc: &Document, ...)`
- `extract_custom_excerpt(doc: &Document, ...)`
- `extract_video_url(doc: &Document)`
- `extract_video_metadata(doc: &Document)`
- `extract_direction(doc: &Document, ...)`
- `extract_next_page_url(doc: &Document, ...)`

### 3. Document Parsing Updates

**Before:**
```rust
let doc = Html::parse_document(&raw_html);
```

**After:**
```rust
let doc = Document::from(raw_html.as_str());
```

This change was applied in three locations:
- `Client::parse()` method (main page)
- `Client::parse()` method (next page in multi-page follow)
- `Client::parse_html()` method

### 4. Selector Migration

#### extract_body_inner_html()

**Before:**
```rust
fn extract_body_inner_html(doc: &Html) -> String {
    if let Ok(selector) = Selector::parse("body") {
        if let Some(body) = doc.select(&selector).next() {
            return body.inner_html();
        }
    }
    String::new()
}
```

**After:**
```rust
fn extract_body_inner_html(doc: &Document) -> String {
    let body = doc.select("body");
    if body.length() > 0 {
        return body.inner_html().to_string();
    }
    String::new()
}
```

Key changes:
- Inline selector string `"body"` instead of `Selector::parse()`
- Use `.length() > 0` instead of `.next()` check
- Call `.to_string()` on `inner_html()` as it returns `Tendril<UTF8>` not `String`

#### extract_article_body_from_ld_json()

**Before:**
```rust
fn extract_article_body_from_ld_json(doc: &Html) -> Option<String> {
    let selector = Selector::parse("script[type='application/ld+json']").ok()?;
    for script in doc.select(&selector) {
        let text = script.text().collect::<String>();
        // ...
    }
    None
}
```

**After:**
```rust
fn extract_article_body_from_ld_json(doc: &Document) -> Option<String> {
    for script in doc.select("script[type='application/ld+json']").iter() {
        let text = script.text().to_string();
        // ...
    }
    None
}
```

Key changes:
- Inline selector string directly in `doc.select()`
- Use `.iter()` to iterate over Selection
- `.text()` returns a String directly, no need for `.collect::<String>()`

## API Translations Reference

| scraper API | dom_query API |
|------------|---------------|
| `Html::parse_document(html)` | `Document::from(html)` |
| `Selector::parse(sel)?` | Inline in `doc.select(sel)` |
| `doc.select(&selector)` | `doc.select(sel).iter()` |
| `element.value().attr("x")` | `selection.attr("x")` |
| `element.text().collect::<String>()` | `selection.text().to_string()` |
| `element.inner_html()` (String) | `selection.inner_html().to_string()` (Tendril → String) |
| `selector.next()` | `selection.length() > 0` for checking |

## Extractor Functions Already Migrated

The following extractor modules had already been migrated to use `Document` (prior work):
- `extractors/fields.rs` - All functions accept `&Document`
- `extractors/select.rs` - All functions accept `&Document`
- `extractors/content.rs` - Public API accepts `&Document`

This meant client.rs could pass `Document` references directly to these functions without any bridging layer.

## Build Verification

Build successful with `cargo build -p digests-hermes`.

Warnings present (unrelated to this migration):
- Unused functions in `extractors/content.rs` - old scraper-based helper functions that are no longer called but haven't been removed yet
- `is_void_element` in `cleaners.rs` - pre-existing warning from Wave 1

## Testing

All existing tests pass. The client.rs file contains 33 integration tests that verify:
- Basic parsing functionality
- Content type conversions (HTML, Markdown, Text)
- Custom extractors
- Generic fallbacks
- Multi-page following
- Metadata extraction
- Video extraction
- RTL/LTR direction detection
- Next page URL extraction
- SSRF protection

No tests required changes as the migration preserved all behavior - only internal implementation details changed.

## Important Implementation Details

### Selection Iteration
- dom_query's `Selection` requires explicit `.iter()` call to iterate
- Example: `doc.select("p").iter()` instead of `doc.select(&selector)`

### String Conversion
- `inner_html()` and `text()` return `Tendril<UTF8>` or custom types
- Must call `.to_string()` to convert to `String` where needed
- Example: `selection.inner_html().to_string()`

### Selection Checking
- Use `.length() > 0` to check if selection has elements
- No direct boolean conversion or `.next()` method

### Inline Selectors
- dom_query validates selectors at runtime during `select()` call
- No need for pre-parsing or error handling on selector creation
- Invalid selectors simply return empty selections

## Notes for Future Development

1. **Remaining scraper usage**: The `scraper` crate is still used in:
   - `metadata_adapter.rs`
   - `formats/mod.rs`
   These will need separate migration efforts.

2. **Unused functions in content.rs**: Several old scraper-based helper functions remain but are unused:
   - `serialize_with_cleaning()`
   - `serialize_node_with_cleaning()`
   - `is_empty_paragraph()`
   - `extract_inner_html_filtered()`

   These can be safely removed in a cleanup pass.

3. **Performance**: dom_query uses the same underlying HTML parser (html5ever) as scraper, so performance characteristics should be similar.

4. **DOM Mutation**: Now that client.rs uses dom_query throughout, it's possible to add DOM mutation capabilities in the future if needed for advanced content cleaning or transformation.

## Conclusion

The client.rs migration was successful with all tests passing and the build succeeding. The file is now fully migrated to dom_query with no scraper dependencies. This completes Wave 1 of the scraper → dom_query migration, covering:
- ✅ dom/brs.rs
- ✅ dom/cleaners.rs
- ✅ dom/scoring.rs
- ✅ client.rs

The codebase is now better positioned for future enhancements that require DOM mutation capabilities.
