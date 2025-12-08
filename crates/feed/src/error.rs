// ABOUTME: Error types for feed parsing operations.
// ABOUTME: Provides FeedError enum with Parse, Invalid, and Empty variants.

use std::fmt;
use thiserror::Error;

/// Errors that can occur during feed parsing.
#[derive(Debug, Error)]
pub enum FeedError {
    /// Failed to parse the feed data (malformed XML/JSON).
    #[error("failed to parse feed: {0}")]
    Parse(String),

    /// The data was parsed but is not a valid feed (missing required fields).
    #[error("invalid feed: {0}")]
    Invalid(String),

    /// The feed contains no items.
    #[error("feed is empty: no items found")]
    Empty,
}

impl FeedError {
    /// Creates a Parse error from an underlying feed-rs error.
    pub fn parse(err: impl fmt::Display) -> Self {
        FeedError::Parse(err.to_string())
    }

    /// Creates an Invalid error with a custom message.
    pub fn invalid(msg: impl Into<String>) -> Self {
        FeedError::Invalid(msg.into())
    }
}
