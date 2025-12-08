// ABOUTME: Configuration options for the Hermes parser including ContentType, Options, and ClientBuilder.
// ABOUTME: ClientBuilder provides a fluent API for constructing Client instances with custom settings.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use crate::client::Client;
use crate::extractors::custom::ExtractorRegistry;

/// The content type format for parsed output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentType {
    #[default]
    Html,
    Markdown,
    Text,
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ContentType::Html => "html",
            ContentType::Markdown => "markdown",
            ContentType::Text => "text",
        };
        write!(f, "{}", s)
    }
}

impl From<&str> for ContentType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "markdown" | "md" => ContentType::Markdown,
            "text" | "txt" => ContentType::Text,
            _ => ContentType::Html,
        }
    }
}

/// Configuration options for the Hermes client.
#[derive(Debug, Clone)]
pub struct Options {
    pub timeout: Duration,
    pub user_agent: String,
    pub allow_private_networks: bool,
    pub content_type: ContentType,
    pub http_client: Option<reqwest::Client>,
    pub headers: HashMap<String, String>,
    pub registry: Option<ExtractorRegistry>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            user_agent: "Hermes/1.0".to_string(),
            allow_private_networks: false,
            content_type: ContentType::Html,
            http_client: None,
            headers: HashMap::new(),
            registry: None,
        }
    }
}

/// Builder for constructing Client instances with custom configuration.
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    opts: Options,
}

impl ClientBuilder {
    /// Create a new ClientBuilder with default options.
    pub fn new() -> Self {
        Self {
            opts: Options::default(),
        }
    }

    /// Set the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.opts.timeout = timeout;
        self
    }

    /// Set the User-Agent header.
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.opts.user_agent = user_agent.into();
        self
    }

    /// Allow or disallow requests to private networks.
    pub fn allow_private_networks(mut self, allow: bool) -> Self {
        self.opts.allow_private_networks = allow;
        self
    }

    /// Set the content type for parsed output.
    pub fn content_type(mut self, content_type: ContentType) -> Self {
        self.opts.content_type = content_type;
        self
    }

    /// Use a custom HTTP client.
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.opts.http_client = Some(client);
        self
    }

    /// Add a custom header to all requests.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.opts.headers.insert(key.into(), value.into());
        self
    }

    /// Set a custom extractor registry.
    pub fn registry(mut self, reg: ExtractorRegistry) -> Self {
        self.opts.registry = Some(reg);
        self
    }

    /// Build the Client with the configured options.
    pub fn build(self) -> Client {
        Client::new(self.opts)
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
