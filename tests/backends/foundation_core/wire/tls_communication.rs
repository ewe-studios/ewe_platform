//! TLS Communication Tests with Real Certificates
//!
//! These tests use actual generated certificates to test full TLS handshake
//! and data transmission between client and server.

#![cfg(not(target_arch = "wasm32"))]
#![cfg(feature = "ssl-rustls")]

use foundation_core::netcap::connection::Connection;
use foundation_core::netcap::ssl::rustls::{RustlsAcceptor, RustlsConnector};
use rustls::pki_types::pem::PemObject;
use rustls::RootCertStore;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use zeroize::Zeroizing;

// Load test certificate and key from fixtures
fn load_test_cert() -> (Vec<u8>, Zeroizing<Vec<u8>>) {
    let cert_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cert.pem");
    let key_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/key.pem");

    let cert = std::fs::read(cert_path).expect("Failed to read test certificate");
    let key = Zeroizing::new(std::fs::read(key_path).expect("Failed to read test key"));

    (cert, key)
}

#[test]
fn test_tls_acceptor_with_real_cert() {
    let (cert, key) = load_test_cert();

    // Create acceptor with real certificate
    let acceptor = RustlsAcceptor::from_pem(cert, key);

    assert!(
        acceptor.is_ok(),
        "Failed to create acceptor with valid certificate: {:?}",
        acceptor.err()
    );
}

#[test]
fn test_tls_full_handshake_and_data_transfer() {
    let (cert, key) = load_test_cert();

    // Bind to localhost on a random port
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to localhost");
    let server_addr = listener.local_addr().unwrap();

    // Create acceptor with test certificate
    let acceptor = RustlsAcceptor::from_pem(cert.clone(), key).expect("Failed to create acceptor");

    // Spawn server thread
    let server_handle = thread::spawn(move || -> Result<(), String> {
        // Accept one connection
        if let Ok((stream, _)) = listener.accept() {
            let connection = Connection::Tcp(stream);

            // Accept TLS connection
            match acceptor.accept(connection) {
                Ok(mut tls_stream) => {
                    // Read request from client
                    let mut buf = [0u8; 1024];
                    if let Ok(n) = tls_stream.read(&mut buf) {
                        let request = String::from_utf8_lossy(&buf[..n]);

                        // Verify we received the expected message
                        if request.contains("Hello from client") {
                            // Send response back
                            let response = b"Hello from server";
                            let _ = tls_stream.write_all(response);
                            let _ = tls_stream.flush();
                            return Ok(());
                        }
                    }
                    Err("Failed to read from client".to_string())
                }
                Err(e) => Err(format!("Failed to accept TLS connection: {}", e)),
            }
        } else {
            Err("Failed to accept TCP connection".to_string())
        }
    });

    // Give server time to start
    thread::sleep(Duration::from_millis(100));

    // Create client connector that trusts our test certificate
    let cert_der = rustls::pki_types::CertificateDer::from_pem_slice(&cert)
        .expect("Failed to parse certificate");

    let mut root_store = RootCertStore::empty();
    root_store
        .add(cert_der)
        .expect("Failed to add test cert to root store");

    let client_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = RustlsConnector::with_config(Arc::new(client_config));

    // Connect to server
    let tcp = TcpStream::connect(server_addr).expect("Failed to connect to server");

    let connection = Connection::Tcp(tcp);

    // Establish TLS connection
    let result = connector.from_tcp_stream("localhost".to_string(), connection);

    assert!(result.is_ok(), "TLS handshake failed: {:?}", result.err());

    let (mut tls_stream, _addr) = result.unwrap();

    // Send message to server
    tls_stream
        .write_all(b"Hello from client")
        .expect("Failed to write to server");
    tls_stream.flush().expect("Failed to flush");

    // Read response from server
    let mut response = [0u8; 1024];
    let n = tls_stream
        .read(&mut response)
        .expect("Failed to read from server");

    let response_str = String::from_utf8_lossy(&response[..n]);
    assert_eq!(
        response_str, "Hello from server",
        "Unexpected response from server"
    );

    // Wait for server thread to complete
    let server_result = server_handle.join().expect("Server thread panicked");
    assert!(
        server_result.is_ok(),
        "Server failed: {:?}",
        server_result.err()
    );
}

#[test]
fn test_multiple_tls_connections() {
    let (cert, key) = load_test_cert();

    // Bind to localhost on a random port
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to localhost");
    let server_addr = listener.local_addr().unwrap();

    // Create acceptor with test certificate
    let acceptor =
        Arc::new(RustlsAcceptor::from_pem(cert.clone(), key).expect("Failed to create acceptor"));

    // Spawn server thread that handles multiple connections
    let server_handle = thread::spawn(move || {
        let mut success_count = 0;

        for _ in 0..3 {
            if let Ok((stream, _)) = listener.accept() {
                let connection = Connection::Tcp(stream);
                let acceptor_clone = acceptor.clone();

                // Handle each connection in a separate thread
                let handle = thread::spawn(move || {
                    if let Ok(mut tls_stream) = acceptor_clone.accept(connection) {
                        let mut buf = [0u8; 1024];
                        if tls_stream.read(&mut buf).is_ok() {
                            let _ = tls_stream.write_all(b"ACK");
                            return true;
                        }
                    }
                    false
                });

                if handle.join().unwrap_or(false) {
                    success_count += 1;
                }
            }
        }

        success_count
    });

    // Give server time to start
    thread::sleep(Duration::from_millis(100));

    // Create client connector
    let cert_der = rustls::pki_types::CertificateDer::from_pem_slice(&cert)
        .expect("Failed to parse certificate");

    let mut root_store = RootCertStore::empty();
    root_store
        .add(cert_der)
        .expect("Failed to add test cert to root store");

    let client_config = Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    );

    // Create multiple client connections
    let mut handles = vec![];

    for i in 0..3 {
        let client_config = client_config.clone();
        let handle = thread::spawn(move || -> std::io::Result<()> {
            let connector = RustlsConnector::with_config(client_config);
            let tcp = TcpStream::connect(server_addr)?;
            let connection = Connection::Tcp(tcp);

            let (mut tls_stream, _) = connector
                .from_tcp_stream("localhost".to_string(), connection)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            tls_stream.write_all(format!("Message {}", i).as_bytes())?;

            let mut response = [0u8; 1024];
            let n = tls_stream.read(&mut response)?;

            if &response[..n] == b"ACK" {
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Bad response",
                ))
            }
        });

        handles.push(handle);
    }

    // Wait for all clients to complete
    let mut client_success = 0;
    for handle in handles {
        if handle
            .join()
            .unwrap_or(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Join failed",
            )))
            .is_ok()
        {
            client_success += 1;
        }
    }

    // Wait for server to complete
    let server_success = server_handle.join().expect("Server thread panicked");

    // Verify all connections succeeded
    assert_eq!(client_success, 3, "Not all clients succeeded");
    assert_eq!(server_success, 3, "Not all server connections succeeded");
}

#[test]
fn test_certificate_verification() {
    let (cert, _key) = load_test_cert();

    // Parse the certificate
    let cert_der = rustls::pki_types::CertificateDer::from_pem_slice(&cert);
    assert!(cert_der.is_ok(), "Failed to parse valid certificate");

    // Add to root store
    let mut root_store = RootCertStore::empty();
    let result = root_store.add(cert_der.unwrap());
    assert!(
        result.is_ok(),
        "Failed to add valid certificate to root store"
    );

    // Verify root store is not empty
    assert!(
        !root_store.is_empty(),
        "Root store should contain certificate"
    );
}
