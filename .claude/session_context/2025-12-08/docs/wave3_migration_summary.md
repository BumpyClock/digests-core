# Scraper to dom_query Migration - Wave 3 Summary

## Overview

Successfully completed Wave 3 of the scraper→dom_query migration for the Hermes crate. This wave focused on migrating the remaining files that had not yet been converted and cleaning up unused legacy code.

## Files Modified

### 1. `/Users/adityasharma/Projects/digests-core/crates/hermes/src/formats/mod.rs`

**Changes:**
- Replaced `use scraper::{Html, Selector}` with `use dom_query::Document`
- Migrated `html_to_text()` function:
  - Changed `Html::parse_document()` → `Document::from()`
  - Changed `document.root_element().text().collect::<Vec<_>>().join(" ")` → `document.text().to_string()`
- Migrated `extract_title()` function:
  - Changed `Html::parse_document()` → `Document::from()`
  - Replaced `Selector::parse()` with inline `document.select()`
  - Changed `element.select(&selector).next()` → `selection.length() > 0`
  - Changed `element.text().collect()` → `selection.text().to_string()`
  - Changed `element.value().attr()` → `selection.attr()`

**Test Results:** ✅ All 26 tests pass

### 2. `/Users/adityasharma/Projects/digests-core/crates/hermes/src/metadata_adapter.rs`

**Changes:**
- Replaced `use scraper::{Html, Selector}` with `use dom_query::Document`
- Migrated helper functions:
  - `get_meta_property()`: Updated to accept `&Document`, use inline selectors
  - `get_meta_name()`: Updated to accept `&Document`, use inline selectors
  - `get_meta()`: Updated to accept `&Document`
- Migrated `extract_metadata_only()` function:
  - Changed `Html::parse_document()` → `Document::from()`
  - Replaced all `Selector::parse()` with inline `document.select()`
  - Changed `element.select(&sel).next()` → `selection.length() > 0`
  - Changed `element.value().attr()` → `selection.attr()`
  - Changed `element.text().collect::<String>()` → `selection.text().to_string()`

**Test Results:** ✅ All 5 tests pass

### 3. `/Users/adityasharma/Projects/digests-core/crates/hermes/src/extractors/content.rs`

**Changes:**
- Removed unused legacy functions:
  - `serialize_with_cleaning()` (line 714)
  - `serialize_node_with_cleaning()` (line 726)
  - `is_empty_paragraph()` (line 818)
  - `extract_inner_html_filtered()` (line 873)

These functions were dead code that had been replaced by the new dom_query implementation in `apply_default_clean()` during earlier waves.

**Note:** Scraper is still imported and used in content.rs for the transform serialization system (`apply_filters_and_transforms_legacy` and related functions). This is expected and documented as part of the partial migration status.

## Build Status

✅ **Compilation:** Successful
```
cargo build -p digests-hermes
```
- Only 1 warning: `is_void_element` is unused in `dom/cleaners.rs` (unrelated to this migration)

## Test Status

**Overall:** 183 passed, 4 failed (same as before Wave 3)

The 4 failing tests are **pre-existing failures** from previous migration waves and are not caused by Wave 3 changes:

1. `client::tests::parse_html_returns_result` - Different HTML wrapping behavior in dom_query
2. `client::tests::parse_respects_content_type_markdown` - Different HTML wrapping behavior
3. `client::tests::parse_returns_content_from_fetch` - Different HTML wrapping behavior
4. `dom::brs::tests::test_brs_to_ps_basic` - Different HTML output structure

These failures are documented in the Wave 2 migration summary as behavioral differences between scraper and dom_query HTML processing.

**Wave 3 specific tests:** ✅ All pass
- formats module: 26/26 tests pass
- metadata_adapter module: 5/5 tests pass

## Scraper Dependency Status

❌ **Cannot be removed yet**

Attempted to remove scraper from Cargo.toml but build fails because the transform serialization system in content.rs still depends on scraper types:
- `apply_filters_and_transforms_legacy()` and its helper functions
- `apply_transforms()`
- `serialize_node_with_transforms()`
- `serialize_filtered()`
- `serialize_node()`
- `serialize_node_preserve()`
- `fix_headings()` and `serialize_node_demote_headings()`
- `rewrite_empty_links()` and `serialize_node_unwrap_links()`

These functions use `scraper::Html`, `scraper::Selector`, and `ego_tree::NodeRef<scraper::Node>` types extensively for custom HTML serialization with transforms.

**Recommendation:** Keep scraper dependency until the transform system is fully migrated to use dom_query's mutation API (`selection.rename()`, `selection.unwrap()`, `selection.replace_with_html()`, etc.).

## Key API Translations Applied

- `Html::parse_document(html)` → `Document::from(html)`
- `Selector::parse(sel)?` → inline in `doc.select(sel)` (no pre-parsing needed)
- `element.select(&selector).next()` → `selection.length() > 0` check
- `element.value().attr("x")` → `selection.attr("x")`
- `element.text().collect::<String>()` → `selection.text().to_string()`
- `document.root_element().text().collect::<Vec<_>>().join(" ")` → `document.text().to_string()`

## Migration Status Summary

### Completed Migrations
✅ Wave 1 (Cargo.toml and dom modules):
- src/dom/brs.rs
- src/dom/cleaners.rs
- src/dom/scoring.rs

✅ Wave 2 (Extractors):
- src/extractors/select.rs
- src/extractors/fields.rs

✅ **Wave 3 (Formats and metadata - THIS WAVE):**
- **src/formats/mod.rs**
- **src/metadata_adapter.rs**
- **src/extractors/content.rs (cleanup only)**

### Partially Migrated
⚠️ src/extractors/content.rs
- Public API uses dom_query (extract_content_html, etc.)
- Internal transform system still uses scraper
- Will require DOM mutation approach to fully migrate

⚠️ src/client.rs
- Uses Document for main parsing
- Some internal operations still reference Html

### Not Yet Migrated
None - all target files have been migrated or partially migrated.

## Next Steps for Complete Migration

1. **Deep migration of content.rs transforms:**
   - Replace custom serialization with dom_query DOM mutation
   - Rewrite `apply_transforms()` to use `selection.rename()`, `selection.unwrap()`, etc.
   - Migrate heading and link rewriting to use DOM mutation

2. **Remove scraper dependency:**
   - After transform migration, remove `scraper = "0.25.0"` from Cargo.toml
   - Remove `use scraper::` imports from content.rs

3. **Fix behavioral test failures:**
   - Investigate HTML output differences in client tests
   - Update test expectations or adjust dom_query usage
   - Fix brs module test

## Notes

- Wave 3 focused on the "low-hanging fruit" - files with simple selector usage
- All migrated functions maintain full API compatibility
- No breaking changes to public interfaces
- The migration maintains the same functionality with cleaner, more maintainable code
- DOM mutation approach (used in `apply_default_clean()`) is the recommended pattern going forward
