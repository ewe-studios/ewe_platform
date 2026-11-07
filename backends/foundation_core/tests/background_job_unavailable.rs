//! A missing background pool is an error, not a panic (#348).
//!
//! WHY a separate test binary: the background pool is process-global, and
//! other tests start one; here nothing does.

#![cfg(all(feature = "multi", not(target_family = "wasm")))]

use foundation_core::valtron::run_background_job;

#[test]
fn a_missing_background_pool_is_an_error() {
    let error = run_background_job(|| {}).expect_err("a missing pool must be an error");
    let message = error.to_string();
    assert!(
        message.contains("Background job pool not initialized"),
        "the error must name the missing pool, got: {message}"
    );
}
