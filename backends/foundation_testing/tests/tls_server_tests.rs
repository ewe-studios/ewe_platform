//! `TestHttpsServer` over a real TLS connection (moved from the inline test
//! module of `src/http/tls_server.rs`).

use std::net::SocketAddr;

use foundation_netio::{HttpClientBuilder, PreparedRequestBuilder};
use foundation_testing::http::{test_tls_connector, HttpResponse, TestHttpsServer};

#[test]
fn test_https_server_starts() {
    let server = TestHttpsServer::start();
    assert!(server.base_url().starts_with("https://127.0.0.1:"));
}

#[test]
fn test_https_roundtrip() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    let server = TestHttpsServer::with_response(|_req| HttpResponse::ok(b"hello from TLS"));

    // `localhost` is the name on the test certificate; it resolves to the
    // server's socket.
    let client = HttpClientBuilder::new()
        .resolve_to(SocketAddr::from(([127, 0, 0, 1], server.port())))
        .with_tls_connector(test_tls_connector())
        .build();

    let request = PreparedRequestBuilder::get("https://localhost/test")
        .unwrap()
        .build();
    let response = client.send(request).expect("HTTPS request should succeed");

    assert_eq!(response.get_status().into_usize(), 200);
}
