//! WHY: Centralized error type for all provider spec fetching operations.
//!
//! WHAT: Covers HTTP transport, JSON parsing, file I/O, and git operations.
//!
//! HOW: Uses `derive_more::From` for automatic conversions and
//! `derive_more::Display` for formatted error messages.

use derive_more::Display;

/// WHY: Centralized error type for all provider spec fetching operations.
///
/// WHAT: Covers HTTP transport, JSON parsing, file I/O, and git operations.
///
/// HOW: Uses `derive_more::Display` for formatted error messages.
/// Manual `From` implementations are provided where needed.
///
/// # Location
///
/// All errors MUST be defined in this file (`errors.rs`). Do NOT define
/// error types in provider-specific modules or other files.
#[derive(Debug, Display)]
pub enum SpecFetchError {
    /// HTTP transport error - wraps `HttpClientError` automatically.
    #[display("HTTP error for {provider}: {source}")]
    Http {
        provider: String,
        source: foundation_core::wire::simple_http::HttpClientError,
    },

    /// Server returned non-200 status.
    #[display("HTTP {status} from {provider}")]
    BadStatus { provider: String, status: u16 },

    /// JSON deserialization failed - wraps `serde_json::Error` automatically.
    #[display("JSON parse error for {provider}: {source}")]
    Json {
        provider: String,
        source: serde_json::Error,
    },

    /// SHA256 hash computation failed.
    #[display("SHA256 hash error for {provider}: {source}")]
    Hash {
        provider: String,
        source: std::io::Error,
    },

    /// Failed to write file to disk.
    #[display("failed to write {path}: {source}")]
    WriteFile {
        path: String,
        source: std::io::Error,
    },

    /// Git operation failed (clone, pull, commit, etc.).
    #[display("git operation failed for {repo}: {reason}")]
    Git {
        repo: String,
        reason: String,
    },

    /// Generic/unspecified error.
    #[display("generic error: {_0}")]
    Generic(String),
}

impl From<foundation_core::wire::simple_http::HttpClientError> for SpecFetchError {
    fn from(source: foundation_core::wire::simple_http::HttpClientError) -> Self {
        Self::Http {
            provider: "unknown".to_string(),
            source,
        }
    }
}

impl From<serde_json::Error> for SpecFetchError {
    fn from(source: serde_json::Error) -> Self {
        Self::Json {
            provider: "unknown".to_string(),
            source,
        }
    }
}

impl From<std::io::Error> for SpecFetchError {
    fn from(source: std::io::Error) -> Self {
        Self::WriteFile {
            path: "unknown".to_string(),
            source,
        }
    }
}

impl SpecFetchError {
    /// Create a generic error from a string.
    pub fn generic(msg: impl Into<String>) -> Self {
        Self::Generic(msg.into())
    }
}

impl Clone for SpecFetchError {
    fn clone(&self) -> Self {
        match self {
            SpecFetchError::Http { provider, source } => SpecFetchError::Http {
                provider: provider.clone(),
                source: foundation_core::wire::simple_http::HttpClientError::FailedWith(
                    source.to_string().into(),
                ),
            },
            SpecFetchError::BadStatus { provider, status } => SpecFetchError::BadStatus {
                provider: provider.clone(),
                status: *status,
            },
            SpecFetchError::Json { provider, source } => SpecFetchError::Json {
                provider: provider.clone(),
                source: serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    source.to_string(),
                )),
            },
            SpecFetchError::Hash { provider, source } => SpecFetchError::Hash {
                provider: provider.clone(),
                source: std::io::Error::new(source.kind(), source.to_string()),
            },
            SpecFetchError::WriteFile { path, source } => SpecFetchError::WriteFile {
                path: path.clone(),
                source: std::io::Error::new(source.kind(), source.to_string()),
            },
            SpecFetchError::Git { repo, reason } => SpecFetchError::Git {
                repo: repo.clone(),
                reason: reason.clone(),
            },
            SpecFetchError::Generic(msg) => SpecFetchError::Generic(msg.clone()),
        }
    }
}

impl std::error::Error for SpecFetchError {}
