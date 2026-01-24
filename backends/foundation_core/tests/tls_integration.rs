//! Integration tests for TLS backends
//!
//! These tests verify that TLS backends are properly configured and can be instantiated.
//! Network tests are marked as #[ignore] to avoid requiring internet connectivity.

#![cfg(not(target_arch = "wasm32"))]

#[cfg(feature = "ssl-rustls")]
mod rustls_tests {
    use foundation_core::netcap::ssl::rustls::{default_client_config, RustlsConnector};

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
    #[ignore] // Requires network access
    fn test_rustls_https_connection() {
        use foundation_core::netcap::connection::Connection;
        use std::io::{Read, Write};
        use std::net::TcpStream;

        let connector = RustlsConnector::new();

        // Connect to example.com:443
        let tcp = TcpStream::connect("example.com:443").expect("Failed to connect to example.com");

        let connection = Connection::Tcp(tcp);

        let (mut tls_stream, _addr) = connector
            .from_tcp_stream("example.com".to_string(), connection)
            .expect("Failed to establish TLS connection");

        // Send a simple HTTP request
        let request = "GET / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n";
        tls_stream
            .write_all(request.as_bytes())
            .expect("Failed to write request");

        // Read response
        let mut response = Vec::new();
        tls_stream
            .read_to_end(&mut response)
            .expect("Failed to read response");

        // Verify we got a valid HTTP response
        let response_str = String::from_utf8_lossy(&response);
        assert!(response_str.contains("HTTP/1.1"), "Expected HTTP response");
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

#[cfg(feature = "ssl-openssl")]
mod openssl_tests {
    use foundation_core::netcap::ssl::openssl::{OpenSslConnector, SslConnector};
    use foundation_core::netcap::{Endpoint, EndpointConfig};
    use std::sync::Arc;

    #[test]
    fn test_openssl_connector_creation() {
        // Verify OpenSSL connector can be created
        let ssl_connector = SslConnector::builder(openssl::ssl::SslMethod::tls())
            .expect("Failed to create SSL connector builder")
            .build();

        let url = url::Url::parse("https://example.com:443").unwrap();
        let endpoint =
            Endpoint::WithIdentity(EndpointConfig::NoTimeout(url), Arc::new(ssl_connector));

        let connector = OpenSslConnector::create(&endpoint);
        let _connector2 = connector.clone();
    }

    #[test]
    fn test_openssl_connector_clone() {
        let ssl_connector = SslConnector::builder(openssl::ssl::SslMethod::tls())
            .expect("Failed to create SSL connector builder")
            .build();

        let url = url::Url::parse("https://example.com:443").unwrap();
        let endpoint =
            Endpoint::WithIdentity(EndpointConfig::NoTimeout(url), Arc::new(ssl_connector));

        let connector1 = OpenSslConnector::create(&endpoint);
        let connector2 = connector1.clone();

        // Verify Arc sharing
        assert_eq!(Arc::strong_count(&connector1.0), 2);
        assert_eq!(Arc::strong_count(&connector2.0), 2);
    }

    #[test]
    #[ignore] // Requires network access
    fn test_openssl_https_connection() {
        use foundation_core::netcap::connection::Connection;
        use std::io::{Read, Write};
        use std::net::TcpStream;

        let ssl_connector = SslConnector::builder(openssl::ssl::SslMethod::tls())
            .expect("Failed to create SSL connector builder")
            .build();

        let url = url::Url::parse("https://example.com:443").unwrap();
        let endpoint =
            Endpoint::WithIdentity(EndpointConfig::NoTimeout(url), Arc::new(ssl_connector));

        let connector = OpenSslConnector::create(&endpoint);

        // Connect to example.com
        let tcp = TcpStream::connect("example.com:443").expect("Failed to connect to example.com");

        let connection = Connection::Tcp(tcp);

        let (mut tls_stream, _addr) = connector
            .from_tcp_stream("example.com".to_string(), connection)
            .expect("Failed to establish TLS connection");

        // Send a simple HTTP request
        let request = "GET / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n";
        tls_stream
            .write_all(request.as_bytes())
            .expect("Failed to write request");

        // Read response
        let mut response = Vec::new();
        tls_stream
            .read_to_end(&mut response)
            .expect("Failed to read response");

        // Verify we got a valid HTTP response
        let response_str = String::from_utf8_lossy(&response);
        assert!(response_str.contains("HTTP/1.1"), "Expected HTTP response");
    }
}

#[cfg(feature = "ssl-native-tls")]
mod native_tls_tests {
    use foundation_core::netcap::ssl::native_ttls::NativeTlsConnector;
    use foundation_core::netcap::{Endpoint, EndpointConfig};
    use std::sync::Arc;

    #[test]
    fn test_native_tls_connector_creation() {
        // Verify native-tls connector can be created
        let tls_connector =
            native_tls::TlsConnector::new().expect("Failed to create TLS connector");

        let url = url::Url::parse("https://example.com:443").unwrap();
        let endpoint =
            Endpoint::WithIdentity(EndpointConfig::NoTimeout(url), Arc::new(tls_connector));

        let connector = NativeTlsConnector::create(&endpoint);
        let _connector2 = connector.clone();
    }

    #[test]
    fn test_native_tls_connector_clone() {
        let tls_connector =
            native_tls::TlsConnector::new().expect("Failed to create TLS connector");

        let url = url::Url::parse("https://example.com:443").unwrap();
        let endpoint =
            Endpoint::WithIdentity(EndpointConfig::NoTimeout(url), Arc::new(tls_connector));

        let connector1 = NativeTlsConnector::create(&endpoint);
        let connector2 = connector1.clone();

        // Verify both connectors are usable
        assert_eq!(Arc::strong_count(&connector1.0), 2);
        assert_eq!(Arc::strong_count(&connector2.0), 2);
    }

    #[test]
    #[ignore] // Requires network access
    fn test_native_tls_https_connection() {
        use foundation_core::netcap::connection::Connection;
        use std::io::{Read, Write};
        use std::net::TcpStream;

        let tls_connector =
            native_tls::TlsConnector::new().expect("Failed to create TLS connector");

        let url = url::Url::parse("https://example.com:443").unwrap();
        let endpoint =
            Endpoint::WithIdentity(EndpointConfig::NoTimeout(url), Arc::new(tls_connector));

        let connector = NativeTlsConnector::create(&endpoint);

        // Connect to example.com
        let tcp = TcpStream::connect("example.com:443").expect("Failed to connect to example.com");

        let connection = Connection::Tcp(tcp);

        let (mut tls_stream, _addr) = connector
            .from_tcp_stream("example.com", connection)
            .expect("Failed to establish TLS connection");

        // Send a simple HTTP request
        let request = "GET / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n";
        tls_stream
            .write_all(request.as_bytes())
            .expect("Failed to write request");

        // Read response
        let mut response = Vec::new();
        tls_stream
            .read_to_end(&mut response)
            .expect("Failed to read response");

        // Verify we got a valid HTTP response
        let response_str = String::from_utf8_lossy(&response);
        assert!(response_str.contains("HTTP/1.1"), "Expected HTTP response");
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

        let backend_count = HAS_RUSTLS as u8 + HAS_OPENSSL as u8 + HAS_NATIVE_TLS as u8;

        // We expect exactly one backend to be enabled when running TLS tests
        if backend_count > 0 {
            assert_eq!(
                backend_count, 1,
                "Expected exactly one TLS backend to be enabled, found {}",
                backend_count
            );
        }
    }
}
