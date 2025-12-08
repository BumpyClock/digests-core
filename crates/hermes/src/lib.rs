// ABOUTME: Main library entry point for the Hermes web content parser.
// ABOUTME: Re-exports the public API: Client, ClientBuilder, ParseResult, ParseError, ErrorCode, ContentType, Options.

//! Hermes - A web content parser for extracting article content from URLs.
//!
//! This crate provides functionality to fetch and parse web pages, extracting
//! clean article content, metadata, and converting to various output formats.
//!
//! # Example
//!
//! ```no_run
//! use digests_hermes::{Client, ParseError};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), ParseError> {
//!     let client = Client::builder().build();
//!     let result = client.parse("https://example.com/article").await?;
//!     println!("{}", result.format_markdown());
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod dom;
pub mod error;
pub mod extractors;
pub mod formats;
pub mod options;
pub mod resource;
pub mod result;

pub use crate::client::Client;
pub use crate::error::{ErrorCode, ParseError};
pub use crate::extractors::custom::{
    ContentExtractor, CustomExtractor, ExtractorRegistry, FieldExtractor, SelectorSpec,
    TransformSpec,
};
pub use crate::extractors::loader::load_builtin_registry;
pub use crate::options::{ClientBuilder, ContentType, Options};
pub use crate::result::{ParseResult, Result};
