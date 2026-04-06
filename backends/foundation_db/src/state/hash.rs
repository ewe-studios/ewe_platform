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

use crate::errors::StorageError;

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deterministic_hash() {
        let config = json!({"name": "my-worker", "account_id": "abc123"});
        let h1 = config_hash(&config).unwrap();
        let h2 = config_hash(&config).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn different_configs_different_hashes() {
        let a = json!({"name": "worker-a"});
        let b = json!({"name": "worker-b"});
        assert_ne!(config_hash(&a).unwrap(), config_hash(&b).unwrap());
    }

    #[test]
    fn empty_object_hashes() {
        let empty = json!({});
        let h = config_hash(&empty).unwrap();
        assert_eq!(h.len(), 64);
    }
}
