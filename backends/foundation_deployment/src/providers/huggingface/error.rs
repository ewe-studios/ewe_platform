//! Hugging Face Hub API error types.

use derive_more::From;
use std::fmt;

/// Hugging Face Hub API error types.
#[derive(Debug, From)]
pub enum HuggingFaceError {
    /// HTTP error with status, URL, and response body.
    Http {
        status: u16,
        url: String,
        body: String,
    },

    /// Authentication required (401).
    AuthRequired,

    /// Repository not found (404 on repo endpoint).
    RepoNotFound { repo_id: String },

    /// Revision not found (404 on revision endpoint).
    RevisionNotFound { repo_id: String, revision: String },

    /// File not found (404 on file endpoint).
    FileNotFound { path: String, repo_id: String },

    /// Invalid repository type.
    InvalidRepoType {
        expected: crate::providers::huggingface::types::RepoType,
        actual: crate::providers::huggingface::types::RepoType,
    },

    /// Invalid parameter.
    InvalidParameter(String),

    /// Generic backend error (wraps other errors).
    #[from(ignore)]
    Backend(String),

    /// Valtron execution error.
    #[from(ignore)]
    Valtron(String),

    /// I/O error.
    Io(std::io::Error),

    /// JSON parse error.
    Json(serde_json::Error),

    /// HTTP parse error.
    HttpParse(http::Error),
}

impl fmt::Display for HuggingFaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HuggingFaceError::Http { status, url, body } => {
                write!(f, "HTTP error: {} {} - {}", status, url, body)
            }
            HuggingFaceError::AuthRequired => write!(f, "Authentication required"),
            HuggingFaceError::RepoNotFound { repo_id } => {
                write!(f, "Repository not found: {}", repo_id)
            }
            HuggingFaceError::RevisionNotFound { repo_id, revision } => {
                write!(f, "Revision not found: {} in {}", revision, repo_id)
            }
            HuggingFaceError::FileNotFound { path, repo_id } => {
                write!(f, "File not found: {} in {}", path, repo_id)
            }
            HuggingFaceError::InvalidRepoType { expected, actual } => {
                write!(f, "Invalid repository type: expected {}, got {}", expected, actual)
            }
            HuggingFaceError::InvalidParameter(msg) => {
                write!(f, "Invalid parameter: {}", msg)
            }
            HuggingFaceError::Backend(msg) => write!(f, "Backend error: {}", msg),
            HuggingFaceError::Valtron(msg) => write!(f, "Valtron error: {}", msg),
            HuggingFaceError::Io(e) => write!(f, "I/O error: {}", e),
            HuggingFaceError::Json(e) => write!(f, "JSON error: {}", e),
            HuggingFaceError::HttpParse(e) => write!(f, "HTTP parse error: {}", e),
        }
    }
}

impl std::error::Error for HuggingFaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HuggingFaceError::Io(e) => Some(e),
            HuggingFaceError::Json(e) => Some(e),
            HuggingFaceError::HttpParse(e) => Some(e),
            _ => None,
        }
    }
}

/// Result type for Hugging Face operations.
pub type Result<T> = std::result::Result<T, HuggingFaceError>;
