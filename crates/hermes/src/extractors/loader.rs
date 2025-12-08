// ABOUTME: Loader module for custom extractor registries from embedded JSON data.
// ABOUTME: Provides load_builtin_registry() to initialize the default ExtractorRegistry.

//! Custom extractor registry loader.
//!
//! This module provides functions to load custom extractors from embedded JSON data
//! and build an `ExtractorRegistry` for domain-specific content extraction.

use crate::extractors::custom::{CustomExtractor, ExtractorRegistry};

/// Embedded JSON containing the full corpus of custom extractors.
const BUILTIN_EXTRACTORS_JSON: &str = include_str!("../../data/custom_extractors_full.json");

/// Loads the builtin extractor registry from embedded JSON.
///
/// Parses the embedded JSON file containing custom extractor definitions and
/// registers each extractor (including its supported domains) into a new registry.
///
/// # Panics
///
/// Panics if the embedded JSON is malformed or cannot be deserialized.
pub fn load_builtin_registry() -> ExtractorRegistry {
    let extractors: Vec<CustomExtractor> =
        serde_json::from_str(BUILTIN_EXTRACTORS_JSON).expect("failed to parse builtin extractors");

    let mut registry = ExtractorRegistry::new();
    for extractor in extractors {
        registry.register(extractor);
    }
    registry
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
}
