// ABOUTME: DOM manipulation and traversal utilities for HTML parsing.
// ABOUTME: Provides Go-compatible readability scoring, content cleaning, and paragraph conversion.

//! DOM utilities for HTML document manipulation.
//!
//! This module provides helpers for traversing and manipulating HTML documents,
//! including Go-compatible readability scoring, sibling merging, and content cleaning.

pub mod brs;
pub mod cleaners;
pub mod scoring;

pub use brs::{brs_to_ps, rewrite_top_level};
pub use cleaners::{
    clean_article, is_empty_paragraph, is_unlikely_candidate, process_h1_tags,
    should_remove_header, should_remove_image,
};
pub use scoring::{
    compute_text_metrics, extract_best_content, find_top_candidate, get_node_id, get_tag_name,
    get_weight, has_sentence_end, link_density, link_density_cached, merge_siblings,
    normalize_spaces, score_content, NodeTextMetrics, TextMetricsMap,
};
