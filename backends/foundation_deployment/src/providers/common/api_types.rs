//! Shared API types for all providers.
//!
//! WHY: Avoid duplicating `ApiError`, `ApiPending`, `ApiResponse`, `Empty`, and `Operation`
//!      in every provider. These types are generated identically across all providers.
//!
//! WHAT: Provider-agnostic error and response types used by all generated API clients.
//!
//! HOW: Re-export these from each provider's `shared` module for convenience.

use foundation_core::wire::simple_http::SimpleHeaders;
use serde::{Deserialize, Serialize};

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

/// Empty response body for endpoints that return `{}`.
#[derive(Debug, Clone, Serialize, Deserialize, foundation_macros::JsonHash)]
pub struct Empty {
    #[serde(flatten)]
    pub data: std::collections::HashMap<String, serde_json::Value>,
}

/// Operation metadata returned by long-running GCP operations.
#[derive(Debug, Clone, Serialize, Deserialize, foundation_macros::JsonHash)]
pub struct Operation {
    #[serde(flatten)]
    pub data: std::collections::HashMap<String, serde_json::Value>,
}
