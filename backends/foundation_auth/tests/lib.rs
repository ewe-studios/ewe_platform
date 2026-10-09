//! Test module tree for foundation_auth integration tests.
//!
//! Structure:
//! - units/         — unit tests per module
//! - integration/   — integration + E2E scenario tests
//! - credential_store_tests.rs — existing (flat)
//! - oauth_tests.rs            — existing (flat)

pub mod units;
pub mod integration;
