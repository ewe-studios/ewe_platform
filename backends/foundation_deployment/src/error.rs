//! Unified error types for the deployment system.
//!
//! WHY: A single error enum keeps provider-agnostic and provider-specific
//! failures in one place, so callers handle errors uniformly.
//!
//! WHAT: `DeploymentError` covers process failures, config issues, provider
//! detection, build/deploy failures, state store errors, HTTP errors, and
//! provider-specific API errors.
//!
//! HOW: Manual `Display` + `Error` impls. `From` conversions for common
//! upstream types (`std::io::Error`, `HttpClientError`).

/// Unified error type for deployment operations.
///
/// Provider-agnostic errors at the top level, provider-specific errors
/// nested inside dedicated variants.
#[derive(Debug)]
pub enum DeploymentError {
    /// A shelled-out process failed.
    ProcessFailed {
        command: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },

    /// Config file is invalid or missing.
    ConfigInvalid { file: String, reason: String },

    /// No provider detected in project directory.
    NoProviderDetected { project_dir: String },

    /// Build step failed.
    BuildFailed(String),

    /// Deployment was rejected (e.g. quota, permissions).
    DeployRejected { reason: String },

    /// State store operation failed.
    StateFailed(String),

    /// HTTP request to provider API failed.
    HttpError(foundation_core::wire::simple_http::HttpClientError),

    /// IO error.
    IoError(std::io::Error),

    /// `SQLite` / Turso error.
    SqliteError(String),

    /// Executor scheduling error.
    ExecutorError { reason: String },

    /// Cloudflare-specific error details.
    Cloudflare {
        status: u16,
        message: String,
        error_code: Option<String>,
    },

    /// GCP-specific error details.
    Gcp { status: u16, message: String },

    /// AWS-specific error details.
    Aws {
        status: u16,
        message: String,
        request_id: Option<String>,
    },

    /// IO error with path context.
    Io {
        path: String,
        source: std::io::Error,
    },

    /// JSON parsing failed.
    JsonInvalid {
        file: String,
        reason: String,
    },

    /// Generic error message.
    Generic(String),
}

impl std::error::Error for DeploymentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HttpError(e) => Some(e),
            Self::IoError(e) => Some(e),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl core::fmt::Display for DeploymentError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ProcessFailed {
                command,
                exit_code,
                stderr,
                ..
            } => {
                write!(
                    f,
                    "process '{command}' failed (exit {exit_code:?}): {stderr}"
                )
            }
            Self::ConfigInvalid { file, reason } => {
                write!(f, "invalid config '{file}': {reason}")
            }
            Self::NoProviderDetected { project_dir } => {
                write!(f, "no deployment provider detected in '{project_dir}'")
            }
            Self::BuildFailed(msg) => write!(f, "build failed: {msg}"),
            Self::DeployRejected { reason } => write!(f, "deploy rejected: {reason}"),
            Self::StateFailed(msg) => write!(f, "state error: {msg}"),
            Self::HttpError(err) => write!(f, "HTTP error: {err}"),
            Self::IoError(err) => write!(f, "IO error: {err}"),
            Self::SqliteError(msg) => write!(f, "SQLite error: {msg}"),
            Self::ExecutorError { reason } => write!(f, "executor error: {reason}"),
            Self::Cloudflare {
                status, message, ..
            } => {
                write!(f, "Cloudflare API error ({status}): {message}")
            }
            Self::Gcp { status, message } => {
                write!(f, "GCP API error ({status}): {message}")
            }
            Self::Aws {
                status, message, ..
            } => {
                write!(f, "AWS API error ({status}): {message}")
            }
            Self::Io { path, source } => {
                write!(f, "IO error at '{path}': {source}")
            }
            Self::JsonInvalid { file, reason } => {
                write!(f, "JSON parse error in '{file}': {reason}")
            }
            Self::Generic(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<std::io::Error> for DeploymentError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

impl From<foundation_core::wire::simple_http::HttpClientError> for DeploymentError {
    fn from(err: foundation_core::wire::simple_http::HttpClientError) -> Self {
        Self::HttpError(err)
    }
}

impl From<serde_json::Error> for DeploymentError {
    fn from(err: serde_json::Error) -> Self {
        Self::JsonInvalid {
            file: "unknown".to_string(),
            reason: err.to_string(),
        }
    }
}

impl From<foundation_db::StorageError> for DeploymentError {
    fn from(err: foundation_db::StorageError) -> Self {
        Self::StateFailed(err.to_string())
    }
}

impl DeploymentError {
    /// Create a generic error from a string.
    pub fn generic(msg: impl Into<String>) -> Self {
        Self::Generic(msg.into())
    }
}
