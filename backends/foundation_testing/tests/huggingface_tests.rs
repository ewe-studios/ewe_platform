//! Tests for the `foundation_testing::huggingface` test harness.
//!
//! The `huggingface` module is gated behind the `huggingface` feature, so these
//! integration tests carry the same gate.

#![cfg(feature = "huggingface")]

use foundation_testing::huggingface::{TestHarness, DEFAULT_ARTEFACTS_DIR, MODELS_SUBDIR};

#[test]
fn test_artefacts_dir_constant() {
    assert_ne!(DEFAULT_ARTEFACTS_DIR, "");
    assert_eq!(MODELS_SUBDIR, "models");
}

#[test]
fn test_harness_creation() {
    let temp_dir = std::env::temp_dir();
    let harness = TestHarness::new(&temp_dir);
    // artefacts_dir should be: temp_dir/artefacts/models
    assert!(harness.artefacts_dir().ends_with(MODELS_SUBDIR));
    assert!(harness
        .artefacts_dir()
        .parent()
        .unwrap()
        .ends_with(DEFAULT_ARTEFACTS_DIR));
}
