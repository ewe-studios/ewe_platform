//! SHA-256 fingerprints in lowercase hexadecimal.
//!
//! WHY: a fingerprint lets a log line or a database row name a key, a secret or
//! a certificate without holding the value. The same text form is used on every
//! target, so a native host and a Worker compare the same string.
//!
//! WHAT: [`sha256_hex`], the one helper for the crate.
//!
//! HOW: `sha2` builds the digest. It is pure Rust, so it builds for wasm.

use sha2::{Digest, Sha256};

/// Lowercase hexadecimal SHA-256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}
