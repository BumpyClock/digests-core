// ABOUTME: Error types for the Hermes parser including ErrorCode enum and ParseError struct.
// ABOUTME: Provides categorized errors with convenience constructors and boolean helpers.

use std::fmt;

/// Error codes representing different categories of parse failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidUrl,
    Fetch,
    Timeout,
    Ssrf,
    Extract,
    Context,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ErrorCode::InvalidUrl => "invalid URL",
            ErrorCode::Fetch => "fetch error",
            ErrorCode::Timeout => "timeout",
            ErrorCode::Ssrf => "SSRF blocked",
            ErrorCode::Extract => "extraction error",
            ErrorCode::Context => "context cancelled",
        };
        write!(f, "{}", s)
    }
}

/// The main error type for parse operations.
#[derive(Debug, thiserror::Error)]
pub struct ParseError {
    pub code: ErrorCode,
    pub url: String,
    pub op: String,
    #[source]
    pub source: Option<anyhow::Error>,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hermes: {} {}: {}", self.op, self.url, self.code)?;
        if let Some(ref src) = self.source {
            write!(f, ": {}", src)?;
        }
        Ok(())
    }
}

impl ParseError {
    /// Create an InvalidUrl error.
    pub fn invalid_url(
        url: impl Into<String>,
        op: impl Into<String>,
        source: Option<anyhow::Error>,
    ) -> Self {
        Self {
            code: ErrorCode::InvalidUrl,
            url: url.into(),
            op: op.into(),
            source,
        }
    }

    /// Create a Fetch error.
    pub fn fetch(
        url: impl Into<String>,
        op: impl Into<String>,
        source: Option<anyhow::Error>,
    ) -> Self {
        Self {
            code: ErrorCode::Fetch,
            url: url.into(),
            op: op.into(),
            source,
        }
    }

    /// Create a Timeout error.
    pub fn timeout(
        url: impl Into<String>,
        op: impl Into<String>,
        source: Option<anyhow::Error>,
    ) -> Self {
        Self {
            code: ErrorCode::Timeout,
            url: url.into(),
            op: op.into(),
            source,
        }
    }

    /// Create an SSRF error.
    pub fn ssrf(
        url: impl Into<String>,
        op: impl Into<String>,
        source: Option<anyhow::Error>,
    ) -> Self {
        Self {
            code: ErrorCode::Ssrf,
            url: url.into(),
            op: op.into(),
            source,
        }
    }

    /// Create an Extract error.
    pub fn extract(
        url: impl Into<String>,
        op: impl Into<String>,
        source: Option<anyhow::Error>,
    ) -> Self {
        Self {
            code: ErrorCode::Extract,
            url: url.into(),
            op: op.into(),
            source,
        }
    }

    /// Create a Context error.
    pub fn context(
        url: impl Into<String>,
        op: impl Into<String>,
        source: Option<anyhow::Error>,
    ) -> Self {
        Self {
            code: ErrorCode::Context,
            url: url.into(),
            op: op.into(),
            source,
        }
    }

    /// Create a not-implemented error (Extract code with empty url).
    pub fn not_implemented(op: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Extract,
            url: String::new(),
            op: op.into(),
            source: Some(anyhow::anyhow!("not implemented yet")),
        }
    }

    /// Returns true if this is a Timeout error.
    pub fn is_timeout(&self) -> bool {
        self.code == ErrorCode::Timeout
    }

    /// Returns true if this is an SSRF error.
    pub fn is_ssrf(&self) -> bool {
        self.code == ErrorCode::Ssrf
    }

    /// Returns true if this is a Fetch error.
    pub fn is_fetch(&self) -> bool {
        self.code == ErrorCode::Fetch
    }

    /// Returns true if this is an Extract error.
    pub fn is_extract(&self) -> bool {
        self.code == ErrorCode::Extract
    }

    /// Returns true if this is an InvalidUrl error.
    pub fn is_invalid_url(&self) -> bool {
        self.code == ErrorCode::InvalidUrl
    }

    /// Returns true if this is a Context error.
    pub fn is_context(&self) -> bool {
        self.code == ErrorCode::Context
    }
}
