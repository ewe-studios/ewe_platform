//! Taken from the tiny-http project <https://github.com/tiny-http/tiny-http>/

#![cfg(not(target_family = "wasm"))]

use crate::native::connection::Connection;
use crate::native::connection::{DataStreamAddr, Endpoint, EndpointConfig, SocketAddr};
use crate::shared::errors::{DataStreamError, DataStreamResult};
use foundation_core::io::ioutils::ReadTimeoutOperations;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::ServerName;
use rustls::{RootCertStore, ALL_VERSIONS};
use std::error::Error;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, Mutex};
use webpki_roots::TLS_SERVER_ROOTS;
use zeroize::Zeroizing;

pub use rustls::{ClientConfig, ServerConfig};

/// A server-certificate verifier that accepts any certificate chain but still
/// validates the handshake signatures. Used for the insecure
/// `DOCKER_TLS_VERIFY=0` mode — the TLS session is encrypted, the server is not
/// authenticated.
#[derive(Debug)]
struct NoServerCertVerify(Arc<CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for NoServerCertVerify {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

#[must_use]
pub fn initialize_tls_provider() -> Option<CryptoProvider> {
    // Priority: aws-lc-rs > ring. When both are enabled (--all-features), use aws-lc-rs.
    #[cfg(feature = "ssl-provider-awsrc")]
    {
        Some(rustls::crypto::aws_lc_rs::default_provider())
    }

    #[cfg(all(feature = "ssl-provider-ring", not(feature = "ssl-provider-awsrc")))]
    {
        Some(rustls::crypto::ring::default_provider())
    }

    #[cfg(all(
        not(feature = "ssl-provider-ring"),
        not(feature = "ssl-provider-awsrc")
    ))]
    {
        None
    }
}

/// Creates a default `ClientConfig` with Mozilla's root certificates.
///
/// This function builds a TLS client configuration using the `webpki-roots` crate,
/// which provides a bundle of root certificates from Mozilla's CA Certificate Program.
/// The configuration supports TLS 1.2 and 1.3 protocols with safe default cipher suites.
///
/// # Panics
///
/// 1. Panics if tls provider could not be deteremined.
/// 2. Panics if versions could not be set
///
/// # Returns
///
/// An `Arc<ClientConfig>` ready to use for establishing TLS connections.
///
/// # Example
///
/// ```no_run
/// use foundation_netio::netcap::ssl::rustls::default_client_config;
///
/// let config = default_client_config();
/// // Use config with RustlsConnector
/// ```
#[must_use]
pub fn default_client_config() -> Arc<ClientConfig> {
    let mut root_store = RootCertStore::empty();
    root_store.extend(TLS_SERVER_ROOTS.iter().cloned());

    let provider = Arc::new(initialize_tls_provider().expect("should generate provider"));
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(ALL_VERSIONS)
        .expect("correct versions")
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Arc::new(config)
}

/// A wrapper around an owned Rustls connection and corresponding stream.
///
/// Uses an internal Mutex to permit disparate reader & writer threads to access the stream independently.
#[derive(Debug)]
pub struct RustlsStream<T>(Arc<Mutex<rustls::StreamOwned<T, Connection>>>);

/// Expose the fd of the TCP socket the TLS session wraps so the connection can
/// be registered with the reactor (Decision 12 §12). Readiness lives on the
/// socket, not the TLS layer. Unix-only (native-socket).
#[cfg(unix)]
impl<T> std::os::unix::io::AsRawFd for RustlsStream<T> {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        // Reading the fd only touches the socket handle; recover from a poisoned
        // lock rather than panic (the fd is still valid).
        let guard = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.sock.as_raw_fd()
    }
}

#[cfg(unix)]
impl<T> std::os::unix::io::AsFd for RustlsStream<T> {
    fn as_fd(&self) -> std::os::unix::io::BorrowedFd<'_> {
        // SAFETY: the fd is owned by the wrapped socket and outlives the borrow.
        unsafe {
            std::os::unix::io::BorrowedFd::borrow_raw(
                <Self as std::os::unix::io::AsRawFd>::as_raw_fd(self),
            )
        }
    }
}

impl<T> RustlsStream<T> {
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn try_clone_connection(&self) -> std::io::Result<Connection> {
        let guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.sock.try_clone()
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn read_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        let guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.sock.read_timeout()
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn write_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        let guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.sock.write_timeout()
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn set_write_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        let mut guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.sock.set_write_timeout(dur)
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn set_read_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        let mut guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.sock.set_read_timeout(dur)
    }
}

impl<T> RustlsStream<T> {
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn local_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        let guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.sock.local_addr()
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn peer_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        let guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.sock.peer_addr()
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn stream_addr(&self) -> DataStreamResult<DataStreamAddr> {
        let local_addr = self.local_addr()?;
        let peer_addr = self.peer_addr()?;

        match (local_addr, peer_addr) {
            (Some(l1), Some(l2)) => Ok(DataStreamAddr::new(l1, Some(l2))),
            (Some(l1), None) => Ok(DataStreamAddr::new(l1, None)),
            (None, Some(_)) => Err(DataStreamError::NoLocalAddr),
            _ => Err(DataStreamError::NoAddr),
        }
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn shutdown(&mut self, how: Shutdown) -> std::io::Result<()> {
        let guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.sock.shutdown(how)
    }
}

impl<T> Clone for RustlsStream<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl ReadTimeoutOperations for RustlsStream<rustls::ClientConnection> {
    fn read_timeout_into(
        &mut self,
        buf: &mut [u8],
        timeout: std::time::Duration,
    ) -> std::result::Result<usize, std::io::Error> {
        let current_timeout = self.read_timeout()?;

        self.set_read_timeout(Some(timeout))?;

        let read_result = self.read(buf);

        self.set_read_timeout(current_timeout)?;

        read_result
    }

    fn set_read_timeout_as(
        &mut self,
        timeout: std::time::Duration,
    ) -> std::result::Result<(), std::io::Error> {
        self.set_read_timeout(Some(timeout))
    }

    fn get_current_read_timeout(
        &self,
    ) -> std::result::Result<Option<std::time::Duration>, std::io::Error> {
        self.read_timeout()
    }
}

impl ReadTimeoutOperations for RustlsStream<rustls::ServerConnection> {
    fn read_timeout_into(
        &mut self,
        buf: &mut [u8],
        timeout: std::time::Duration,
    ) -> std::result::Result<usize, std::io::Error> {
        let current_timeout = self.read_timeout()?;

        self.set_read_timeout(Some(timeout))?;

        let read_result = self.read(buf);

        self.set_read_timeout(current_timeout)?;

        read_result
    }

    fn set_read_timeout_as(
        &mut self,
        timeout: std::time::Duration,
    ) -> std::result::Result<(), std::io::Error> {
        self.set_read_timeout(Some(timeout))
    }

    fn get_current_read_timeout(
        &self,
    ) -> std::result::Result<Option<std::time::Duration>, std::io::Error> {
        self.read_timeout()
    }
}

impl Read for RustlsStream<rustls::ClientConnection> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.read(buf)
    }
}

impl Read for RustlsStream<rustls::ServerConnection> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.read(buf)
    }
}

impl Write for RustlsStream<rustls::ClientConnection> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.flush()
    }
}

impl Write for RustlsStream<rustls::ServerConnection> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.flush()
    }
}

pub type RustTlsServerStream = RustlsStream<rustls::ServerConnection>;

#[derive(Clone)]
pub struct RustlsAcceptor(Arc<rustls::ServerConfig>);

impl RustlsAcceptor {
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn from_pem(
        certificates: Vec<u8>,
        private_key: Zeroizing<Vec<u8>>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        use rustls::pki_types::pem::PemObject;
        use rustls::pki_types::{CertificateDer, PrivateKeyDer};

        let certs_result: Result<
            Vec<rustls::pki_types::CertificateDer<'static>>,
            rustls::pki_types::pem::Error,
        > = CertificateDer::pem_slice_iter(certificates.as_slice()).collect();

        let certs = certs_result?;
        let p_key = PrivateKeyDer::from_pem_slice(private_key.as_slice())?;

        let tls_conf = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, p_key)?;

        Ok(Self(Arc::new(tls_conf)))
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn accept(
        &self,
        stream: Connection,
    ) -> Result<RustTlsServerStream, Box<dyn Error + Send + Sync + 'static>> {
        let connection = rustls::ServerConnection::new(self.0.clone())?;
        Ok(RustlsStream(Arc::new(Mutex::new(
            rustls::StreamOwned::new(connection, stream),
        ))))
    }
}

#[derive(Clone)]
pub struct RustlsConnector(Arc<rustls::ClientConfig>);

pub type RustTlsClientStream = RustlsStream<rustls::ClientConnection>;

impl RustlsConnector {
    /// Creates a new `RustlsConnector` with default root certificates from Mozilla's CA Certificate Program.
    ///
    /// This is a convenience method that uses [`default_client_config()`] to create a connector
    /// with sensible defaults for most HTTPS connections.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use foundation_netio::netcap::ssl::rustls::RustlsConnector;
    ///
    /// let connector = RustlsConnector::new();
    /// // connector is ready to establish TLS connections
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self(default_client_config())
    }

    /// Creates a new `RustlsConnector` with a custom `ClientConfig`.
    ///
    /// Use this when you need to customize the TLS configuration, such as:
    /// - Using custom root certificates
    /// - Disabling certificate validation (testing only!)
    /// - Configuring client authentication
    ///
    /// # Example
    ///
    /// ```no_run
    /// use foundation_netio::netcap::ssl::rustls::RustlsConnector;
    /// use std::sync::Arc;
    ///
    /// let config = rustls::ClientConfig::builder()
    ///     .with_root_certificates(rustls::RootCertStore::empty())
    ///     .with_no_client_auth();
    ///
    /// let connector = RustlsConnector::with_config(Arc::new(config));
    /// ```
    #[must_use]
    pub fn with_config(config: Arc<rustls::ClientConfig>) -> Self {
        Self(config)
    }

    /// Build a **mutual-TLS** client connector from PEM material — the client
    /// presents `cert_pem`/`key_pem` and (when `verify`) checks the server
    /// against `ca_pem` (or the Mozilla roots if `ca_pem` is `None`).
    ///
    /// This is exactly the Docker-over-TLS shape (`ca.pem` / `cert.pem` /
    /// `key.pem`, `DOCKER_TLS_VERIFY`). `verify = false` accepts any server
    /// certificate (handshake signatures are still checked) — the insecure
    /// `DOCKER_TLS_VERIFY=0` mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the PEM cannot be parsed, no crypto provider is
    /// available, or the client-auth key is rejected.
    pub fn from_client_mutual_pem(
        ca_pem: Option<&[u8]>,
        cert_pem: &[u8],
        key_pem: &[u8],
        verify: bool,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        use rustls::pki_types::pem::PemObject;
        use rustls::pki_types::{CertificateDer, PrivateKeyDer};

        let cert_chain: Vec<CertificateDer<'static>> =
            CertificateDer::pem_slice_iter(cert_pem).collect::<Result<_, _>>()?;
        let key = PrivateKeyDer::from_pem_slice(key_pem)?;

        let provider = Arc::new(initialize_tls_provider().ok_or("no rustls crypto provider")?);
        let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(ALL_VERSIONS)?;

        let config = if verify {
            let mut roots = RootCertStore::empty();
            if let Some(ca) = ca_pem {
                for cert in CertificateDer::pem_slice_iter(ca) {
                    roots.add(cert?)?;
                }
            } else {
                roots.extend(TLS_SERVER_ROOTS.iter().cloned());
            }
            builder
                .with_root_certificates(roots)
                .with_client_auth_cert(cert_chain, key)?
        } else {
            builder
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoServerCertVerify(provider)))
                .with_client_auth_cert(cert_chain, key)?
        };
        Ok(Self(Arc::new(config)))
    }

    #[must_use]
    pub fn create(endpoint: &Endpoint<Arc<rustls::ClientConfig>>) -> Self {
        match &endpoint {
            Endpoint::WithIdentity(_, identity) => {
                Self(identity.clone())
            }
            _ => unreachable!("You generally won't call this method with Endpoint::NoIdentity since its left to you to generate")
        }
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn client_tls_from_endpoint(
        endpoint: &Endpoint<Arc<rustls::ClientConfig>>,
    ) -> Result<(RustTlsClientStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let connector = Self::create(endpoint);
        connector.from_endpoint(endpoint)
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn from_tcp_stream(
        &self,
        sni: String,
        plain: Connection,
    ) -> Result<(RustTlsClientStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let local_addr = plain.local_addr()?;
        let peer_addr = plain.peer_addr()?;

        let addr = match (local_addr, peer_addr) {
            (Some(l1), Some(l2)) => Ok(DataStreamAddr::new(l1, Some(l2))),
            (Some(l1), None) => Ok(DataStreamAddr::new(l1, None)),
            (None, Some(_)) => Err(DataStreamError::NoPeerAddr),
            _ => Err(DataStreamError::NoAddr),
        }?;

        let server_name: ServerName = sni.try_into().map_err(Box::new)?;
        let conn = rustls::ClientConnection::new(self.0.clone(), server_name)?;
        let ssl_stream = rustls::StreamOwned::new(conn, plain);
        let shared_stream = Arc::new(Mutex::new(ssl_stream));

        Ok((RustlsStream(shared_stream), addr))
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn from_endpoint(
        &self,
        endpoint: &Endpoint<Arc<rustls::ClientConfig>>,
    ) -> Result<(RustTlsClientStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let host = endpoint.host();
        let host_socket_addr: core::net::SocketAddr = host.parse()?;

        let plain_stream = match endpoint {
            Endpoint::WithDefault(config) => match config {
                EndpointConfig::WithTimeout(_, timeout) => {
                    TcpStream::connect_timeout(&host_socket_addr, *timeout)
                }
                _ => TcpStream::connect(host_socket_addr),
            },
            Endpoint::WithIdentity(config, _) => match config {
                EndpointConfig::WithTimeout(_, timeout) => {
                    TcpStream::connect_timeout(&host_socket_addr, *timeout)
                }
                _ => TcpStream::connect(host_socket_addr),
            },
        }?;

        self.from_tcp_stream(host, Connection::Tcp(plain_stream))
    }
}

impl Default for RustlsConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_client_config() {
        let config = default_client_config();
        // Verify config is created successfully
        assert!(Arc::strong_count(&config) == 1);
    }

    #[test]
    fn test_rustls_connector_new() {
        let connector = RustlsConnector::new();
        // Verify connector is created with default config
        assert!(Arc::strong_count(&connector.0) >= 1);
    }

    #[test]
    fn test_rustls_connector_default() {
        let connector = RustlsConnector::default();
        // Verify Default trait works
        assert!(Arc::strong_count(&connector.0) >= 1);
    }

    #[test]
    fn test_rustls_connector_clone() {
        let connector1 = RustlsConnector::new();
        let connector2 = connector1.clone();
        // Verify Arc is shared
        assert!(Arc::strong_count(&connector1.0) == 2);
        assert!(Arc::strong_count(&connector2.0) == 2);
    }

    #[test]
    fn test_rustls_acceptor_from_pem_invalid() {
        // Test with invalid certificate data
        let certs = vec![0u8; 10];
        let key = Zeroizing::new(vec![0u8; 10]);

        let result = RustlsAcceptor::from_pem(certs, key);
        // Should fail with invalid PEM data
        assert!(result.is_err());
    }

    #[test]
    fn test_server_name_parsing() {
        // Test valid SNI
        let valid_sni = "example.com";
        let result: Result<ServerName, _> = valid_sni.try_into();
        assert!(result.is_ok());

        // Test invalid SNI (empty)
        let invalid_sni = "";
        let result: Result<ServerName, _> = invalid_sni.try_into();
        assert!(result.is_err());

        // Test invalid SNI (too long)
        let too_long = "a".repeat(300);
        let result: Result<ServerName, _> = too_long.as_str().try_into();
        assert!(result.is_err());
    }

    #[test]
    fn test_rustls_connector_create_from_endpoint() {
        let custom_config = default_client_config();
        let uri = foundation_core::url::Uri::parse("https://example.com:443").unwrap();
        let endpoint =
            Endpoint::WithIdentity(EndpointConfig::NoTimeout(uri), custom_config.clone());

        let _ = RustlsConnector::create(&endpoint);

        // Verify the connector uses the custom config
        assert!(Arc::strong_count(&custom_config) >= 2);
    }

    #[test]
    fn test_root_cert_store_not_empty() {
        let _config = default_client_config();
        // The config should have root certificates loaded
        // We can't directly inspect the root store, but we know it was created with TLS_SERVER_ROOTS
        assert!(!TLS_SERVER_ROOTS.is_empty());
    }
}
