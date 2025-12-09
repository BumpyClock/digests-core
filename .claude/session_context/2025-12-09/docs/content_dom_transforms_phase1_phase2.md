# Content.rs DOM Transform Helpers - Phase 1 + Phase 2 Implementation

## Summary
Successfully implemented Phase 1 + Phase 2 of the content.rs refactoring by adding new dom_query-based transform helper functions. The new code provides a clean, mutation-based alternative to the existing scraper-based transform system.

## Files Modified
- `/Users/adityasharma/Projects/digests-core/crates/hermes/src/extractors/content.rs`

## Changes Made

### 1. Added `apply_transform_to_selection()` (lines 847-871)
A helper function that applies a single `TransformSpec` to a `Selection` using dom_query's mutation API.

**Supported Transforms:**
- `TransformSpec::Tag { value }` - Renames element using `sel.rename()`
- `TransformSpec::Noop` - Does nothing
- `TransformSpec::NoscriptToDiv` - Renames to "div" using `sel.rename()`
- `TransformSpec::Unwrap` - Removes element but keeps children using `sel.replace_with_html()`
- `TransformSpec::MoveAttr { from, to }` - Copies attribute value using `sel.attr()` and `sel.set_attr()`
- `TransformSpec::SetAttr { name, value }` - Sets attribute using `sel.set_attr()`

### 2. Added `handle_noscript_special_case()` (lines 873-888)
Handles the special Verge/Vox pattern where a `<noscript>` containing a single `<img>` child should be wrapped in a `<span>`.

**Logic:**
- Selects all noscript elements matching the selector
- Checks if each has exactly one element child
- If that child is an `<img>`, wraps it in `<span>` using `replace_with_html()`

### 3. Added `apply_transforms_dom()` (lines 890-916)
The main entry point for applying transforms using dom_query mutation. This function replaces the scraper-based `apply_transforms()`.

**Features:**
- Early return if no transforms
- Handles noscript special case first
- Collects nodes into Vec to avoid mutation-during-iteration issues
- Creates Selection for each node and applies the transform
- Returns transformed HTML as string

**Key Design Decision:**
The function collects all matching nodes into a Vec before applying transforms to avoid potential issues with mutating the DOM while iterating over it.

## Compilation Status
✅ The functions I added to `content.rs` compile successfully

**Note:** There are compilation errors in `crates/hermes/src/dom/cleaners.rs` (unrelated to my changes):
- `strip_unlikely()` and `clean_conditionally()` are missing a third parameter
- These errors existed before my changes and are not caused by my implementation

**Functions Usage:**
The three helper functions I added are now being used by additional code that was added after my implementation:
- `apply_transform_to_selection()` - Used by `apply_filters_and_transforms_unified()` (line 946)
- `handle_noscript_special_case()` - Used by both `apply_transforms_dom()` (line 905) and `apply_filters_and_transforms_unified()` (line 940)
- `apply_transforms_dom()` - Available for use by integration code

## Type Corrections Made
Fixed several type mismatches during implementation:
1. `replace_with_html()` expects `String`, not `&String` - removed reference
2. `set_attr()` expects `&str` - added proper borrowing with intermediate variable

## Technical Notes

### Why Collect Nodes First?
The pattern of collecting nodes before mutation:
```rust
let nodes: Vec<_> = doc.select(selector_str).nodes().iter().cloned().collect();
for node in nodes {
    let sel = Selection::from(node);
    apply_transform_to_selection(&sel, transform);
}
```

This prevents potential issues where:
1. Mutating the DOM while iterating could invalidate iterators
2. Transform operations like `Unwrap` modify the tree structure
3. Collecting first ensures all intended nodes are transformed

### Noscript Special Case
The special case handling for noscript was extracted into its own function for clarity. This matches the existing behavior in `serialize_node_with_transforms()` (lines 876-890) where a noscript containing a single img child is wrapped in a span.

## Old Functions Preserved
As requested, the old scraper-based functions remain untouched:
- `apply_transforms()` (line 815)
- `serialize_node_with_transforms()` (line 918)

## Next Steps (For Integration Agent)
1. Update `apply_filters_and_transforms_legacy()` to call `apply_transforms_dom()` instead of `apply_transforms()`
2. Run integration tests to ensure behavior matches
3. Consider adding unit tests specifically for the new functions
4. Eventually remove old scraper-based functions once fully migrated

## Testing Notes
The existing test suite should continue to pass since the new functions are not yet wired up. When they are integrated, tests should verify:
- Tag renaming works correctly
- Unwrap removes parent but keeps children
- MoveAttr and SetAttr modify attributes correctly
- Noscript special case preserves single img child in span
- Multiple transforms can be applied in sequence

## Additional Code Added After Implementation
After my implementation was complete, additional code was added that builds on the helper functions I created:

### `apply_filters_and_transforms_unified()` (lines 919-966)
A unified pipeline that consolidates all processing steps into a single parse-mutate-serialize cycle. This function:
- Parses HTML once
- Applies transforms using `apply_transform_to_selection()` and `handle_noscript_special_case()`
- Applies default cleaning using `apply_default_clean_to_doc()`
- Removes elements matching clean selectors
- Applies post-cleaners
- Serializes once

This is a more efficient approach than the legacy code which parses and serializes multiple times.

### Supporting Functions (lines 968-1037)
- `apply_default_clean_to_doc()` - Applies default cleaning to a Document in-place
- `collapse_consecutive_brs_doc()` - Collapses consecutive `<br>` tags
- `remove_empty_paragraphs_doc()` - Removes empty paragraphs
- `fix_headings_doc()` - Stub for fixing heading structure (TODO)
- `rewrite_empty_links_doc()` - Stub for rewriting empty links (TODO)

## Location
My implementation: lines 847-916 in `/Users/adityasharma/Projects/digests-core/crates/hermes/src/extractors/content.rs`
Additional code: lines 919-1037
