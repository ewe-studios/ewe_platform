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
use tracing_test::traced_test;

#[test]
fn test_client_config_default() {
    let config = ClientConfig::default();

    assert_eq!(config.max_redirects, 5);
    assert!(config.default_headers.is_empty());
}

#[test]
fn test_client_config_fields_public() {
    let mut config = ClientConfig::default()
        .with_connect_timeout(Duration::from_secs(10))
        .with_max_retries(3);

    assert_eq!(config.connect_timeout, Duration::from_secs(10));
    assert_eq!(config.max_retries, 3);
}

/// WHAT: Debug test for InvalidLocation error mapping using the public API.
/// WHY: Prints actual error string for invalid URL to adjust assertion accordingly.
#[test]
fn debug_invalid_location_error() {
    use foundation_core::valtron::single::initialize_pool;
    initialize_pool(45);

    // Setup: Create a client with default config
    let client = SimpleHttpClient::from_system();

    // Build a GET request to an invalid URL
    let request_result = client.get("http://");
    assert!(request_result.is_err());

    // Print actual error string for visibility
    if let Err(err) = request_result {
        let err_str = format!("{:?}", err);
        println!("Actual error string for invalid location: {}", err_str);
    } else {
        panic!("Expected error due to invalid location, but got Ok");
    }
}

#[test]
fn test_client_config_clone() {
    let config = ClientConfig::default();
    let cloned = config.clone();

    assert_eq!(cloned.max_redirects, config.max_redirects);
}
