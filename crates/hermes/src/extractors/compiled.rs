// ABOUTME: Pre-compiled CSS selector cache for O(1) selector lookup.
// ABOUTME: Eliminates repeated parsing of CSS selectors in hot paths.

//! Selector caching for efficient repeated DOM queries.
//!
//! CSS selector parsing is expensive relative to the actual DOM matching.
//! This module provides a thread-safe cache that compiles selectors once
//! and reuses them for all subsequent queries.

use std::collections::HashMap;
use std::sync::RwLock;

use dom_query::Matcher;
use once_cell::sync::Lazy;

/// Thread-safe cache of compiled CSS selectors.
///
/// Uses a RwLock for efficient read-heavy workloads: most accesses are cache hits
/// (reads), with occasional cache misses requiring writes.
static SELECTOR_CACHE: Lazy<RwLock<HashMap<String, Option<Matcher>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Gets or compiles a CSS selector, caching the result.
///
/// Returns `Some(Matcher)` if the selector is valid, `None` if invalid.
/// Subsequent calls with the same selector string return the cached result.
///
/// # Thread Safety
///
/// This function is safe to call from multiple threads. Cache reads use a
/// shared lock; cache writes use an exclusive lock.
pub fn get_or_compile(css: &str) -> Option<Matcher> {
    // Fast path: check read lock for cached value
    {
        let cache = SELECTOR_CACHE.read().unwrap();
        if let Some(cached) = cache.get(css) {
            return cached.clone();
        }
    }

    // Slow path: compile and cache
    let compiled = Matcher::new(css).ok();
    let mut cache = SELECTOR_CACHE.write().unwrap();
    // Double-check after acquiring write lock (another thread may have inserted)
    if let Some(cached) = cache.get(css) {
        return cached.clone();
    }
    cache.insert(css.to_string(), compiled.clone());
    compiled
}

/// Precompiles a batch of selectors into the cache.
///
/// Call this during initialization (e.g., after loading the extractor registry)
/// to warm the cache and avoid lock contention during extraction.
pub fn precompile_selectors<I, S>(selectors: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut cache = SELECTOR_CACHE.write().unwrap();
    for css in selectors {
        let css = css.as_ref();
        if !cache.contains_key(css) {
            let compiled = Matcher::new(css).ok();
            cache.insert(css.to_string(), compiled);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_selector_is_cached() {
        let matcher = get_or_compile("div.container");
        assert!(matcher.is_some());

        // Second call should return cached value
        let matcher2 = get_or_compile("div.container");
        assert!(matcher2.is_some());
    }

    #[test]
    fn test_invalid_selector_returns_none() {
        let matcher = get_or_compile("[[[invalid");
        assert!(matcher.is_none());

        // Invalid selectors are also cached (as None)
        let matcher2 = get_or_compile("[[[invalid");
        assert!(matcher2.is_none());
    }

    #[test]
    fn test_precompile_selectors() {
        let selectors = vec!["h1", "h2", "p.intro", "a[href]"];
        precompile_selectors(selectors);

        // All should be cached
        assert!(get_or_compile("h1").is_some());
        assert!(get_or_compile("h2").is_some());
        assert!(get_or_compile("p.intro").is_some());
        assert!(get_or_compile("a[href]").is_some());
    }
}
