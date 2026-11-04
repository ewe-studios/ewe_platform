//! Error types for `foundation_db` storage operations.
//!
//! All error types use `derive_more::From` for automatic conversions
//! and manual `impl Display` for error messages.

use derive_more::From;

/// Which side of a store's read/write connection lock a call wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    /// Shared: reads and read-only raw statements.
    Read,
    /// Exclusive: writes, raw statements that may write, and batches.
    Write,
}

impl core::fmt::Display for LockMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Read => f.write_str("read"),
            Self::Write => f.write_str("write"),
        }
    }
}

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

    /// Deserialization error.
    #[from(ignore)]
    Deserialization(String),

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

    /// A conditional write's precondition failed (e.g. `If-None-Match: *`
    /// against an existing object, or a stale `If-Match` `ETag`).
    #[from(ignore)]
    PreconditionFailed(String),

    /// A store's connection lock was not free in time. See `lock_timeout` on
    /// the store: a task that holds a row stream and then calls another
    /// operation on the same store waits for itself, and gets this error
    /// instead of hanging.
    #[from(ignore)]
    LockTimeout {
        /// Which side of the lock was wanted.
        mode: LockMode,
        /// How long the call waited.
        waited: std::time::Duration,
    },

    /// I/O error during filesystem operations.
    Io(std::io::Error),

    /// JSON serialization/deserialization error.
    Json(serde_json::Error),

    /// Base64 decoding error.
    Base64(base64::DecodeError),

    /// Hex decoding error.
    Hex(hex::FromHexError),

    /// Turso error.
    #[cfg(all(feature = "turso", not(target_family = "wasm")))]
    Turso(turso::Error),

    /// libsql error.
    #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
    Libsql(libsql::Error),
}

impl core::fmt::Display for StorageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Backend(s) => write!(f, "Backend error: {s}"),
            Self::Connection(s) => write!(f, "Connection failed: {s}"),
            Self::NotFound(s) => write!(f, "Key not found: {s}"),
            Self::Serialization(s) => write!(f, "Serialization error: {s}"),
            Self::Deserialization(s) => write!(f, "Deserialization error: {s}"),
            Self::Encryption(s) => write!(f, "Encryption error: {s}"),
            Self::Migration(s) => write!(f, "Migration error: {s}"),
            Self::Generic(s) => write!(f, "Storage error: {s}"),
            Self::SqlConversion(s) => write!(f, "SQL conversion error: {s}"),
            Self::PreconditionFailed(s) => write!(f, "Precondition failed: {s}"),
            Self::LockTimeout { mode, waited } => write!(
                f,
                "Lock timeout: the {mode} lock was not free after {waited:?}; \
                 an open row stream from this store may be waiting on the same task"
            ),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Base64(e) => write!(f, "Base64 error: {e}"),
            Self::Hex(e) => write!(f, "Hex error: {e}"),
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            Self::Turso(e) => write!(f, "Turso error: {e}"),
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            Self::Libsql(e) => write!(f, "libsql error: {e}"),
        }
    }
}

impl std::error::Error for StorageError {}

/// Result type alias for storage operations.
pub type StorageResult<T> = Result<T, StorageError>;
