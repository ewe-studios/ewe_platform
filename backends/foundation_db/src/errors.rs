//! Error types for foundation_db storage operations.

use derive_more::Display;

/// Main error type for storage operations.
#[derive(Debug, Display)]
pub enum StorageError {
    /// Backend-specific error.
    #[display("Backend error: {_0}")]
    Backend(String),

    /// Connection failed.
    #[display("Connection failed: {_0}")]
    Connection(String),

    /// Key not found.
    #[display("Key not found: {_0}")]
    NotFound(String),

    /// Serialization error.
    #[display("Serialization error: {_0}")]
    Serialization(String),

    /// Encryption error.
    #[display("Encryption error: {_0}")]
    Encryption(String),

    /// Migration error.
    #[display("Migration error: {_0}")]
    Migration(String),

    /// Generic storage error.
    #[display("Storage error: {_0}")]
    Generic(String),

    /// libsql error.
    #[display("libsql error: {_0}")]
    Libsql(libsql::Error),

    /// std io error.
    #[display("IO error: {_0}")]
    Io(std::io::Error),

    /// JSON error.
    #[display("JSON error: {_0}")]
    Json(serde_json::Error),

    /// Conversion error for base64.
    #[display("Base64 error: {_0}")]
    Base64(base64::DecodeError),

    /// Hex conversion error.
    #[display("Hex error: {_0}")]
    Hex(hex::FromHexError),
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StorageError::Libsql(e) => Some(e),
            StorageError::Io(e) => Some(e),
            StorageError::Json(e) => Some(e),
            StorageError::Base64(e) => Some(e),
            StorageError::Hex(e) => Some(e),
            _ => None,
        }
    }
}

impl From<libsql::Error> for StorageError {
    fn from(e: libsql::Error) -> Self {
        StorageError::Libsql(e)
    }
}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        StorageError::Io(e)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        StorageError::Json(e)
    }
}

impl From<base64::DecodeError> for StorageError {
    fn from(e: base64::DecodeError) -> Self {
        StorageError::Base64(e)
    }
}

impl From<hex::FromHexError> for StorageError {
    fn from(e: hex::FromHexError) -> Self {
        StorageError::Hex(e)
    }
}

impl From<chacha20poly1305::Error> for StorageError {
    fn from(e: chacha20poly1305::Error) -> Self {
        StorageError::Encryption(e.to_string())
    }
}

/// Result type alias for storage operations.
pub type StorageResult<T> = Result<T, StorageError>;
