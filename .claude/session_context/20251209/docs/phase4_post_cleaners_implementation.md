# Phase 4: Post-Cleaners dom_query Implementation

## Summary
Successfully implemented dom_query-based versions of post-cleaner functions that work on `&Document` references with in-place mutation.

## Files Modified
- `/Users/adityasharma/Projects/digests-core/crates/hermes/src/extractors/content.rs`

## Implementation Details

### Functions Added (lines 1616-1660)

1. **`fix_headings_doc(doc: &Document)`** (lines 1609-1626)
   - Demotes all h1 elements except the first to h2
   - Uses dom_query's `Selection` and `replace_with_html()` for in-place mutation
   - Collects all h1 nodes into a Vec to avoid mutation-during-iteration issues
   - Uses string replacement on outer HTML to rename tags (following pattern from `dom/cleaners.rs`)

2. **`rewrite_empty_links_doc(doc: &Document)`** (lines 1633-1652)
   - Unwraps `<a>` tags with empty href or href="#"
   - Preserves inner content while removing the anchor tag
   - Only unwraps anchors that contain text content (skips empty anchors)
   - Uses `replace_with_html()` with inner HTML to perform unwrapping

3. **`apply_post_cleaners_doc(doc: &Document)`** (lines 1654-1660)
   - Wrapper function that applies both cleaners in sequence
   - Provides a single entry point for all post-cleaning operations

## Technical Approach

### DOM Mutation Strategy
- Collect nodes into a `Vec` before iteration to prevent concurrent modification issues
- Use `Selection::from(node)` to create selections from collected nodes
- Use `replace_with_html()` for tag renaming and unwrapping operations
- Pass `String` directly (not `&String`) to `replace_with_html()` for correct type

### Tag Renaming Pattern
```rust
let outer = sel.html().to_string();
let new_html = outer
    .replacen("<h1", "<h2", 1)
    .replacen("</h1>", "</h2>", 1);
sel.replace_with_html(new_html);
```
This pattern follows the approach used in `crates/hermes/src/dom/cleaners.rs` (lines 100-107).

### Unwrapping Pattern
```rust
let inner = sel.inner_html().to_string();
sel.replace_with_html(inner);
```

## Compilation Status

✅ **No errors in content.rs**
- The three new functions compile successfully
- No warnings generated
- Necessary imports (`Document`, `Selection`) already present

⚠️ **Unrelated errors exist in other files**
- `client.rs`: Missing argument in `find_top_candidate()` and `merge_siblings()` calls
- These are pre-existing issues from other migration phases
- Not related to Phase 4 implementation

## Design Decisions

1. **Private Functions**: All three functions are private (`fn` not `pub fn`) as they are internal helpers for the migration process

2. **No Modification of Existing Code**: Per requirements, did not modify existing `fix_headings()`, `rewrite_empty_links()`, or `apply_post_cleaners()` functions

3. **Naming Convention**: Added `_doc` suffix to clearly indicate these are the dom_query/Document versions

4. **Location**: Placed new functions immediately after the existing `apply_post_cleaners()` at line 1614, keeping related functionality together

## Next Steps

These functions are now ready to be integrated into the main extraction pipeline once the upstream functions (`extract_inner_html_filtered()`, `apply_transforms()`, etc.) are migrated to use `Document` instead of `Html`.

## Testing

The functions compile successfully with `cargo check --package digests-hermes`. Integration testing should be performed once they are wired into the main extraction flow.
