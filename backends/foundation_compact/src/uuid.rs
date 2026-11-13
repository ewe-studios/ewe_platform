//! Version-4 Universally Unique Identifier (UUID) generation.
//!
//! This module wraps the `uuid` crate's formatting API only. The 16 random bytes
//! come from [`crate::entropy`] (the native OS random source, or
//! `crypto.getRandomValues` on wasm), never from the `uuid` crate's own
//! `getrandom`/`js`/`rng` backends. Callers that need a UUID use this module so
//! the entropy source stays in one place (rule 11).

use ::uuid::Builder;

pub use ::uuid::Uuid;

/// Generate a random version-4 UUID from the platform entropy source.
///
/// The 16 bytes come from [`crate::entropy::fill`]; [`Builder::from_random_bytes`]
/// sets the version-4 and RFC 4122 variant bits. The result is a plain value, so
/// callers that need the canonical string call `.to_string()`.
///
/// # Errors
///
/// Returns [`crate::entropy::Error`] if the platform entropy source is
/// unavailable.
pub fn new_v4() -> Result<Uuid, crate::entropy::Error> {
    let mut bytes = [0u8; 16];
    crate::entropy::fill(&mut bytes)?;
    Ok(Builder::from_random_bytes(bytes).into_uuid())
}
