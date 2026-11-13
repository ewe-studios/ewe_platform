//! Test module tree for `foundation_auth` integration tests.
//!
//! Structure:
//! - units/         — unit tests per module
//! - integration/   — integration + E2E scenario tests
//! - `credential_store_tests.rs` — existing (flat)
//! - `oauth_tests.rs`            — existing (flat)

// The `Send` check on the deeply nested handler futures needs a deeper limit.
#![recursion_limit = "512"]

pub mod integration;
pub mod units;
