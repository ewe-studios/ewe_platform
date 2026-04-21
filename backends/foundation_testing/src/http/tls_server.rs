//! HTTPS test server with self-signed TLS certificate.
//!
//! WHY: Integration tests for HTTP clients need a real HTTPS endpoint.
//! Generated API functions (e.g., Cloudflare, GCP) use hardcoded `https://` URLs,
//! so `StaticSocketAddr` alone isn't enough — the client needs a TLS-speaking server.
//!
//! WHAT: `TestHttpsServer` wraps accepted TCP connections with `RustlsAcceptor` to
//! serve HTTPS. Includes a helper to create a matching `SSLConnector` that trusts
//! the embedded self-signed certificate.
//!
//! HOW: Embeds a self-signed localhost cert/key pair. On accept, performs TLS handshake
//! via `RustlsAcceptor`, then reads/writes HTTP over the TLS stream.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use foundation_core::netcap::connection::Connection;
use foundation_core::netcap::ssl::{SSLAcceptor, SSLConnector};
use foundation_core::wire::simple_http::{Proto, SendSafeBody, SimpleHeaders, SimpleMethod, SimpleUrl};

use zeroize::Zeroizing;

use super::server::{HttpRequest, HttpResponse};

type ResponseHandler = Arc<Mutex<Box<dyn Fn(&HttpRequest) -> HttpResponse + Send>>>;

// Self-signed localhost certificate (valid until Jan 2027).
const TEST_CERT_PEM: &[u8] = include_bytes!("fixtures/cert.pem");
const TEST_KEY_PEM: &[u8] = include_bytes!("fixtures/key.pem");

/// HTTPS test server for integration testing.
///
/// Identical API to `TestHttpServer` but speaks TLS. Use `test_tls_connector()`
/// to obtain a `SSLConnector` that trusts the embedded self-signed certificate.
pub struct TestHttpsServer {
    addr: String,
    _handle: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

impl TestHttpsServer {
    /// Start a default HTTPS test server that responds 200 OK.
    #[must_use]
    pub fn start() -> Self {
        Self::with_response(|_req| HttpResponse::ok(b"OK"))
    }

    /// Start HTTPS server with custom response handler.
    #[must_use]
    pub fn with_response<F>(handler: F) -> Self
    where
        F: Fn(&HttpRequest) -> HttpResponse + Send + 'static,
    {
        let acceptor = SSLAcceptor::from_pem(
            TEST_CERT_PEM.to_vec(),
            Zeroizing::new(TEST_KEY_PEM.to_vec()),
        )
        .expect("Failed to create TLS acceptor from embedded test certificate");

        let listener = TcpListener::bind("127.0.0.1:0")
            .expect("Failed to bind test HTTPS server to localhost");
        let port = listener.local_addr().unwrap().port();
        let addr = format!("https://127.0.0.1:{port}");

        let running = Arc::new(AtomicBool::new(true));
        let handler: ResponseHandler = Arc::new(Mutex::new(Box::new(handler)));

        let running_clone = Arc::clone(&running);
        let handler_clone = Arc::clone(&handler);

        let handle = thread::spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("Failed to set non-blocking");

            while running_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _sock_addr)) => {
                        let handler = Arc::clone(&handler_clone);
                        let acceptor = acceptor.clone();
                        thread::spawn(move || {
                            let conn = Connection::Tcp(stream);
                            let tls_stream = match acceptor.accept(conn) {
                                Ok(s) => s,
                                Err(e) => {
                                    tracing::debug!("TLS accept failed: {e}");
                                    return;
                                }
                            };
                            if let Err(e) = handle_tls_connection(tls_stream, &handler) {
                                tracing::debug!("TestHttpsServer connection error: {e}");
                            }
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => {
                        tracing::debug!("TestHttpsServer accept error: {e}");
                        break;
                    }
                }
            }
        });

        Self {
            addr,
            _handle: Some(handle),
            running,
        }
    }

    /// Get full URL for a path on this test server.
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.addr, path)
    }

    /// Get base URL (e.g., `https://127.0.0.1:12345`).
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.addr
    }

    /// Get the port the server is listening on.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.addr
            .rsplit(':')
            .next()
            .unwrap()
            .parse()
            .expect("valid port in addr")
    }
}

impl Drop for TestHttpsServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// Create a `SSLConnector` that trusts the embedded self-signed test certificate.
///
/// Use this when building a `SimpleHttpClient` that talks to `TestHttpsServer`:
///
/// ```ignore
/// let server = TestHttpsServer::start();
/// let client = SimpleHttpClient::with_resolver(dns)
///     .with_tls_connector(test_tls_connector());
/// ```
#[must_use]
pub fn test_tls_connector() -> SSLConnector {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::CertificateDer;

    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(TEST_CERT_PEM)
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to parse test certificate PEM");

    let mut root_store = rustls::RootCertStore::empty();
    for cert in certs {
        root_store.add(cert).expect("Failed to add test cert to root store");
    }

    let provider = Arc::new(
        foundation_core::netcap::ssl::rustls::initialize_tls_provider()
            .expect("should have TLS provider"),
    );

    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(rustls::ALL_VERSIONS)
        .expect("valid TLS versions")
        .with_root_certificates(root_store)
        .with_no_client_auth();

    SSLConnector::with_config(Arc::new(config))
}

fn handle_tls_connection(
    mut tls_stream: foundation_core::netcap::ssl::ServerSSLStream,
    handler: &ResponseHandler,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = vec![0u8; 8192];
    let mut request_data = Vec::new();

    loop {
        match tls_stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                request_data.extend_from_slice(&buf[..n]);
                if request_data.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(std::time::Duration::from_millis(1));
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }

    let request_str = String::from_utf8_lossy(&request_data);
    let mut lines = request_str.lines();

    let first_line = lines.next().unwrap_or("");
    let mut parts = first_line.split_whitespace();
    let method_str = parts.next().unwrap_or("GET");
    let path_str = parts.next().unwrap_or("/");

    let method = SimpleMethod::from(method_str);
    let path = SimpleUrl::url_with_query(path_str);
    let proto = Proto::HTTP11;
    let headers = SimpleHeaders::default();

    let request = HttpRequest {
        method,
        path,
        proto,
        headers,
        body: SendSafeBody::None,
    };

    let response = {
        let handler_guard = handler.lock().unwrap();
        handler_guard(&request)
    };

    let rendered = response.render();
    tls_stream.write_all(&rendered)?;
    tls_stream.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation_core::wire::simple_http::client::{SimpleHttpClient, StaticSocketAddr};
    use std::net::SocketAddr;

    #[test]
    fn test_https_server_starts() {
        let server = TestHttpsServer::start();
        assert!(server.base_url().starts_with("https://127.0.0.1:"));
    }

    #[test]
    fn test_https_roundtrip() {
        let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

        let server = TestHttpsServer::with_response(|_req| {
            HttpResponse::ok(b"hello from TLS")
        });

        let port = server.port();
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let dns = StaticSocketAddr::new(addr);
        let connector = test_tls_connector();

        let client = SimpleHttpClient::with_resolver(dns)
            .with_tls_connector(connector);

        let response = client
            .get("https://localhost/test")
            .unwrap()
            .build_client()
            .unwrap()
            .send()
            .expect("HTTPS request should succeed");

        assert_eq!(response.get_status().into_usize(), 200);
    }
}
