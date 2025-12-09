# content.rs Migration to dom_query - Status

## Overview
Migration of `/Users/adityasharma/Projects/digests-core/crates/hermes/src/extractors/content.rs` from `scraper` to `dom_query` is partially complete.

## Completed Work

### 1. Public API Migration
- ✅ Updated function signatures for:
  - `extract_content_html()` - now accepts `&Document`
  - `extract_content_html_opts()` - now accepts `&Document`
  - `extract_content_first_html()` - now accepts `&Document`
  - `extract_content_raw_first_html()` - now accepts `&Document`

### 2. Domain Transform Helpers
- ✅ Migrated all domain-specific transform functions to use `Selection` instead of `scraper::ElementRef`:
  - `reddit_role_img_transform()`
  - `youtube_iframe_transform()`
  - `gawker_youtube_transform()`
  - `cnn_video_thumb()`
  - `embed_twitter_blockquote()`
  - `img_data_src_to_src()`
  - `wrap_tag_fn()` and all `wrap_tag_*()` functions
  - `latimes_trb_ar_la_transform()`
  - `natgeo_parsys_transform()`
  - `wrap_with_same_tag()`
  - `unwrap_keep_children_fn()`
  - `build_element_with_attr()`

### 3. Default Cleaning
- ✅ Rewrote `apply_default_clean()` to use dom_query DOM mutation instead of custom serialization
  - Uses `doc.select().remove()` for element removal
  - Directly manipulates DOM for BR collapsing
  - Removes empty paragraphs via DOM mutation

### 4. apply_domain_function_transforms
- ✅ Updated to use `Document::from()` instead of `Html::parse_fragment()`
- ✅ Created new `serialize_doc_with_replacements()` function that uses dom_query's `replace_with_html()` instead of custom serialization

## Remaining Work

### Critical - Still Uses scraper Types

1. **extract_inner_html_filtered()** (line 837)
   - Parameter: `element: &scraper::ElementRef` → needs to be `&Selection`
   - Uses: `Html::parse_fragment()` → should use `Document::from()`
   - Uses: `ego_tree::NodeId` → should use `dom_query::NodeId`
   - Custom serialization with `serialize_node_preserve()` → use DOM mutation

2. **apply_transforms()** (line 914)
   - Uses: `Html::parse_fragment()` → should use `Document::from()`
   - Uses: `Selector::parse()` → inline selectors in `doc.select()`
   - Uses: `ego_tree::NodeId` → should use `dom_query::NodeId`
   - Custom serialization with `serialize_node_with_transforms()` → use DOM mutation

3. **serialize_with_cleaning()** (line 709)
   - OBSOLETE - can be removed, replaced by new `apply_default_clean()`

4. **serialize_node_with_cleaning()** (line 721)
   - OBSOLETE - can be removed

5. **is_empty_paragraph()** (line 777)
   - Uses: `ego_tree::NodeRef<scraper::Node>` → should use Selection or be rewritten

6. **collect_node_ids()** (line 1063)
   - Uses: `scraper::ElementRef` → should use `Selection`
   - Uses: `ego_tree::NodeId` → should use `dom_query::NodeId`

7. **serialize_filtered()** (line 1078)
   - Uses: `Html` → should use `Document`
   - Uses: `ego_tree::NodeId` → should use `dom_query::NodeId`
   - Custom serialization → use DOM mutation

8. **serialize_node()** (line 1094)
   - Uses: `ego_tree::NodeRef<scraper::Node>` → use dom_query types
   - Custom serialization → use DOM mutation

9. **serialize_node_preserve()** (line 1178)
   - Uses: `ego_tree::NodeRef<scraper::Node>` → use dom_query types
   - OBSOLETE - can likely be removed

10. **serialize_node_with_transforms()** (line 910)
    - Uses: `ego_tree::NodeRef<scraper::Node>` → use dom_query types
    - Custom serialization → use DOM mutation

11. **fix_headings()** (line 1299)
    - Uses: `Html::parse_fragment()` → should use `Document::from()`
    - Uses: `Selector::parse()` → inline selectors
    - Uses: `ego_tree::NodeId` → should use `dom_query::NodeId`
    - Custom serialization → use DOM mutation

12. **serialize_node_demote_headings()** (line 1330)
    - Uses: `ego_tree::NodeRef<scraper::Node>` → use dom_query types
    - Custom serialization → use DOM mutation

13. **rewrite_empty_links()** (line 1388)
    - Uses: `Html::parse_fragment()` → should use `Document::from()`
    - Uses: `Selector::parse()` → inline selectors
    - Uses: `ego_tree::NodeId` → should use `dom_query::NodeId`
    - Custom serialization → use DOM mutation

14. **serialize_node_unwrap_links()** (line 1425)
    - Uses: `ego_tree::NodeRef<scraper::Node>` → use dom_query types
    - Custom serialization → use DOM mutation

15. **All tests** (lines 1487-2227)
    - Test setup uses `Html::parse_document()` → should use `Document::from()`
    - All need updating to use new API

## Migration Strategy

### Phase 1: DOM Mutation Approach (Recommended)
Instead of custom serialization, use dom_query's built-in DOM mutation:
- For transforms: Use `selection.rename(new_tag)`, `selection.unwrap()`, `selection.remove()`
- For noscript→div: Use `selection.replace_with_html()` with formatted HTML
- For attribute operations: Use `selection.set_attr()`, `selection.remove_attr()`

### Phase 2: Key Functions to Rewrite

```rust
fn extract_inner_html_filtered(
    element: &Selection,  // Changed from &scraper::ElementRef
    clean_selectors: &[String],  // Changed from &[Selector]
    transforms: &std::collections::HashMap<String, TransformSpec>,
    use_default_cleaner: bool,
    preserve_tags: bool,
) -> String {
    // Get inner HTML
    let inner_html = element.inner_html().to_string();

    // Apply transforms using DOM mutation
    let doc = Document::from(&inner_html);

    // ... apply transforms directly via DOM mutation ...

    doc.html().to_string()
}
```

### Phase 3: Testing
All tests need to be updated:
- Replace `Html::parse_document()` with `Document::from()`
- Update assertions to match new output format (dom_query may serialize slightly differently)

## Upstream Changes Needed

The calling code in `client.rs` also needs updates:
- Line 797: `Html::parse_document()` → `Document::from()`
- Line 956: `Html::parse_document()` → `Document::from()`
- All call sites passing `&Html` → pass `&Document`

## Files Affected
- `/Users/adityasharma/Projects/digests-core/crates/hermes/src/extractors/content.rs` (primary)
- `/Users/adityasharma/Projects/digests-core/crates/hermes/src/client.rs` (caller)

## Build Errors
Current compilation shows:
- client.rs expects `&Document` but gets `&Html` (needs Document::from() migration)
- Unused import warning for `CaseSensitivity` in content.rs (can be removed)
- Various type mismatches in client.rs due to API changes

## Estimated Effort
- Remaining serialization functions: ~4-6 hours
- Test updates: ~2-3 hours
- Integration testing: ~1-2 hours
- Total: ~7-11 hours

## Notes
- The file is 2227 lines with extensive custom HTML serialization
- Migration benefits: DOM mutation support, cleaner code, better maintainability
- Trade-off: Initial migration effort is significant due to custom serialization patterns
