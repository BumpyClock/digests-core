// ABOUTME: Content extraction strategies for different types of web pages.
// ABOUTME: Includes extractors for articles, metadata, and structured data.

//! Content extraction module.
//!
//! This module contains various extraction strategies for pulling content
//! from web pages, including article body extraction, metadata parsing,
//! and structured data (JSON-LD, OpenGraph, etc.) handling.
//!
//! Submodules:
//! - `compiled`: Pre-compiled CSS selector cache.
//! - `custom`: Custom site-specific extractors with configurable selectors.
//! - `select`: Selector-based field extraction utilities.

pub mod compiled;
pub mod content;
pub mod custom;
pub mod fields;
pub mod loader;
pub mod select;
