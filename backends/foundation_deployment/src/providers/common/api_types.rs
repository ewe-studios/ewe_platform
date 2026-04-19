//! Shared API types for all providers.
//!
//! WHY: Avoid duplicating `ApiError`, `ApiPending`, and `ApiResponse` in every provider.
//!
//! WHAT: Provider-agnostic error and response types used by all generated API clients.
//!
//! HOW: Re-export these from each provider's `shared` module for convenience.

use foundation_core::wire::simple_http::SimpleHeaders;

// Re-export types from foundation_core for convenience
pub use foundation_core::valtron::BoxedSendExecutionAction;
pub use foundation_core::wire::simple_http::client::RequestIntro;

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Provider-agnostic error type for API operations.
#[derive(Debug)]
pub enum ApiError {
    RequestBuildFailed(String),
    RequestSendFailed(String),
    HttpStatus {
        code: u16,
        headers: SimpleHeaders,
        body: Option<String>,
    },
    ParseFailed(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::RequestBuildFailed(e) => write!(f, "request build failed: {e}"),
            ApiError::RequestSendFailed(e) => write!(f, "request send failed: {e}"),
            ApiError::HttpStatus { code, body, .. } => {
                write!(f, "HTTP status {code}")?;
                if let Some(b) = body {
                    write!(f, ": {b}")?;
                }
                Ok(())
            }
            ApiError::ParseFailed(e) => write!(f, "parse failed: {e}"),
        }
    }
}

impl std::error::Error for ApiError {}

/// Progress states for API operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiPending {
    Building,
    Sending,
}

/// Generic API response with status, headers, and parsed body.
#[derive(Debug, Clone)]
pub struct ApiResponse<T> {
    pub status: u16,
    pub headers: SimpleHeaders,
    pub body: T,
}
