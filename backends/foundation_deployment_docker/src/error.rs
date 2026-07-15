//! Error type for the Docker client.
//!
//! WHY: Docker API calls fail in a few distinct ways (transport down, non-2xx
//! response with a `{"message": ...}` body, JSON parse failure, missing socket).
//! Callers need to distinguish these to decide whether to retry, surface, or
//! treat as "not found".
//!
//! WHAT: [`DockerError`] — a `derive_more`-based enum wrapping the generated
//! `ApiError` plus Docker-specific cases.
//!
//! HOW: `From<ApiError>` maps generated request failures; `Display`/`Error` via
//! `derive_more`.

use crate::generated::shared::ApiError;

/// Errors returned by [`crate::client::DockerClient`] operations.
///
/// WHY: Provides a single, matchable error surface over the generated API and
/// the hand-written transport/setup layer.
#[derive(Debug, derive_more::Display)]
pub enum DockerError {
    /// The Docker daemon returned a non-2xx status.
    #[display("docker API error (status {status}): {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Message extracted from the daemon's `{"message": ...}` body, if any.
        message: String,
    },

    /// The request could not be built or sent (transport-level failure).
    #[display("docker transport error: {_0}")]
    Transport(String),

    /// A response body could not be parsed as the expected JSON shape.
    #[display("docker JSON parse error: {_0}")]
    JsonParse(String),

    /// The Docker daemon socket could not be resolved or is unavailable.
    #[display("docker unavailable: {_0}")]
    Unavailable(String),
}

impl std::error::Error for DockerError {}

impl From<ApiError> for DockerError {
    /// Map a generated [`ApiError`] into a [`DockerError`], extracting the
    /// daemon's `{"message": ...}` body for `HttpStatus` responses.
    fn from(e: ApiError) -> Self {
        match e {
            ApiError::HttpStatus { code, body, .. } => {
                let message = body
                    .as_deref()
                    .and_then(|b| {
                        serde_json::from_str::<serde_json::Value>(b)
                            .ok()
                            .and_then(|v| {
                                v.get("message")
                                    .and_then(|m| m.as_str())
                                    .map(str::to_string)
                            })
                    })
                    .or(body)
                    .unwrap_or_else(|| "(no body)".to_string());
                DockerError::Api { status: code, message }
            }
            ApiError::RequestBuildFailed(m) | ApiError::RequestSendFailed(m) => {
                DockerError::Transport(m)
            }
            ApiError::ParseFailed(m) => DockerError::JsonParse(m),
        }
    }
}
