//! Unified error type for the keychain (spec-57, F008).
//!
//! Matches Bitwarden's error shapes: OAuth-style `error`/`error_description`
//! for public API errors, structured `ErrorModel` for validation failures,
//! and internal variants for storage/crypto.

use serde::Serialize;

/// Typed result alias used throughout the crate.
pub type AppResult<T> = Result<T, AppError>;

/// The canonical error type. Every handler returns `AppResult<T>`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// 400 — validation failure, missing field, bad input.
    #[error("bad request: {0}")]
    BadRequest(String),

    /// 401 — missing or invalid auth token.
    #[error("unauthorized")]
    Unauthorized,

    /// 403 — authenticated but not permitted.
    #[error("forbidden")]
    Forbidden,

    /// 404 — resource not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// 409 — conflict (duplicate name, etc.).
    #[error("conflict: {0}")]
    Conflict(String),

    /// 429 — rate limited.
    #[error("too many requests")]
    RateLimited,

    /// 500 — internal error (storage, crypto, unexpected).
    #[error("internal: {0}")]
    Internal(String),
}

impl AppError {
    /// HTTP status code for this error.
    #[must_use]
    pub fn status(&self) -> u16 {
        match self {
            Self::BadRequest(_) => 400,
            Self::Unauthorized => 401,
            Self::Forbidden => 403,
            Self::NotFound(_) => 404,
            Self::Conflict(_) => 409,
            Self::RateLimited => 429,
            Self::Internal(_) => 500,
        }
    }

    /// Bitwarden-compatible JSON error body.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        let (error, description) = match self {
            Self::BadRequest(msg) => ("invalid_request", msg.clone()),
            Self::Unauthorized => ("invalid_token", "The access token is invalid".into()),
            Self::Forbidden => ("access_denied", "You do not have permission".into()),
            Self::NotFound(msg) => ("not_found", msg.clone()),
            Self::Conflict(msg) => ("conflict", msg.clone()),
            Self::RateLimited => ("rate_limited", "Too many requests".into()),
            Self::Internal(msg) => ("internal_error", msg.clone()),
        };
        serde_json::json!({
            "error": error,
            "error_description": description,
            "ErrorModel": {
                "Message": description,
                "Object": "error"
            }
        })
    }
}

/// Bitwarden validation error model — returned as part of 400 responses
/// when field-level validation fails.
#[derive(Debug, Serialize)]
pub struct ValidationError {
    pub message: String,
    pub object: String,
}

impl ValidationError {
    #[must_use]
    pub fn new(message: impl Into<String>, object: impl Into<String>) -> Self {
        Self { message: message.into(), object: object.into() }
    }
}
