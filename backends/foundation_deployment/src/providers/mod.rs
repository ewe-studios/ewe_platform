//! Provider-specific spec fetcher implementations.
//!
//! Each provider module implements custom extraction logic for their
//! specific OpenAPI spec format.

pub mod gcp;
pub mod stripe;
