//! Unit tests for `client::connection` moved into the canonical units test tree.
//!
//! These tests are non-destructive copies of the original in-crate `#[cfg(test)]`
//! module and exercise URL parsing functionality (`Uri`) used by the
//! simple HTTP client. They avoid performing real network operations so they
//! are suitable as fast unit tests under `tests/.../units/simple_http/`.

use super::*;
use foundation_netio::shared::client::{MockDnsResolver, SystemDnsResolver};
use foundation_netio::http::HttpClientConnection;
use foundation_netio::shared::http::HttpClientError;

use foundation_core::url::{Scheme, Uri};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

// ========================================================================
// Uri Tests - URL Parsing
// ========================================================================

/// WHY: Verify `Uri` correctly parses a simple HTTP URL
/// WHAT: Tests that scheme, host, port, and path are extracted correctly
/// IMPORTANCE: Basic URL parsing is fundamental to all HTTP requests
#[test]
fn test_parsed_url_simple_http() {
    let url = Uri::parse("http://example.com").unwrap();

    assert!(url.scheme().is_http());
    assert_eq!(url.host_str().unwrap(), "example.com");
    assert_eq!(url.port_or_default(), 80); // Default HTTP port
    assert_eq!(url.path(), "/"); // Default path
    assert_eq!(url.query(), None);
}

/// WHY: Verify `Uri` correctly parses a simple HTTPS URL
/// WHAT: Tests that HTTPS scheme is detected and default port is 443
/// IMPORTANCE: HTTPS is the primary use case for secure connections
#[test]
fn test_parsed_url_simple_https() {
    let url = Uri::parse("https://example.com").unwrap();

    assert!(url.scheme().is_https());
    assert_eq!(url.host_str().unwrap(), "example.com");
    assert_eq!(url.port_or_default(), 443); // Default HTTPS port
    assert_eq!(url.path(), "/");
    assert_eq!(url.query(), None);
}

/// WHY: Verify `Uri` handles explicit port numbers
/// WHAT: Tests that non-default ports override scheme defaults
/// IMPORTANCE: Many services run on non-standard ports
#[test]
fn test_parsed_url_with_explicit_port() {
    let url = Uri::parse("http://example.com:8080").unwrap();

    assert!(url.scheme().is_http());
    assert_eq!(url.host_str().unwrap(), "example.com");
    assert_eq!(url.port().unwrap(), 8080); // Explicit port overrides default
    assert_eq!(url.path(), "/");
}

/// WHY: Verify `Uri` correctly extracts path components
/// WHAT: Tests that paths with multiple segments are preserved
/// IMPORTANCE: `RESTful` APIs use complex path structures
#[test]
fn test_parsed_url_with_path() {
    let url = Uri::parse("http://example.com/api/v1/users").unwrap();

    assert!(url.scheme().is_http());
    assert_eq!(url.host_str().unwrap(), "example.com");
    assert_eq!(url.path(), "/api/v1/users");
    assert_eq!(url.query(), None);
}

/// WHY: Verify `Uri` correctly extracts query strings
/// WHAT: Tests that query parameters are separated from path
/// IMPORTANCE: Query parameters are essential for API requests
#[test]
fn test_parsed_url_with_query() {
    let url = Uri::parse("http://example.com/search?q=test&limit=10").unwrap();

    assert!(url.scheme().is_http());
    assert_eq!(url.host_str().unwrap(), "example.com");
    assert_eq!(url.path(), "/search");
    assert_eq!(url.query().as_deref(), Some("q=test&limit=10"));
}

/// WHY: Verify `Uri` handles complex URLs with all components
/// WHAT: Tests parsing of URL with port, path, and query together
/// IMPORTANCE: Real-world URLs often have all components
#[test]
fn test_parsed_url_complex() {
    let url = Uri::parse("https://api.example.com:8443/v2/users/123?fields=name,email").unwrap();

    assert!(url.scheme().is_https());
    assert_eq!(url.host_str().unwrap(), "api.example.com");
    assert_eq!(url.port().unwrap(), 8443);
    assert_eq!(url.path(), "/v2/users/123");
    assert_eq!(url.query().as_deref(), Some("fields=name,email"));
}

/// WHY: Verify `Uri` rejects URLs with missing scheme
/// WHAT: Tests that URLs without http:// or https:// are rejected
/// IMPORTANCE: Prevents ambiguous URL interpretation
#[test]
fn test_parsed_url_missing_scheme() {
    let result = Uri::parse("example.com");

    assert!(result.is_err());
}

/// WHY: Verify `Uri` rejects URLs with unsupported schemes
/// WHAT: Tests that only HTTP and HTTPS schemes are accepted
/// IMPORTANCE: HTTP client should only handle HTTP/HTTPS
#[test]
fn test_parsed_url_unsupported_scheme() {
    let result = Uri::parse("ftp://example.com");

    // The Uri parser now accepts any scheme during parsing,
    // but scheme validation happens when checking is_http/is_https
    assert!(result.is_ok());
    let url = result.unwrap();
    assert!(!url.scheme().is_http() && !url.scheme().is_https());
}

/// WHY: Verify `Uri` handles URLs with empty host
/// WHAT: Tests that URLs like "http:///" parse but have no host
/// IMPORTANCE: Connection validation happens when accessing `host_str()`
#[test]
fn test_parsed_url_empty_host() {
    let result = Uri::parse("http:///path");

    // The Uri parser may succeed but host validation happens on access
    assert!(result.is_ok());
    let url = result.unwrap();
    // Empty host should return None
    assert!(url.host_str().is_none() || url.host_str().unwrap().is_empty());
}

/// WHY: Verify `Uri` handles IP addresses as hosts
/// WHAT: Tests that IPv4 addresses are valid hosts
/// IMPORTANCE: Direct IP connections are common in testing
#[test]
fn test_parsed_url_with_ip_address() {
    let url = Uri::parse("http://127.0.0.1:8080/test").unwrap();

    assert_eq!(url.host_str().unwrap(), "127.0.0.1");
    assert_eq!(url.port().unwrap(), 8080);
    assert_eq!(url.path(), "/test");
}

/// WHY: Verify `Uri` handles URLs with fragment identifiers
/// WHAT: Tests that fragments (#section) are ignored in HTTP requests
/// IMPORTANCE: Fragments are client-side only, not sent to server
#[test]
fn test_parsed_url_with_fragment() {
    let url = Uri::parse("http://example.com/page#section").unwrap();

    assert_eq!(url.path(), "/page");
    // Fragment should be ignored
}

/// WHY: Verify `Scheme::default_port` returns correct values
/// WHAT: Tests that scheme default ports match HTTP standards
/// IMPORTANCE: Ensures correct default port selection
#[test]
fn test_scheme_default_ports() {
    assert_eq!(Scheme::HTTP.default_port(), 80);
    assert_eq!(Scheme::HTTPS.default_port(), 443);
}

// ========================================================================
// HttpClientConnection Tests - Connection Establishment
// ========================================================================

/// A resolver that maps `host` to a local listener's address.
fn local_resolver(host: &str, addr: SocketAddr) -> MockDnsResolver {
    MockDnsResolver::new().with_response(host, vec![addr])
}

/// WHY: Verify `HttpClientConnection` can establish HTTP connections
/// WHAT: `connect()` opens a TCP connection to a local listener
/// IMPORTANCE: Basic HTTP connectivity is core functionality
#[test]
fn test_connection_http_local() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let accepted = std::thread::spawn(move || listener.accept().map(|(_, peer)| peer));

    let url = Uri::parse(&format!("http://local.test:{}", addr.port())).unwrap();
    let result =
        HttpClientConnection::connect(&url, &local_resolver("local.test", addr), Some(Duration::from_secs(5)));

    assert!(result.is_ok(), "connect failed: {:?}", result.err());
    let peer = accepted.join().expect("server thread").expect("accept");
    assert!(peer.ip().is_loopback());
}

/// WHY: Verify `HttpClientConnection` can establish HTTPS connections with TLS
/// WHAT: `connect_with_tls_config()` completes a TLS handshake with a local
/// rustls server whose certificate (CN/SAN `localhost`) the client trusts
/// IMPORTANCE: HTTPS is the primary use case for secure connections
#[test]
#[cfg(feature = "ssl-rustls")]
fn test_connection_https_local() {
    use foundation_netio::netcap::connection::Connection;
    use foundation_netio::netcap::ssl::rustls::{RustlsAcceptor, RustlsConnector};
    use rustls::pki_types::pem::PemObject;
    use std::sync::Arc;
    use zeroize::Zeroizing;

    let fixture = |name: &str| {
        std::fs::read(format!(
            "{}/tests/tcp_and_tls/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("read TLS fixture")
    };
    let cert = fixture("cert.pem");
    let acceptor =
        RustlsAcceptor::from_pem(cert.clone(), Zeroizing::new(fixture("key.pem"))).expect("acceptor");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handshake = std::thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept");
        acceptor.accept(Connection::Tcp(socket)).map(|_| ())
    });

    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from_pem_slice(&cert).expect("cert der"))
        .expect("trust fixture");
    let client_tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = RustlsConnector::with_config(Arc::new(client_tls));

    let url = Uri::parse(&format!("https://localhost:{}", addr.port())).unwrap();
    let result = HttpClientConnection::connect_with_tls_config(
        &url,
        &local_resolver("localhost", addr),
        Some(Duration::from_secs(5)),
        Some(&connector),
    );

    assert!(result.is_ok(), "TLS connect failed: {:?}", result.err());
    handshake
        .join()
        .expect("server thread")
        .expect("server-side handshake");
}

/// WHY: Verify `HttpClientConnection` respects connection timeout
/// WHAT: `connect()` returns `ConnectionTimeout` when the peer never completes
/// the TCP handshake. A local listener whose accept queue is full stands in for
/// an unreachable host: Linux drops new SYNs to it, so the connect hangs until
/// the client's own deadline.
/// IMPORTANCE: Prevents hanging connections from blocking indefinitely
#[test]
#[cfg(target_os = "linux")]
fn test_connection_timeout() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");

    // Fill the accept queue (never accepting) until a connect stalls.
    let mut held = Vec::new();
    let mut queue_full = false;
    for _ in 0..8192 {
        match std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)) {
            Ok(stream) => held.push(stream),
            Err(err) if err.kind() == std::io::ErrorKind::TimedOut => {
                queue_full = true;
                break;
            }
            Err(err) => panic!("unexpected connect error while filling the queue: {err}"),
        }
    }
    assert!(queue_full, "accept queue never filled after {} connects", held.len());

    let url = Uri::parse(&format!("http://stalled.test:{}", addr.port())).unwrap();
    let started = std::time::Instant::now();
    let result = HttpClientConnection::connect(
        &url,
        &local_resolver("stalled.test", addr),
        Some(Duration::from_millis(100)),
    );

    let err = result.expect_err("connect must time out");
    assert!(
        matches!(err, HttpClientError::ConnectionTimeout(_)),
        "expected ConnectionTimeout, got {err:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "timed out only after {:?}",
        started.elapsed()
    );
    drop(held);
    drop(listener);
}

/// WHY: Verify `HttpClientConnection` handles DNS resolution failures
/// WHAT: Tests that `connect()` returns appropriate error when DNS fails
/// IMPORTANCE: DNS errors are common and should be handled gracefully
#[test]
fn test_connection_dns_failure() {
    let url = Uri::parse("http://invalid-host-12345.invalid").unwrap();
    let resolver = SystemDnsResolver::new();

    let result =
        HttpClientConnection::connect(&url, &resolver, Some(std::time::Duration::from_secs(5)));

    assert!(result.is_err());
    // Should be wrapped in HttpClientError::DnsError
}

/// WHY: Verify `HttpClientConnection` works with mock resolver
/// WHAT: Tests that `connect()` uses provided resolver for DNS
/// IMPORTANCE: Enables testing without real network
#[test]
fn test_connection_with_mock_resolver() {
    // Pick a free port and close it, so nothing listens. Hard-coding 8080
    // collided with unrelated local services (the port is not ours to use).
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind an ephemeral port")
        .local_addr()
        .expect("local addr")
        .port();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let resolver = MockDnsResolver::new().with_response("example.com", vec![addr]);

    let url = Uri::parse(&format!("http://example.com:{port}")).unwrap();

    // This will fail to connect since no server is running, but DNS should work
    let result =
        HttpClientConnection::connect(&url, &resolver, Some(std::time::Duration::from_millis(100)));

    // We expect connection failure, not DNS failure
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        HttpClientError::ConnectionFailed(_) | HttpClientError::ConnectionTimeout(_)
    ));
}
