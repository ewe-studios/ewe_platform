//! The stable subscription configuration hash.
//!
//! Two create requests with the same normalized configuration hash the same. A re-confirm with a
//! different configuration hashes differently and is refused with a conflict.

use super::model::SubscriptionCreate;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

/// Hash a normalized configuration as lowercase SHA-256 hexadecimal text.
#[must_use]
pub fn stable_config_hash(config: &SubscriptionCreate) -> String {
    // `serde_json` writes struct fields in declaration order and the stream list is already
    // sorted and deduplicated, so the bytes are stable across runs.
    let bytes = serde_json::to_vec(config).unwrap_or_default();
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    out
}
