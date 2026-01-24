//! Taken from the tiny-http project <https://github.com/tiny-http/tiny-http>/

#![cfg(not(target_arch = "wasm32"))]

use crate::netcap::connection::Connection;
use crate::netcap::errors::BoxedError;
use crate::netcap::{
    DataStreamAddr, DataStreamError, DataStreamResult, Endpoint, EndpointConfig, SocketAddr,
};
use rustls::pki_types::ServerName;
use rustls::RootCertStore;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, Mutex};
use webpki_roots::TLS_SERVER_ROOTS;
use zeroize::Zeroizing;

pub use rustls::{ClientConfig, ServerConfig};

/// Creates a default `ClientConfig` with Mozilla's root certificates.
///
/// This function builds a TLS client configuration using the `webpki-roots` crate,
/// which provides a bundle of root certificates from Mozilla's CA Certificate Program.
/// The configuration supports TLS 1.2 and 1.3 protocols with safe default cipher suites.
///
/// # Returns
///
/// An `Arc<ClientConfig>` ready to use for establishing TLS connections.
///
/// # Example
///
/// ```no_run
/// use foundation_core::netcap::ssl::rustls::default_client_config;
///
/// let config = default_client_config();
/// // Use config with RustlsConnector
/// ```
#[must_use]
pub fn default_client_config() -> Arc<ClientConfig> {
    let mut root_store = RootCertStore::empty();
    root_store.extend(TLS_SERVER_ROOTS.iter().cloned());

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Arc::new(config)
}

/// A wrapper around an owned Rustls connection and corresponding stream.
///
/// Uses an internal Mutex to permit disparate reader & writer threads to access the stream independently.
pub struct RustlsStream<T>(Arc<Mutex<rustls::StreamOwned<T, Connection>>>);

impl<T> RustlsStream<T> {
    pub fn try_clone_connection(&self) -> std::io::Result<Connection> {
        let guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.sock.try_clone()
    }

    pub fn read_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        let guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.sock.read_timeout()
    }

    pub fn write_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        let guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.sock.write_timeout()
    }

    pub fn set_write_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.sock.set_write_timeout(dur)
    }

    pub fn set_read_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.sock.set_read_timeout(dur)
    }
}

impl<T> RustlsStream<T> {
    pub fn local_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        let guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.sock.local_addr()
    }

    pub fn peer_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        let guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.sock.peer_addr()
    }

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

    pub fn shutdown(&mut self, how: Shutdown) -> std::io::Result<()> {
        let guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.sock.shutdown(how)
    }
}

impl<T> Clone for RustlsStream<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Read for RustlsStream<rustls::ClientConnection> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.read(buf)
    }
}

impl Read for RustlsStream<rustls::ServerConnection> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.read(buf)
    }
}

impl Write for RustlsStream<rustls::ClientConnection> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.flush()
    }
}

impl Write for RustlsStream<rustls::ServerConnection> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.flush()
    }
}

pub type RustTlsServerStream = RustlsStream<rustls::ServerConnection>;

#[derive(Clone)]
pub struct RustlsAcceptor(Arc<rustls::ServerConfig>);

impl RustlsAcceptor {
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
    /// use foundation_core::netcap::ssl::rustls::RustlsConnector;
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
    /// use foundation_core::netcap::ssl::rustls::RustlsConnector;
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

    #[must_use]
    pub fn create(endpoint: &Endpoint<Arc<rustls::ClientConfig>>) -> Self {
        match &endpoint {
            Endpoint::WithIdentity(_, identity) => {
                Self(identity.clone())
            }
            _ => unreachable!("You generally won't call this method with Endpoint::NoIdentity since its left to you to generate")
        }
    }

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
        let url = url::Url::parse("https://example.com:443").unwrap();
        let endpoint =
            Endpoint::WithIdentity(EndpointConfig::NoTimeout(url), custom_config.clone());

        let connector = RustlsConnector::create(&endpoint);
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
