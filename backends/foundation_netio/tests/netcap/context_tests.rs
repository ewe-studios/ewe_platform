//! Tests for `netcap::ConnectionContext` and its presence on requests.
//!
//! WHY: `ConnectionContext` is the connection-scoped metadata (peer address,
//! TLS, ALPN, …) that `SimpleIncomingRequest` carries once per connection
//! (Decision 12 §13). These tests pin the empty-default contract (additive, no
//! behavior change for directly-built requests) and that a request can carry
//! and expose a populated context.

use std::sync::Arc;

use foundation_netio::netcap::{ConnectionContext, PeerIdentity, TlsInfo};
use foundation_netio::simple_http::shared::SimpleIncomingRequest;

/// WHY: The default must be a valid "nothing known" value so callers that build
/// requests directly (tests, wasm client rendering) pay nothing and change no
/// behavior.
/// WHAT: `ConnectionContext::default`/`empty` is anonymous with no TLS/ALPN.
#[test]
fn test_connection_context_empty_default() {
    let ctx = ConnectionContext::default();
    assert_eq!(ctx.peer_identity, PeerIdentity::Anonymous);
    assert!(ctx.tls.is_none());
    assert!(ctx.alpn.is_none());
    assert!(!ctx.early_data);
    assert!(ctx.quic_connection_id.is_none());
    assert_eq!(ctx.alpn_str(), None);
    assert!(ctx.peer_addr.is_none());
    // `empty()` is the same as `default()`.
    assert_eq!(ConnectionContext::empty().peer_identity, PeerIdentity::Anonymous);
}

/// WHY: A request built without a connection context must still be valid and
/// carry the empty default — the additive contract.
/// WHAT: `SimpleIncomingRequest::builder().build()` yields an empty connection.
#[test]
fn test_request_defaults_to_empty_connection() {
    let req = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/")
        .build()
        .expect("valid request");

    assert_eq!(req.connection.peer_identity, PeerIdentity::Anonymous);
    assert!(req.connection.peer_addr.is_none());
}

/// WHY: The front end attaches a populated context once per connection; handlers
/// must be able to reach connection facts through the request.
/// WHAT: `with_connection` carries the context, and cloning it is a refcount bump
/// (both handles observe the same shared context).
#[test]
fn test_request_carries_populated_connection() {
    let addr = "127.0.0.1:8080".parse().expect("valid socket addr");
    let ctx = Arc::new(ConnectionContext {
        peer_addr: Some(foundation_netio::netcap::SocketAddr::Tcp(addr)),
        peer_identity: PeerIdentity::Ed25519(vec![0xAB; 32]),
        tls: Some(TlsInfo {
            peer_certificates: vec![vec![1, 2, 3]],
            server_name: Some("example.com".to_string()),
        }),
        alpn: Some(b"h2".to_vec()),
        ..ConnectionContext::default()
    });

    let req = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/")
        .with_connection(ctx.clone())
        .build()
        .expect("valid request");

    assert!(req.connection.peer_addr.is_some());
    assert_eq!(req.connection.peer_identity, PeerIdentity::Ed25519(vec![0xAB; 32]));
    assert_eq!(req.connection.alpn_str(), Some("h2"));
    assert_eq!(
        req.connection.tls.as_ref().and_then(|t| t.server_name.as_deref()),
        Some("example.com")
    );
    // Shared by refcount: the builder cloned the Arc, so both point at one value.
    assert_eq!(Arc::strong_count(&ctx), 2);
}
