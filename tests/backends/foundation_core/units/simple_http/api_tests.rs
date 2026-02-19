//! Unit tests for `client::api` moved into the canonical units test tree.
//!
//! These tests are non-destructive copies of the original in-crate `#[cfg(test)]`
//! module. They exercise lightweight behaviors of the `ClientRequest`/`ClientConfig`
//! construction without performing real network operations.

use foundation_core::wire::simple_http::client::{
    ClientConfig, ClientRequest, ClientRequestBuilder, ClientRequestState, MockDnsResolver,
    StaticSocketAddr,
};
use std::net::SocketAddr as StdSocketAddr;

/// WHY: Verify ClientRequest::new creates request in NotStarted state
/// WHAT: Tests that constructor initializes correctly
#[test]
fn test_client_request_new() {
    let prepared = ClientRequestBuilder::get(
        StaticSocketAddr::new(StdSocketAddr::from(([127, 0, 0, 1], 80))),
        "http://example.com",
    )
    .unwrap()
    .build()
    .unwrap();
    let resolver = MockDnsResolver::new();
    let config = ClientConfig::default();

    let request = ClientRequest::new(prepared, resolver, config, None);

    assert!(matches!(
        request.task_state,
        Some(ClientRequestState::NotStarted)
    ));
}
