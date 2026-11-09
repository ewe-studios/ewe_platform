//! Taken from the tiny-http project https://github.com/tiny-http/tiny-http/

#![cfg(not(target_family = "wasm"))]

use crate::native::connection::Connection;
use crate::native::connection::{ConnReadiness, DataStreamAddr, Endpoint, SocketAddr};
use crate::shared::errors::{DataStreamError, DataStreamResult};
use foundation_core::io::ioutils::{PeekError, PeekableReadStream};
use std::error::Error;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

pub use native_tls::{Identity, TlsConnector, TlsStream};

/// The state of a `native_tls` client stream.
///
/// WHY: `TlsConnector::connect` runs the handshake and on a non-blocking socket
/// stops with `WouldBlock`, handing back a `MidHandshakeTlsStream`. Keeping it
/// lets a valtron task resume on readiness instead of holding a worker in a
/// blocking handshake (issues #277 and #279).
enum NativeTlsState {
    /// The handshake started and needs more socket readiness.
    Handshaking(native_tls::MidHandshakeTlsStream<Connection>),
    /// The handshake finished.
    Ready(native_tls::TlsStream<Connection>),
    /// The handshake failed; the message is kept for later calls.
    Failed(String),
}

/// A wrapper around a `native_tls` stream.
///
/// Uses an internal Mutex to permit disparate reader & writer threads to access the stream independently.
pub struct NativeTlsStream {
    inner: Arc<Mutex<NativeTlsState>>,
    /// The wrapped socket fd, kept for readiness registration.
    #[cfg(unix)]
    fd: std::os::unix::io::RawFd,
}

impl Clone for NativeTlsStream {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            #[cfg(unix)]
            fd: self.fd,
        }
    }
}

impl core::fmt::Debug for NativeTlsStream {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("NativeTlsStream(Arc<Mutex<NativeTlsState>>)")
            .finish()
    }
}

/// Expose the fd of the TCP socket the TLS session wraps so the connection can
/// be registered with the reactor (Decision 12 §12). Readiness lives on the
/// socket, not the TLS layer. Unix-only (native-socket).
#[cfg(unix)]
impl std::os::unix::io::AsRawFd for NativeTlsStream {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.fd
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsFd for NativeTlsStream {
    fn as_fd(&self) -> std::os::unix::io::BorrowedFd<'_> {
        // SAFETY: the fd is owned by the wrapped socket and outlives the borrow.
        unsafe {
            std::os::unix::io::BorrowedFd::borrow_raw(
                <Self as std::os::unix::io::AsRawFd>::as_raw_fd(self),
            )
        }
    }
}

impl NativeTlsStream {
    /// Wrap a finished handshake.
    fn ready(stream: native_tls::TlsStream<Connection>) -> Self {
        #[cfg(unix)]
        let fd = {
            use std::os::unix::io::AsRawFd;
            stream.get_ref().as_raw_fd()
        };
        Self {
            inner: Arc::new(Mutex::new(NativeTlsState::Ready(stream))),
            #[cfg(unix)]
            fd,
        }
    }

    fn lock(&self) -> std::io::Result<std::sync::MutexGuard<'_, NativeTlsState>> {
        self.inner
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))
    }

    fn connection(&self) -> std::io::Result<&Connection> {
        let guard = self.lock()?;
        match &*guard {
            NativeTlsState::Handshaking(mid) => Ok(mid.get_ref()),
            NativeTlsState::Ready(stream) => Ok(stream.get_ref()),
            NativeTlsState::Failed(message) => Err(std::io::Error::other(message.clone())),
        }
    }

    fn with_connection<R>(
        &self,
        f: impl FnOnce(&mut Connection) -> std::io::Result<R>,
    ) -> std::io::Result<R> {
        let mut guard = self.lock()?;
        match &mut *guard {
            NativeTlsState::Handshaking(mid) => f(mid.get_mut()),
            NativeTlsState::Ready(stream) => f(stream.get_mut()),
            NativeTlsState::Failed(message) => Err(std::io::Error::other(message.clone())),
        }
    }

    fn is_handshaking(&self) -> bool {
        matches!(
            &*self
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            NativeTlsState::Handshaking(_)
        )
    }

    /// A failed handshake consumes the socket, so the kept fd is stale.
    fn is_failed(&self) -> bool {
        matches!(
            &*self
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            NativeTlsState::Failed(_)
        )
    }

    /// Drive the `native_tls` handshake until it finishes or the socket would
    /// block.
    ///
    /// WHY: a valtron task parks on [`Self::readiness`] when this returns
    /// `WouldBlock` (issue #277). The stream must be non-blocking, or this call
    /// waits in the kernel instead.
    ///
    /// # Errors
    /// `WouldBlock` when the socket must turn, or the handshake error.
    pub fn drive_handshake(&self) -> std::io::Result<()> {
        let mut guard = self.lock()?;
        let state = std::mem::replace(
            &mut *guard,
            NativeTlsState::Failed("the handshake is being driven".to_string()),
        );
        match state {
            NativeTlsState::Ready(stream) => {
                *guard = NativeTlsState::Ready(stream);
                Ok(())
            }
            NativeTlsState::Handshaking(mid) => match mid.handshake() {
                Ok(stream) => {
                    *guard = NativeTlsState::Ready(stream);
                    Ok(())
                }
                Err(native_tls::HandshakeError::WouldBlock(mid)) => {
                    *guard = NativeTlsState::Handshaking(mid);
                    Err(std::io::Error::from(std::io::ErrorKind::WouldBlock))
                }
                Err(native_tls::HandshakeError::Failure(err)) => {
                    let message = err.to_string();
                    *guard = NativeTlsState::Failed(message.clone());
                    Err(std::io::Error::other(message))
                }
            },
            NativeTlsState::Failed(message) => {
                *guard = NativeTlsState::Failed(message.clone());
                Err(std::io::Error::other(message))
            }
        }
    }

    pub fn read_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        self.connection()?.read_timeout()
    }

    pub fn write_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        self.connection()?.write_timeout()
    }

    pub fn set_write_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        self.with_connection(|conn| conn.set_write_timeout(dur))
    }

    pub fn set_read_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        self.with_connection(|conn| conn.set_read_timeout(dur))
    }

    /// Put the wrapped socket into (or out of) non-blocking mode.
    ///
    /// # Errors
    /// The socket's error, or an error if the stream lock is poisoned.
    pub fn set_nonblocking(&self, nonblocking: bool) -> std::io::Result<()> {
        self.with_connection(|conn| conn.set_nonblocking(nonblocking))
    }

    /// Whether the wrapped socket is in non-blocking mode.
    ///
    /// # Errors
    /// The socket's error, or an error if the stream lock is poisoned.
    pub fn is_nonblocking(&self) -> std::io::Result<bool> {
        self.connection()?.is_nonblocking()
    }

    /// Whether the wrapped socket can be driven non-blocking with readiness
    /// parking. The socket underneath decides.
    #[must_use]
    pub fn supports_nonblocking_io(&self) -> bool {
        self.connection()
            .is_ok_and(Connection::supports_nonblocking_io)
    }

    /// The readiness a task stalled on this TLS stream must wait for.
    ///
    /// `native_tls` does not report whether a stalled handshake needs the socket
    /// readable or writable, so a pending handshake waits for read readiness
    /// (the peer's next handshake message). A finished handshake uses the
    /// caller's `interest`.
    #[cfg(unix)]
    #[must_use]
    pub fn readiness(
        &self,
        interest: crate::native::readiness::IoInterest,
    ) -> Option<foundation_core::valtron::EventReadinessPtr> {
        if self.is_failed() {
            return None;
        }
        let effective = if self.is_handshaking() {
            crate::native::readiness::IoInterest::Readable
        } else {
            interest
        };
        Some(std::sync::Arc::new(
            crate::native::readiness::SocketReadiness::new(self.fd, effective),
        ))
    }

    /// A readiness handle for a synchronous I/O driver (`conn_io`) that blocks
    /// its thread. Feature 06.
    #[cfg(unix)]
    #[must_use]
    pub fn conn_readiness(&self) -> ConnReadiness {
        ConnReadiness::Fd(self.fd, Some(std::sync::Arc::new(self.clone())))
    }

    /// The fd and direction the pending handshake needs, if any.
    #[cfg(unix)]
    #[must_use]
    pub fn handshake_poll_target(
        &self,
    ) -> Option<(
        std::os::unix::io::RawFd,
        crate::native::readiness::IoInterest,
    )> {
        self.conn_readiness()
            .poll_fd(crate::native::readiness::IoInterest::Readable)
    }
}

#[cfg(unix)]
impl crate::native::connection::TlsInterest for NativeTlsStream {
    fn corrected(
        &self,
        interest: crate::native::readiness::IoInterest,
    ) -> crate::native::readiness::IoInterest {
        if self.is_handshaking() {
            crate::native::readiness::IoInterest::Readable
        } else {
            interest
        }
    }

    fn fd(&self) -> std::os::unix::io::RawFd {
        self.fd
    }
}

// These struct methods form the implict contract for swappable TLS implementations
impl NativeTlsStream {
    pub fn try_clone_connection(&self) -> std::io::Result<Connection> {
        self.connection()?.try_clone()
    }

    pub fn local_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        self.with_connection(|conn| conn.local_addr())
    }

    pub fn peer_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        self.with_connection(|conn| conn.peer_addr())
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
        self.with_connection(|conn| conn.shutdown(how))
    }
}

impl PeekableReadStream for NativeTlsStream {
    fn peek(&mut self, buf: &mut [u8]) -> std::result::Result<usize, PeekError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| PeekError::LockAcquisitionError)?;
        match &mut *guard {
            NativeTlsState::Ready(stream) => stream.get_mut().peek(buf),
            NativeTlsState::Handshaking(_) => Err(PeekError::IOError(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "the TLS handshake is still in progress",
            ))),
            NativeTlsState::Failed(message) => {
                Err(PeekError::IOError(std::io::Error::other(message.clone())))
            }
        }
    }
}

impl Read for NativeTlsStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut guard = self.lock()?;
        match &mut *guard {
            NativeTlsState::Ready(stream) => stream.read(buf),
            NativeTlsState::Handshaking(_) => Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "the TLS handshake is still in progress",
            )),
            NativeTlsState::Failed(message) => Err(std::io::Error::other(message.clone())),
        }
    }
}

impl Write for NativeTlsStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut guard = self.lock()?;
        match &mut *guard {
            NativeTlsState::Ready(stream) => stream.write(buf),
            NativeTlsState::Handshaking(_) => Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "the TLS handshake is still in progress",
            )),
            NativeTlsState::Failed(message) => Err(std::io::Error::other(message.clone())),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut guard = self.lock()?;
        match &mut *guard {
            NativeTlsState::Ready(stream) => stream.flush(),
            NativeTlsState::Handshaking(_) => Ok(()),
            NativeTlsState::Failed(message) => Err(std::io::Error::other(message.clone())),
        }
    }
}

impl foundation_core::io::ioutils::ReadTimeoutOperations for NativeTlsStream {
    fn read_timeout_into(
        &mut self,
        buf: &mut [u8],
        timeout: std::time::Duration,
    ) -> std::io::Result<usize> {
        let previous = self.read_timeout()?;
        self.set_read_timeout(Some(timeout))?;
        let result = self.read(buf);
        self.set_read_timeout(previous)?;
        result
    }

    fn set_read_timeout_as(&mut self, timeout: std::time::Duration) -> std::io::Result<()> {
        self.set_read_timeout(Some(timeout))
    }

    fn get_current_read_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        self.read_timeout()
    }
}

// Implementation for accepting incoming client connection within a TLS servers.
pub struct NativeTlsAcceptor(native_tls::TlsAcceptor);

impl NativeTlsAcceptor {
    pub fn from_identity(identity: Identity) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let acceptor = native_tls::TlsAcceptor::new(identity)?;
        Ok(Self(acceptor))
    }

    pub fn from_der2(
        der: Vec<u8>,
        password: Zeroizing<Vec<u8>>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let identity = native_tls::Identity::from_pkcs8(&der, &password)?;
        Self::from_identity(identity)
    }

    pub fn from_der(
        der: Vec<u8>,
        password: Zeroizing<String>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let identity = native_tls::Identity::from_pkcs12(&der, &password)?;
        Self::from_identity(identity)
    }

    pub fn from_pem(
        certificates: Vec<u8>,
        private_key: Zeroizing<Vec<u8>>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let identity = native_tls::Identity::from_pkcs8(&certificates, &private_key)?;
        Self::from_identity(identity)
    }

    pub fn accept(
        &self,
        stream: Connection,
    ) -> Result<NativeTlsStream, Box<dyn Error + Send + Sync + 'static>> {
        let stream = self.0.accept(stream)?;
        Ok(NativeTlsStream::ready(stream))
    }
}

// Implementation for creating client connection to TLS servers.
pub struct NativeTlsConnector(Arc<native_tls::TlsConnector>);

impl NativeTlsConnector {
    pub fn new() -> Self {
        let connector = TlsConnector::new().expect("should generate tls connector");
        Self(std::sync::Arc::new(connector))
    }

    pub fn create(endpoint: &Endpoint<Arc<native_tls::TlsConnector>>) -> Self {
        match &endpoint {
            Endpoint::WithIdentity(_config, identity) => {
                Self(identity.clone())
            }
            _ => unreachable!("You generally won't call this method with Endpoint::NoIdentity since its left to you to generate")
        }
    }

    pub fn client_tls_from_endpoint(
        endpoint: &Endpoint<Arc<native_tls::TlsConnector>>,
    ) -> Result<(NativeTlsStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let connector = Self::create(endpoint);
        connector.from_endpoint(endpoint)
    }

    /// Wrap a plain connection and start the TLS handshake.
    ///
    /// `native_tls` runs the handshake inside `connect`, so the call returns a
    /// pending stream on a non-blocking socket (issue #277). A task resumes it
    /// with the stream's private handshake driver; a sync caller uses
    /// [`Self::from_tcp_stream_complete`].
    ///
    /// The plain connection must be non-blocking for the call not to wait in
    /// the kernel.
    pub fn from_tcp_stream(
        &self,
        sni: String,
        plain: Connection,
    ) -> Result<(NativeTlsStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let local_addr = plain.local_addr()?;
        let peer_addr = plain.peer_addr()?;

        let addr = match (local_addr, peer_addr) {
            (Some(l1), Some(l2)) => Ok(DataStreamAddr::new(l1, Some(l2))),
            (Some(l1), None) => Ok(DataStreamAddr::new(l1, None)),
            (None, Some(_)) => Err(DataStreamError::NoPeerAddr),
            _ => Err(DataStreamError::NoAddr),
        }?;

        #[cfg(unix)]
        let fd = {
            use std::os::unix::io::AsRawFd;
            plain.as_raw_fd()
        };

        let state = match self.0.connect(sni.as_str(), plain) {
            Ok(stream) => NativeTlsState::Ready(stream),
            Err(native_tls::HandshakeError::WouldBlock(mid)) => NativeTlsState::Handshaking(mid),
            Err(native_tls::HandshakeError::Failure(err)) => return Err(Box::new(err)),
        };

        Ok((
            NativeTlsStream {
                inner: Arc::new(Mutex::new(state)),
                #[cfg(unix)]
                fd,
            },
            addr,
        ))
    }

    /// Wrap a plain connection and drive the TLS handshake to completion.
    ///
    /// The returned stream is blocking, as a synchronous caller expects.
    ///
    /// # Errors
    /// The handshake failed or the timeout expired.
    pub fn from_tcp_stream_complete(
        &self,
        sni: String,
        plain: Connection,
        timeout: Option<std::time::Duration>,
    ) -> Result<(NativeTlsStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let (stream, addr) = self.from_tcp_stream(sni, plain)?;
        finish_handshake(&stream, timeout)
            .map_err(|err| Box::new(err) as Box<dyn Error + Send + Sync>)?;
        stream.set_nonblocking(false)?;
        Ok((stream, addr))
    }

    /// Connect to the endpoint and finish the TLS handshake.
    ///
    /// The connect runs on a non-blocking socket and the handshake is driven by
    /// readiness, so no blocking socket call holds a worker (issues #277 and
    /// #279).
    ///
    /// # Errors
    /// The connect or handshake failed, or the timeout expired.
    pub fn from_endpoint(
        &self,
        endpoint: &Endpoint<Arc<native_tls::TlsConnector>>,
    ) -> Result<(NativeTlsStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let host = endpoint.host();
        let host_socket_addr: core::net::SocketAddr = host.parse()?;
        let timeout = super::endpoint_timeout(endpoint);

        let plain_stream = super::connect_plain(host_socket_addr, timeout)?;
        self.from_tcp_stream_complete(host, Connection::Tcp(plain_stream), timeout)
    }
}

/// Drive the handshake to completion on readiness, blocking the calling thread.
#[cfg(unix)]
fn finish_handshake(
    stream: &NativeTlsStream,
    timeout: Option<std::time::Duration>,
) -> std::io::Result<()> {
    crate::native::readiness::complete_handshake_blocking(
        || stream.drive_handshake(),
        || stream.handshake_poll_target(),
        timeout,
    )
}

/// Drive the handshake on a blocking socket (non-unix platforms have no
/// `poll(2)` readiness here).
#[cfg(not(unix))]
fn finish_handshake(
    stream: &NativeTlsStream,
    _timeout: Option<std::time::Duration>,
) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.drive_handshake()
}
