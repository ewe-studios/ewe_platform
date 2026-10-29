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
/// ```rust
/// use foundation_netio::netcap::ssl::rustls::default_client_config;
///
/// let config = default_client_config();
/// assert!(std::sync::Arc::strong_count(&config) >= 1);
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

impl<T> RustlsStream<T> {
    /// Put the wrapped socket into (or out of) non-blocking mode.
    ///
    /// WHY: rustls handles a non-blocking socket correctly. A write either takes
    /// the plaintext or returns `WouldBlock` without taking it, a flush leaves any
    /// unsent TLS records queued, and the handshake makes progress across
    /// `WouldBlock`s. So the TLS client can park on readiness the same way plain
    /// TCP does.
    ///
    /// # Errors
    /// The socket's `fcntl` error, or an error if the stream mutex is poisoned.
    pub fn set_nonblocking(&self, nonblocking: bool) -> std::io::Result<()> {
        let guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.sock.set_nonblocking(nonblocking)
    }

    /// Whether the wrapped socket is in non-blocking mode.
    ///
    /// # Errors
    /// The socket's error, or an error if the stream mutex is poisoned.
    pub fn is_nonblocking(&self) -> std::io::Result<bool> {
        let guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard.sock.is_nonblocking()
    }

    /// Whether the wrapped socket can be driven non-blocking with readiness
    /// parking. rustls itself always can; the socket underneath decides.
    #[must_use]
    pub fn supports_nonblocking_io(&self) -> bool {
        self.0
            .lock()
            .is_ok_and(|guard| guard.sock.supports_nonblocking_io())
    }
}

impl<T, D> RustlsStream<T>
where
    T: core::ops::Deref<Target = rustls::ConnectionCommon<D>>,
{
    /// The readiness a task stalled on this TLS stream must wait for.
    ///
    /// WHY: the I/O call the caller made does not say which way the socket must
    /// turn. A plaintext *read* can stall on a TLS record rustls still has to
    /// *write* (a handshake message or an alert). A plaintext *write* can stall
    /// mid-handshake waiting for the server's reply, which needs the socket
    /// *readable*. Parking on the wrong direction either never wakes or wakes at
    /// once and spins. So the TLS session's own state picks the direction:
    /// pending TLS output means writable, a handshake still waiting on the
    /// peer means readable, and otherwise the caller's `interest` stands.
    ///
    /// Returns `None` if the mutex is poisoned or the socket cannot report
    /// readiness.
    #[must_use]
    pub fn readiness(
        &self,
        interest: crate::native::readiness::IoInterest,
    ) -> Option<foundation_core::valtron::EventReadinessPtr> {
        use crate::native::readiness::IoInterest;
        let guard = self.0.lock().ok()?;
        let effective = if guard.conn.wants_write() {
            IoInterest::Writable
        } else if guard.conn.is_handshaking() {
            IoInterest::Readable
        } else {
            interest
        };
        guard.sock.readiness(effective)
    }
}

/// A readiness handle for a synchronous I/O driver (`conn_io`) that blocks its
/// thread, on a stream whose TLS connection type is `'static` and `Send + Sync`.
/// Feature 06.
///
/// The handle clones this stream (cheap `Arc` bump) so the driver can query the
/// TLS state (`wants_write` / `is_handshaking`) at each wait. The `'static`
/// bound lets the clone coerce to `Arc<dyn TlsInterest>`.
#[cfg(unix)]
impl<T, D> RustlsStream<T>
where
    T: core::ops::Deref<Target = rustls::ConnectionCommon<D>> + Send + Sync + std::fmt::Debug + 'static,
{
    #[must_use]
    pub fn conn_readiness(&self) -> crate::native::connection::ConnReadiness {
        use std::os::fd::AsRawFd;
        let fd = self.as_raw_fd();
        crate::native::connection::ConnReadiness::Fd(
            fd,
            Some(std::sync::Arc::new(self.clone())),
        )
    }
}
#[cfg(unix)]
impl<T, D> crate::native::connection::TlsInterest for RustlsStream<T>
where
    T: core::ops::Deref<Target = rustls::ConnectionCommon<D>> + Send + Sync + std::fmt::Debug,
{
    fn corrected(
        &self,
        interest: crate::native::readiness::IoInterest,
    ) -> crate::native::readiness::IoInterest {
        use crate::native::readiness::IoInterest;
        let guard = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if guard.conn.wants_write() {
            IoInterest::Writable
        } else if guard.conn.is_handshaking() {
            IoInterest::Readable
        } else {
            interest
        }
    }

    fn fd(&self) -> std::os::unix::io::RawFd {
        use std::os::fd::AsRawFd;
        self.as_raw_fd()
    }
}

impl<T, D> RustlsStream<T>
where
    T: core::ops::DerefMut<Target = rustls::ConnectionCommon<D>>,
{
    /// Ask rustls to refresh the TLS 1.3 traffic keys.
    ///
    /// The next write carries the key update record. Until it is flushed the
    /// session has pending TLS output (`wants_write`), which is the state a
    /// plaintext read must still make progress past (Feature 06: the sync
    /// driver corrects a read to the write direction in this state).
    ///
    /// # Errors
    /// The rustls key-update error, or an error if the stream mutex is poisoned.
    pub fn refresh_traffic_keys(&self) -> std::io::Result<()> {
        let mut guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        guard
            .conn
            .refresh_traffic_keys()
            .map_err(std::io::Error::other)
    }
}

impl RustlsStream<rustls::ClientConnection> {
    /// Drive the TLS handshake on the non-blocking socket until it finishes or
    /// would block.
    ///
    /// WHY: the first `write` of the request used to drive the handshake
    /// implicitly, so the handshake's wait used the body's idle timeout instead
    /// of the connect deadline. Driving it here as its own step lets the caller
    /// use the connect deadline and park on the direction rustls asks for.
    ///
    /// WHAT: calls [`rustls::ConnectionCommon::complete_io`] in a loop while the
    /// handshake makes progress. Returns `Ok(())` once the handshake is done,
    /// `Err(WouldBlock)` when the socket must turn before more progress is
    /// possible, or the handshake's real error (a bad certificate, a protocol
    /// alert, …).
    ///
    /// The caller parks on [`Self::readiness`] when it gets `WouldBlock`; that
    /// method already picks the direction from the TLS session (`wants_write`
    /// first, then "still handshaking").
    ///
    /// # Errors
    /// `WouldBlock` (park and call again), or the handshake's error.
    pub fn drive_handshake(&self) -> std::io::Result<()> {
        let mut guard = self
            .0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))?;
        let rustls::StreamOwned { conn, sock } = &mut *guard;

        loop {
            if !conn.is_handshaking() {
                return Ok(());
            }
            match conn.complete_io(sock) {
                Ok((rdlen, wrlen)) => {
                    // Progress was made: drive again. No progress while still
                    // handshaking cannot normally happen (`complete_io` returns
                    // `WouldBlock` in that case), but do not spin if it does.
                    if rdlen == 0 && wrlen == 0 {
                        return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => return Err(err),
                Err(err) => return Err(err),
            }
        }
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
    /// ```rust
    /// use foundation_netio::netcap::ssl::rustls::RustlsConnector;
    ///
    /// let connector = RustlsConnector::new();
    /// let _ = connector.clone();
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
    /// ```rust
    /// use foundation_netio::netcap::ssl::rustls::{default_client_config, RustlsConnector};
    ///
    /// let connector = RustlsConnector::with_config(default_client_config());
    /// let _ = connector.clone();
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
