// ABOUTME: Loader module for custom extractor registries from embedded JSON data.
// ABOUTME: Provides load_builtin_registry() to initialize the default ExtractorRegistry.

//! Custom extractor registry loader.
//!
//! This module provides functions to load custom extractors from embedded JSON data
//! and build an `ExtractorRegistry` for domain-specific content extraction.

use once_cell::sync::Lazy;

use crate::extractors::custom::{CustomExtractor, ExtractorRegistry, SelectorSpec, TransformSpec};

/// Embedded JSON containing the full corpus of custom extractors.
const BUILTIN_EXTRACTORS_JSON: &str = include_str!("../../data/custom_extractors_full.json");

/// Lazily initialized builtin registry. Parsed once on first access, then cached.
static BUILTIN_REGISTRY: Lazy<ExtractorRegistry> = Lazy::new(|| {
    let extractors: Vec<CustomExtractor> =
        serde_json::from_str(BUILTIN_EXTRACTORS_JSON).expect("failed to parse builtin extractors");

    let mut registry = ExtractorRegistry::new();
    for mut extractor in extractors {
        post_process_transforms(&mut extractor);
        registry.register(extractor);
    }
    registry
});

/// Loads the builtin extractor registry from embedded JSON.
///
/// Returns a clone of the cached registry. The first call parses the embedded
/// JSON and caches the result; subsequent calls return a clone in O(1) time
/// (amortized over many extractions per clone).
///
/// # Panics
///
/// Panics if the embedded JSON is malformed or cannot be deserialized (on first call only).
pub fn load_builtin_registry() -> ExtractorRegistry {
    BUILTIN_REGISTRY.clone()
}

/// Post-processes an extractor's transforms to convert Noop variants to concrete
/// behaviors based on selector string heuristics.
///
/// Heuristics applied:
/// - Selector contains "noscript": NoscriptToDiv
/// - Selector equals "img" or contains "figure img" or "img.lazy" or "amp-img": MoveAttr data-src -> src
/// - Selector contains "source": MoveAttr data-srcset -> srcset, then data-src -> src
/// - Selector contains "video" (for data-src attrs): MoveAttr data-src -> src
/// - Selector contains "a" and "data-href": MoveAttr data-href -> href
fn post_process_transforms(extractor: &mut CustomExtractor) {
    if let Some(ref mut content) = extractor.content {
        let transforms = &mut content.transforms;
        let selectors: Vec<String> = transforms.keys().cloned().collect();

        for selector in selectors {
            if let Some(transform) = transforms.get(&selector) {
                if !matches!(transform, TransformSpec::Noop) {
                    continue;
                }

                let new_transform = infer_transform_from_selector(&selector);
                if !matches!(new_transform, TransformSpec::Noop) {
                    transforms.insert(selector, new_transform);
                }
            }
        }

        // If content selectors are missing/empty, provide a sensible default so extraction works.
        if content.field.selectors.is_empty() {
            content.field.selectors = vec![
                SelectorSpec::Css("div.c-entry-content".to_string()),
                SelectorSpec::Css("div.entry-content".to_string()),
                SelectorSpec::Css("article".to_string()),
                SelectorSpec::Css("main".to_string()),
            ];
        }
    }
}

/// Infers a concrete transform from selector string heuristics.
fn infer_transform_from_selector(selector: &str) -> TransformSpec {
    let selector_lower = selector.to_lowercase();

    // Selector contains "noscript": NoscriptToDiv
    if selector_lower.contains("noscript") {
        return TransformSpec::NoscriptToDiv;
    }

    // Selector equals "img" or contains "figure img": MoveAttr data-src -> src
    if selector == "img" || selector_lower.contains("figure img") {
        return TransformSpec::MoveAttr {
            from: "data-src".to_string(),
            to: "src".to_string(),
        };
    }

    // Selector contains "source": MoveAttr data-srcset -> srcset (primary), data-src -> src
    // We can only apply one transform per selector, so prioritize data-srcset -> srcset
    if selector_lower.contains("source") {
        return TransformSpec::MoveAttr {
            from: "data-srcset".to_string(),
            to: "srcset".to_string(),
        };
    }

    // Selector contains "video": MoveAttr data-src -> src
    if selector_lower.contains("video") {
        return TransformSpec::MoveAttr {
            from: "data-src".to_string(),
            to: "src".to_string(),
        };
    }

    // Selectors with "img" in them (like "img.lazyload", "amp-img"): MoveAttr data-src -> src
    if selector_lower.contains("img") {
        return TransformSpec::MoveAttr {
            from: "data-src".to_string(),
            to: "src".to_string(),
        };
    }

    // Lazy/original/zoom data attributes
    if selector_lower.contains("lazy")
        || selector_lower.contains("original")
        || selector_lower.contains("zoom")
    {
        return TransformSpec::MoveAttr {
            from: "data-original".to_string(),
            to: "src".to_string(),
        };
    }

    // Anchor data-href -> href
    if selector_lower.contains("a") && selector_lower.contains("data-href") {
        return TransformSpec::MoveAttr {
            from: "data-href".to_string(),
            to: "href".to_string(),
        };
    }

    TransformSpec::Noop
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_builtin_registry_succeeds() {
        let registry = load_builtin_registry();
        assert!(!registry.is_empty());
    }

    #[test]
    fn builtin_registry_has_over_100_extractors() {
        let registry = load_builtin_registry();
        assert!(
            registry.len() > 100,
            "expected over 100 extractors, got {}",
            registry.len()
        );
    }

    #[test]
    fn builtin_registry_contains_nytimes() {
        let registry = load_builtin_registry();
        let extractor = registry.get("www.nytimes.com");
        assert!(extractor.is_some(), "www.nytimes.com extractor not found");
        let ext = extractor.unwrap();
        assert_eq!(ext.domain, "www.nytimes.com");
        assert!(ext.title.is_some());
        assert!(ext.content.is_some());
    }

    #[test]
    fn builtin_registry_contains_medium() {
        let registry = load_builtin_registry();
        let extractor = registry.get("medium.com");
        assert!(extractor.is_some(), "medium.com extractor not found");
        let ext = extractor.unwrap();
        assert_eq!(ext.domain, "medium.com");
        assert!(ext.title.is_some());
        assert!(ext.content.is_some());
    }

    #[test]
    fn builtin_registry_contains_theguardian() {
        let registry = load_builtin_registry();
        let extractor = registry.get("www.theguardian.com");
        assert!(
            extractor.is_some(),
            "www.theguardian.com extractor not found"
        );
        let ext = extractor.unwrap();
        assert_eq!(ext.domain, "www.theguardian.com");
        assert!(ext.title.is_some());
        assert!(ext.content.is_some());
    }

    #[test]
    fn medium_img_transform_is_move_attr_after_post_process() {
        let registry = load_builtin_registry();
        let extractor = registry.get("medium.com");
        assert!(extractor.is_some(), "medium.com extractor not found");
        let ext = extractor.unwrap();
        let content = ext
            .content
            .as_ref()
            .expect("medium.com should have content");
        let img_transform = content.transforms.get("img");
        assert!(
            img_transform.is_some(),
            "medium.com should have img transform"
        );
        let transform = img_transform.unwrap();
        assert!(
            matches!(transform, TransformSpec::MoveAttr { from, to } if from == "data-src" && to == "src"),
            "img transform should be MoveAttr data-src -> src, got {:?}",
            transform
        );
    }

    #[test]
    fn infer_transform_noscript_to_div() {
        let transform = infer_transform_from_selector("noscript");
        assert!(matches!(transform, TransformSpec::NoscriptToDiv));

        let transform = infer_transform_from_selector("div noscript.lazy");
        assert!(matches!(transform, TransformSpec::NoscriptToDiv));
    }

    #[test]
    fn infer_transform_img_to_move_attr() {
        let transform = infer_transform_from_selector("img");
        assert!(
            matches!(transform, TransformSpec::MoveAttr { ref from, ref to } if from == "data-src" && to == "src")
        );

        let transform = infer_transform_from_selector("img.lazyload");
        assert!(
            matches!(transform, TransformSpec::MoveAttr { ref from, ref to } if from == "data-src" && to == "src")
        );

        let transform = infer_transform_from_selector("figure img");
        assert!(
            matches!(transform, TransformSpec::MoveAttr { ref from, ref to } if from == "data-src" && to == "src")
        );
    }

    #[test]
    fn infer_transform_source_to_srcset() {
        let transform = infer_transform_from_selector("source");
        assert!(
            matches!(transform, TransformSpec::MoveAttr { ref from, ref to } if from == "data-srcset" && to == "srcset")
        );

        let transform = infer_transform_from_selector("picture source");
        assert!(
            matches!(transform, TransformSpec::MoveAttr { ref from, ref to } if from == "data-srcset" && to == "srcset")
        );
    }

    #[test]
    fn infer_transform_video_to_move_attr() {
        let transform = infer_transform_from_selector("video");
        assert!(
            matches!(transform, TransformSpec::MoveAttr { ref from, ref to } if from == "data-src" && to == "src")
        );

        let transform = infer_transform_from_selector("video.player");
        assert!(
            matches!(transform, TransformSpec::MoveAttr { ref from, ref to } if from == "data-src" && to == "src")
        );
    }

    #[test]
    fn infer_transform_unknown_stays_noop() {
        let transform = infer_transform_from_selector(".embed-twitter");
        assert!(matches!(transform, TransformSpec::Noop));

        let transform = infer_transform_from_selector("iframe");
        assert!(matches!(transform, TransformSpec::Noop));
    }
}
