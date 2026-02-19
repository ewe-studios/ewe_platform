//! Unit tests for `client::client` moved into the canonical units test tree.
//!
//! These tests exercise `ClientConfig` and `SimpleHttpClient` builder/constructor
//! behaviors in a fast, deterministic way. They are non-destructive copies of
//! the original in-crate tests and import the public API via `foundation_core`.
//!
//! They intentionally avoid performing real network operations.

use foundation_core::wire::simple_http::client::*;
use foundation_core::wire::simple_http::*;
use std::time::Duration;

#[test]
fn test_client_config_default() {
    let config = ClientConfig::default();

    assert!(config.connect_timeout.is_some());
    assert!(config.read_timeout.is_some());
    assert!(config.write_timeout.is_some());
    assert_eq!(config.max_redirects, 5);
    assert!(config.default_headers.is_empty());
    assert!(!config.pool_enabled);
    assert_eq!(config.pool_max_connections, 10);
}

#[test]
fn test_client_config_fields_public() {
    let mut config = ClientConfig::default();
    config.connect_timeout = Some(Duration::from_secs(10));
    config.max_redirects = 3;
    config.pool_enabled = true;

    assert_eq!(config.connect_timeout, Some(Duration::from_secs(10)));
    assert_eq!(config.max_redirects, 3);
    assert!(config.pool_enabled);
}

#[test]
fn test_client_config_clone() {
    let config = ClientConfig::default();
    let cloned = config.clone();

    assert_eq!(cloned.max_redirects, config.max_redirects);
    assert_eq!(cloned.pool_enabled, config.pool_enabled);
}
