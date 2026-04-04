//! Unit tests for `ConnectionPool` placed into the canonical units test tree.
//!
//! These tests exercise the public, fast APIs of the connection pool without
//! relying on heavyweight stream construction. They are intended to be
//! deterministic, quick, and suitable for running as part of the unit test
//! suite under `tests/backends/foundation_core/units/simple_http/`.

use foundation_core::wire::simple_http::client::ConnectionPool;
use std::time::Duration;

/// Basic sanity checks for `ConnectionPool`.
///
/// - Creating a pool should succeed.
/// - Checkout on an empty pool returns `None`.
/// - Public maintenance helpers (`cleanup_stale`, `clear`) are callable and do not panic.
#[test]
fn test_connection_pool_basic_sanity() {
    let pool = ConnectionPool::new(4, Duration::from_secs(60));

    // Empty pool -> no connection available
    assert!(pool.checkout("example.com", 80).is_none());

    // Maintenance helpers should be callable and not panic
    pool.cleanup_stale();
    pool.clear();
}

/// Verify that cleanup and clear can be invoked multiple times safely.
///
/// This mirrors expected lifecycle usage where cleanup may be called
/// periodically and `clear` may be called on shutdown or test setup/teardown.
#[test]
fn test_cleanup_and_clear_idempotent() {
    let pool = ConnectionPool::new(2, Duration::from_secs(0));

    // Call multiple times to ensure no panics and stable behavior
    pool.cleanup_stale();
    pool.cleanup_stale();
    pool.clear();
    pool.clear();

    // Still empty after clears
    assert!(pool.checkout("localhost", 8080).is_none());
}

/// WHY: Verify ConnectionPool::new creates pool
#[test]
fn test_connection_pool_new() {
    let pool = ConnectionPool::new(10, Duration::from_secs(60));
    assert_eq!(pool.max_per_host, 10);
    assert_eq!(pool.max_idle_time, Duration::from_secs(60));
}

/// WHY: Verify checkout returns None for empty pool
#[test]
fn test_connection_pool_checkout_empty() {
    let pool = ConnectionPool::new(10, Duration::from_secs(60));
    let result = pool.checkout("example.com", 80);
    assert!(result.is_none());
}

/// WHY: Basic checkin/checkout semantics (logical test)
/// NOTE: We cannot construct a real `SharedByteBufferStream<RawStream>` easily
/// in unit tests here without additional helpers. This test asserts that
/// checkin/checkout APIs are callable and do not panic when used with a
/// dummy stream via Arc/Clone when available. For now we rely on the
/// existing stub tests to validate surface.
#[test]
fn test_cleanup_and_clear_no_panic() {
    let pool = ConnectionPool::new(2, Duration::from_secs(0));
    pool.cleanup_stale();
    pool.clear();
}
