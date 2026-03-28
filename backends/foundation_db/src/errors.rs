//! Error types for `foundation_db` storage operations.
//!
//! All error types use `derive_more::From` for automatic conversions
//! and manual `impl Display` for error messages.

use derive_more::From;

/// Main error type for storage operations.
///
/// Uses `derive_more::From` for automatic `From<T>` impls on nested
/// error variants. String-wrapping variants use `#[from(ignore)]`.
#[derive(From, Debug)]
pub enum StorageError {
    /// Backend-specific error.
    #[from(ignore)]
    Backend(String),

    /// Connection failed.
    #[from(ignore)]
    Connection(String),

    /// Key not found.
    #[from(ignore)]
    NotFound(String),

    /// Serialization error.
    #[from(ignore)]
    Serialization(String),

    /// Encryption error.
    #[from(ignore)]
    Encryption(String),

    /// Migration error.
    #[from(ignore)]
    Migration(String),

    /// Generic storage error.
    #[from(ignore)]
    Generic(String),

    /// SQL conversion error.
    #[from(ignore)]
    SqlConversion(String),

    /// I/O error during filesystem operations.
    Io(std::io::Error),

    /// JSON serialization/deserialization error.
    Json(serde_json::Error),

    /// Base64 decoding error.
    Base64(base64::DecodeError),

    /// Hex decoding error.
    Hex(hex::FromHexError),

    /// Turso error.
    #[cfg(feature = "turso")]
    Turso(turso::Error),

    /// libsql error.
    #[cfg(feature = "libsql")]
    Libsql(libsql::Error),
}

impl core::fmt::Display for StorageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Backend(s) => write!(f, "Backend error: {s}"),
            Self::Connection(s) => write!(f, "Connection failed: {s}"),
            Self::NotFound(s) => write!(f, "Key not found: {s}"),
            Self::Serialization(s) => write!(f, "Serialization error: {s}"),
            Self::Encryption(s) => write!(f, "Encryption error: {s}"),
            Self::Migration(s) => write!(f, "Migration error: {s}"),
            Self::Generic(s) => write!(f, "Storage error: {s}"),
            Self::SqlConversion(s) => write!(f, "SQL conversion error: {s}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Base64(e) => write!(f, "Base64 error: {e}"),
            Self::Hex(e) => write!(f, "Hex error: {e}"),
            #[cfg(feature = "turso")]
            Self::Turso(e) => write!(f, "Turso error: {e}"),
            #[cfg(feature = "libsql")]
            Self::Libsql(e) => write!(f, "libsql error: {e}"),
        }
    }
}

impl std::error::Error for StorageError {}

/// Result type alias for storage operations.
pub type StorageResult<T> = Result<T, StorageError>;
