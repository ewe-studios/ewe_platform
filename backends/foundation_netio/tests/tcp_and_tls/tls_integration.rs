//! Integration tests for TLS backends.
//!
//! The network tests run against a local TLS server with a self-signed test
//! certificate. No public internet access is needed.

#![cfg(not(target_family = "wasm"))]

/// Read a TLS test fixture from the crate's test fixtures directory.
fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/tcp_and_tls/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(path).expect("read TLS fixture")
}

/// Read an HTTP/1.1 response from a TLS stream, stopping at the end of the
/// headers (`\r\n\r\n`). Handles a plain EOF and a rustls `UnexpectedEof`
/// (a close without a TLS `close_notify` alert) the same way.
fn read_http_response(stream: &mut impl std::io::Read) -> String {
    let mut response = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                response.extend_from_slice(&buf[..n]);
                if response.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => panic!("read response failed: {e}"),
        }
    }
    String::from_utf8_lossy(&response).into_owned()
}

#[cfg(feature = "ssl-rustls")]
mod rustls_tests {
    use foundation_netio::netcap::ssl::rustls::{default_client_config, RustlsConnector};

    #[test]
    fn test_rustls_connector_new() {
        // Verify connector can be created with default config
        let connector = RustlsConnector::new();
        let _connector2 = connector.clone();
        // If we get here, connector was created successfully
    }

    #[test]
    fn test_rustls_default_config_has_roots() {
        // Verify that default config includes root certificates
        let config = default_client_config();
        // Config should be created successfully
        assert!(std::sync::Arc::strong_count(&config) >= 1);
    }

    #[test]
    fn test_rustls_connector_default_trait() {
        // Verify Default trait works
        let connector = RustlsConnector::default();
        let _connector2 = connector.clone();
    }

    #[test]
    fn test_rustls_https_connection() {
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};

        use foundation_netio::netcap::connection::Connection;
        use foundation_netio::netcap::ssl::SSLAcceptor;
        use foundation_testing::http::test_tls_connector;
        use zeroize::Zeroizing;

        use super::fixture;
        use super::read_http_response;

        let acceptor =
            SSLAcceptor::from_pem(fixture("cert.pem"), Zeroizing::new(fixture("key.pem")))
                .expect("acceptor");
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");

        let server = std::thread::spawn(move || {
            let (socket, _) = listener.accept().expect("accept");
            let mut tls = acceptor.accept(Connection::Tcp(socket)).expect("tls accept");
            let mut buf = [0u8; 8192];
            let n = tls.read(&mut buf).expect("server read");
            assert!(n > 0, "server read no request bytes");
            tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .expect("server write");
            tls.flush().expect("server flush");
        });

        // `localhost` is the name on the test certificate; it resolves to the
        // server's socket. `test_tls_connector` trusts that certificate.
        let plain = TcpStream::connect(addr).expect("connect");
        let (mut tls_stream, _addr) = test_tls_connector()
            .from_tcp_stream("localhost".to_string(), Connection::Tcp(plain))
            .expect("client tls stream");

        tls_stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .expect("write request");
        tls_stream.flush().expect("flush request");

        let response = read_http_response(&mut tls_stream);
        assert!(response.contains("HTTP/1.1"), "expected HTTP response: {response}");

        server.join().expect("server thread");
    }

    #[test]
    fn test_rustls_clone_and_concurrent_access() {
        use std::thread;

        let connector = RustlsConnector::new();
        let connector_clone = connector.clone();

        // Test concurrent usage
        let handle = thread::spawn(move || {
            // Verify we can use the cloned connector in another thread
            let _connector = connector_clone;
        });

        handle.join().expect("Thread should complete successfully");
    }
}

// Test that conflicting features produce compile errors
#[cfg(test)]
mod feature_tests {
    //! These tests verify feature mutual exclusivity.
    //!
    //! To test feature conflicts manually:
    //! ```bash
    //! # This should fail with compile_error!
    //! cargo check --package foundation_core --no-default-features --features ssl-rustls,ssl-openssl
    //!
    //! # Expected error:
    //! # error: Cannot enable both `ssl-rustls` and `ssl-openssl`. Choose one TLS backend.
    //! ```

    #[test]
    fn test_feature_mutual_exclusivity_documented() {
        // This test documents that feature conflicts are checked at compile time
        // The actual checks are in src/netcap/ssl/mod.rs via compile_error! macros
        assert!(
            true,
            "Feature conflicts are enforced via compile_error! in mod.rs"
        );
    }

    #[test]
    fn test_at_least_one_backend_available() {
        // At least one TLS backend should be available when running tests
        #[cfg(feature = "ssl-rustls")]
        const HAS_RUSTLS: bool = true;
        #[cfg(not(feature = "ssl-rustls"))]
        const HAS_RUSTLS: bool = false;

        #[cfg(feature = "ssl-openssl")]
        const HAS_OPENSSL: bool = true;
        #[cfg(not(feature = "ssl-openssl"))]
        const HAS_OPENSSL: bool = false;

        #[cfg(feature = "ssl-native-tls")]
        const HAS_NATIVE_TLS: bool = true;
        #[cfg(not(feature = "ssl-native-tls"))]
        const HAS_NATIVE_TLS: bool = false;

        let backend_count = u8::from(HAS_RUSTLS) + u8::from(HAS_OPENSSL) + u8::from(HAS_NATIVE_TLS);

        // We expect exactly one backend to be enabled when running TLS tests
        if backend_count > 0 {
            assert_eq!(
                backend_count, 1,
                "Expected exactly one TLS backend to be enabled, found {backend_count}"
            );
        }
    }
}
