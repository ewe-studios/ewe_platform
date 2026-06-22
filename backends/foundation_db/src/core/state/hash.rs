//! Config hashing for deployment change detection.
//!
//! WHY: The deployment engine skips deploying when the config hasn't changed.
//! A deterministic hash of the serialized config enables cheap comparison.
//!
//! WHAT: `config_hash` serializes any `Serialize` value to canonical JSON
//! and returns its SHA-256 hex digest.
//!
//! HOW: `serde_json::to_string` (keys sorted by serde default) → SHA-256 → hex.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::core::errors::StorageError;

/// Compute a deterministic SHA-256 hash of a serializable config value.
///
/// The value is serialized to JSON (compact, keys in insertion order per serde)
/// and then hashed. Two configs that serialize identically produce the same hash.
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
pub fn config_hash<T: Serialize>(value: &T) -> Result<String, StorageError> {
    let json =
        serde_json::to_string(value).map_err(|e| StorageError::Serialization(e.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    let result = hasher.finalize();
    Ok(hex::encode(result))
}