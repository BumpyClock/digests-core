# Scoring.rs Migration from scraper to dom_query

## Summary

Successfully migrated `/Users/adityasharma/Projects/digests-core/crates/hermes/src/dom/scoring.rs` from the read-only `scraper` crate to the mutable `dom_query` crate. This migration enables DOM mutation capabilities while preserving all existing functionality and test coverage.

## Files Modified

### Primary Migration Files

1. **`/Users/adityasharma/Projects/digests-core/crates/hermes/Cargo.toml`**
   - Added `dom_query = "0.24.0"` dependency

2. **`/Users/adityasharma/Projects/digests-core/crates/hermes/src/dom/scoring.rs`**
   - Complete API migration from scraper to dom_query
   - Changed all function signatures to use `Document` and `Selection` from dom_query
   - Updated 13 unit tests to work with the new API
   - Exported helper functions `get_node_id` and `get_tag_name` as public functions

3. **`/Users/adityasharma/Projects/digests-core/crates/hermes/src/dom/cleaners.rs`**
   - Updated imports from `get_weight_dq` and `link_density_dq` to `get_weight` and `link_density`
   - All usages of scoring functions now use the unified API

4. **`/Users/adityasharma/Projects/digests-core/crates/hermes/src/dom/mod.rs`**
   - Added `get_node_id` and `get_tag_name` to public exports

5. **`/Users/adityasharma/Projects/digests-core/crates/hermes/src/client.rs`**
   - Updated `score_generic_content` function to use `Document::from()` instead of `Html::parse_document()`
   - Updated debug logging to use new helper functions

### Secondary Fixes

6. **`/Users/adityasharma/Projects/digests-core/crates/hermes/src/dom/brs.rs`**
   - Fixed lifetime issues in BR replacement logic caused by dom_query API differences
   - Refactored loop to avoid borrowing conflicts

## Key API Translations

| scraper API | dom_query API |
|------------|---------------|
| `Html::parse_document(html)` | `Document::from(html)` |
| `Selector::parse(sel)?` | Inline in `doc.select(sel)` |
| `doc.select(&selector)` | `doc.select(sel).iter()` |
| `element.value().name()` | Helper function `get_tag_name(&selection)` |
| `element.value().attr("class")` | `selection.attr("class")` |
| `element.text().collect::<String>()` | `selection.text()` |
| `element.id()` (NodeId) | `selection.nodes().first().map(\|node\| node.id)` |
| `element.parent().and_then(ElementRef::wrap)` | `get_parent(&selection)` helper |
| `element.html()` | `selection.html().to_string()` |

## Important Implementation Details

### NodeId Handling
- dom_query uses `NodeId` from its own crate (re-exported from the underlying tree structure)
- NodeId is accessed via `node.id` field, not a method
- Used as HashMap keys in `NodeScores` for tracking element scores

### Selection Iteration
- dom_query's `Selection` is not directly iterable
- Must call `.iter()` to get an iterator over matched elements
- Example: `doc.select("p").iter()` instead of `doc.select("p")`

### HTML Output
- `.html()` returns `Tendril<UTF8>` not `String`
- Must call `.to_string()` to convert to String
- Example: `selection.html().to_string()`

### First Element Access
- `.first()` on Selection returns a `Selection`, not `Option<Selection>`
- Check `.length() > 0` to verify if selection is non-empty
- Example: `let body = doc.select("body").first(); if body.length() > 0 { ... }`

### Lifetime Annotations
- Helper function `get_parent` requires explicit lifetime annotation
- Signature: `fn get_parent<'a>(selection: &Selection<'a>) -> Option<Selection<'a>>`

## Test Results

All 13 unit tests in the scoring module pass successfully:

- test_score_commas
- test_score_paragraph
- test_get_weight
- test_link_density
- test_has_sentence_end
- test_score_content
- test_extract_best_content
- test_find_top_candidate_respects_score_attrs
- test_find_top_candidate_skips_non_candidate_tags
- test_find_top_candidate_fallbacks_to_body
- test_merge_siblings_wraps_and_includes
- test_merge_siblings_filters_non_top_candidate_tags
- test_merge_siblings_paragraph_rules

## Backward Compatibility

- Deprecated wrapper functions `get_weight_dq` and `link_density_dq` are provided for temporary backward compatibility
- These will be removed in a future update once all code is confirmed to use the direct functions
- cleaners.rs has already been updated to use the direct functions

## Notes for Future Development

1. **scraper Still in Use**: The `scraper` crate is still used by other parts of the codebase (extractors, fields, etc.). Those modules will need separate migration efforts.

2. **dom_query Capabilities**: Now that scoring.rs uses dom_query, it's possible to add DOM mutation capabilities in the future if needed (e.g., modifying elements during scoring).

3. **Performance**: dom_query uses the same underlying HTML parser (html5ever) as scraper, so performance characteristics should be similar.

4. **BR Replacement**: The brs.rs module also required updates due to lifetime handling differences in dom_query. The refactored approach collects BRs first, then processes them to avoid borrow checker issues.

5. **Warnings**: One dead_code warning exists for `is_void_element` function in cleaners.rs - this is pre-existing and not related to the migration.

## Conclusion

The migration was successful with all tests passing. The scoring algorithm logic remains identical - only the API calls changed to use dom_query instead of scraper. The codebase is now better positioned for future enhancements that require DOM mutation capabilities.
