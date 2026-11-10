//! Taken from the tiny-http project https://github.com/tiny-http/tiny-http/

use crate::native::connection::Connection;
use crate::native::connection::{ConnReadiness, DataStreamAddr, Endpoint, SocketAddr};
use crate::shared::errors::{DataStreamError, DataStreamResult};
use std::error::Error;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

pub use openssl::ssl::SslConnector;

/// The state of an OpenSSL client stream.
///
/// WHY: OpenSSL's handshake API is the only way to start a session and it can
/// stop midway on a non-blocking socket. Keeping the `MidHandshakeSslStream`
/// lets a valtron task resume the handshake when readiness arrives (issue
/// #277), instead of holding a worker in a blocking call (issue #279).
enum OpenSslState {
    /// The handshake started and needs more socket readiness.
    Handshaking(openssl::ssl::MidHandshakeSslStream<Connection>),
    /// The handshake finished.
    Ready(openssl::ssl::SslStream<Connection>),
    /// The handshake failed; the message is kept for later calls.
    Failed(String),
}

pub struct OpenSslStream {
    /// `None` only while a method moves the state out to drive it.
    state: Option<OpenSslState>,
    /// The socket direction the pending handshake needs.
    want: crate::native::readiness::IoInterest,
    /// The wrapped socket fd, kept for readiness registration. A failed
    /// handshake consumes the stream, so the fd is not reachable after it.
    #[cfg(unix)]
    fd: std::os::unix::io::RawFd,
}

impl OpenSslStream {
    /// Wrap a finished handshake.
    fn ready(inner: openssl::ssl::SslStream<Connection>) -> Self {
        #[cfg(unix)]
        let fd = {
            use std::os::unix::io::AsRawFd;
            inner.get_ref().as_raw_fd()
        };
        Self {
            state: Some(OpenSslState::Ready(inner)),
            want: crate::native::readiness::IoInterest::Readable,
            #[cfg(unix)]
            fd,
        }
    }

    fn connection(&self) -> std::io::Result<&Connection> {
        match self.state.as_ref() {
            Some(OpenSslState::Handshaking(mid)) => Ok(mid.get_ref()),
            Some(OpenSslState::Ready(stream)) => Ok(stream.get_ref()),
            Some(OpenSslState::Failed(message)) => Err(std::io::Error::other(message.clone())),
            None => Err(std::io::Error::other("the TLS stream has no state")),
        }
    }

    fn connection_mut(&mut self) -> std::io::Result<&mut Connection> {
        match self.state.as_mut() {
            Some(OpenSslState::Handshaking(mid)) => Ok(mid.get_mut()),
            Some(OpenSslState::Ready(stream)) => Ok(stream.get_mut()),
            Some(OpenSslState::Failed(message)) => Err(std::io::Error::other(message.clone())),
            None => Err(std::io::Error::other("the TLS stream has no state")),
        }
    }

    fn is_handshaking(&self) -> bool {
        matches!(self.state, Some(OpenSslState::Handshaking(_)))
    }

    fn is_failed(&self) -> bool {
        matches!(&self.state, Some(OpenSslState::Failed(_)) | None)
    }

    /// Drive the handshake until it finishes or the socket would block.
    ///
    /// # Errors
    /// `WouldBlock` when the socket must turn, or the handshake error.
    fn drive_handshake(&mut self) -> std::io::Result<()> {
        use openssl::error::ErrorCode;
        use openssl::ssl::HandshakeError;

        loop {
            match self.state.take() {
                Some(OpenSslState::Ready(stream)) => {
                    self.state = Some(OpenSslState::Ready(stream));
                    return Ok(());
                }
                Some(OpenSslState::Handshaking(mid)) => match mid.handshake() {
                    Ok(stream) => {
                        self.state = Some(OpenSslState::Ready(stream));
                        return Ok(());
                    }
                    Err(HandshakeError::WouldBlock(mid)) => {
                        self.want = if mid.error().code() == ErrorCode::WANT_WRITE {
                            crate::native::readiness::IoInterest::Writable
                        } else {
                            crate::native::readiness::IoInterest::Readable
                        };
                        self.state = Some(OpenSslState::Handshaking(mid));
                        return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
                    }
                    Err(HandshakeError::Failure(mid)) => {
                        let message = mid.into_error().to_string();
                        self.state = Some(OpenSslState::Failed(message.clone()));
                        return Err(std::io::Error::other(message));
                    }
                    Err(HandshakeError::SetupFailure(err)) => {
                        let message = err.to_string();
                        self.state = Some(OpenSslState::Failed(message.clone()));
                        return Err(std::io::Error::other(message));
                    }
                },
                Some(OpenSslState::Failed(message)) => {
                    self.state = Some(OpenSslState::Failed(message.clone()));
                    return Err(std::io::Error::other(message));
                }
                None => return Err(std::io::Error::other("the TLS stream has no state")),
            }
        }
    }

    pub fn try_clone_connection(&self) -> std::io::Result<Connection> {
        self.connection()?.try_clone()
    }

    pub fn read_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        self.connection()?.read_timeout()
    }

    pub fn write_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        self.connection()?.write_timeout()
    }

    pub fn set_write_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        self.connection_mut()?.set_write_timeout(dur)
    }

    pub fn set_read_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        self.connection_mut()?.set_read_timeout(dur)
    }
}

impl Read for OpenSslStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.state.as_mut() {
            Some(OpenSslState::Ready(stream)) => stream.read(buf),
            Some(OpenSslState::Handshaking(_)) => Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "the TLS handshake is still in progress",
            )),
            Some(OpenSslState::Failed(message)) => Err(std::io::Error::other(message.clone())),
            None => Err(std::io::Error::other("the TLS stream has no state")),
        }
    }
}

impl Write for OpenSslStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.state.as_mut() {
            Some(OpenSslState::Ready(stream)) => stream.write(buf),
            Some(OpenSslState::Handshaking(_)) => Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "the TLS handshake is still in progress",
            )),
            Some(OpenSslState::Failed(message)) => Err(std::io::Error::other(message.clone())),
            None => Err(std::io::Error::other("the TLS stream has no state")),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self.state.as_mut() {
            Some(OpenSslState::Ready(stream)) => stream.flush(),
            Some(OpenSslState::Handshaking(_)) => Ok(()),
            Some(OpenSslState::Failed(message)) => Err(std::io::Error::other(message.clone())),
            None => Err(std::io::Error::other("the TLS stream has no state")),
        }
    }
}

/// An OpenSSL stream which has been split into two mutually exclusive streams (e.g. for read / write)
pub struct SplitOpenSslStream(Arc<Mutex<OpenSslStream>>);

/// Expose the fd of the TCP socket the TLS session wraps so the connection can
/// be registered with the reactor (Decision 12 §12). Readiness lives on the
/// socket, not the TLS layer. Unix-only (native-socket).
#[cfg(unix)]
impl std::os::unix::io::AsRawFd for SplitOpenSslStream {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        // Reading the fd only touches the socket handle; recover from a poisoned
        // lock rather than panic (the fd is still valid).
        let guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        guard.fd
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsFd for SplitOpenSslStream {
    fn as_fd(&self) -> std::os::unix::io::BorrowedFd<'_> {
        // SAFETY: the fd is owned by the wrapped socket and outlives the borrow.
        unsafe {
            std::os::unix::io::BorrowedFd::borrow_raw(
                <Self as std::os::unix::io::AsRawFd>::as_raw_fd(self),
            )
        }
    }
}

impl core::fmt::Debug for SplitOpenSslStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SplitOpenSslStream(Arc<Mutex<OpenSslStream>>)")
            .finish()
    }
}

impl SplitOpenSslStream {
    fn lock(&self) -> std::io::Result<std::sync::MutexGuard<'_, OpenSslStream>> {
        self.0
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {e}")))
    }

    pub fn read_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        self.lock()?.read_timeout()
    }

    pub fn write_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        self.lock()?.write_timeout()
    }

    pub fn set_write_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        self.lock()?.set_write_timeout(dur)
    }

    pub fn set_read_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        self.lock()?.set_read_timeout(dur)
    }

    /// Drive the OpenSSL handshake until it finishes or the socket would block.
    ///
    /// WHY: a valtron task parks on [`Self::readiness`] when this returns
    /// `WouldBlock` (issue #277). The stream must be non-blocking, or this call
    /// waits in the kernel instead.
    ///
    /// # Errors
    /// `WouldBlock` when the socket must turn, or the handshake error.
    pub fn drive_handshake(&self) -> std::io::Result<()> {
        self.lock()?.drive_handshake()
    }

    /// Put the wrapped socket into (or out of) non-blocking mode.
    ///
    /// # Errors
    /// The socket's error, or an error if the stream lock is poisoned.
    pub fn set_nonblocking(&self, nonblocking: bool) -> std::io::Result<()> {
        self.lock()?.connection_mut()?.set_nonblocking(nonblocking)
    }

    /// Whether the wrapped socket is in non-blocking mode.
    ///
    /// # Errors
    /// The socket's error, or an error if the stream lock is poisoned.
    pub fn is_nonblocking(&self) -> std::io::Result<bool> {
        self.lock()?.connection()?.is_nonblocking()
    }

    /// Whether the wrapped socket can be driven non-blocking with readiness
    /// parking. OpenSSL always can; the socket underneath decides.
    #[must_use]
    pub fn supports_nonblocking_io(&self) -> bool {
        let Ok(guard) = self.0.lock() else {
            return false;
        };
        guard
            .connection()
            .is_ok_and(Connection::supports_nonblocking_io)
    }

    /// The readiness a task stalled on this TLS stream must wait for.
    ///
    /// A pending handshake says which direction OpenSSL asked for. Once the
    /// handshake is done, the caller's `interest` stands.
    #[cfg(unix)]
    #[must_use]
    pub fn readiness(
        &self,
        interest: crate::native::readiness::IoInterest,
    ) -> Option<foundation_core::valtron::EventReadinessPtr> {
        let guard = self.0.lock().ok()?;
        if guard.is_failed() {
            return None;
        }
        let effective = if guard.is_handshaking() {
            guard.want
        } else {
            interest
        };
        Some(std::sync::Arc::new(
            crate::native::readiness::SocketReadiness::new(guard.fd, effective),
        ))
    }

    /// A readiness handle for a synchronous I/O driver (`conn_io`) that blocks
    /// its thread. Feature 06.
    #[cfg(unix)]
    #[must_use]
    pub fn conn_readiness(&self) -> ConnReadiness {
        use std::os::fd::AsRawFd;
        ConnReadiness::Fd(self.as_raw_fd(), Some(std::sync::Arc::new(self.clone())))
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
impl crate::native::connection::TlsInterest for SplitOpenSslStream {
    fn corrected(
        &self,
        interest: crate::native::readiness::IoInterest,
    ) -> crate::native::readiness::IoInterest {
        let guard = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if guard.is_handshaking() {
            guard.want
        } else {
            interest
        }
    }

    fn fd(&self) -> std::os::unix::io::RawFd {
        use std::os::fd::AsRawFd;
        self.as_raw_fd()
    }
}

// These struct methods form the implict contract for swappable TLS implementations
impl SplitOpenSslStream {
    pub fn local_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        self.lock()?.connection_mut()?.local_addr()
    }

    pub fn peer_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        self.lock()?.connection_mut()?.peer_addr()
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
        self.lock()?.connection_mut()?.shutdown(how)
    }
}

impl Clone for SplitOpenSslStream {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Read for SplitOpenSslStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.lock()?.read(buf)
    }
}

impl Write for SplitOpenSslStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.lock()?.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.lock()?.flush()
    }
}

impl foundation_core::io::ioutils::ReadTimeoutOperations for SplitOpenSslStream {
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

pub struct OpenSslAcceptor(openssl::ssl::SslContext);

impl OpenSslAcceptor {
    pub fn from_pem(
        certificates: Vec<u8>,
        private_key: Zeroizing<Vec<u8>>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        use openssl::pkey::PKey;
        use openssl::ssl::{self, SslVerifyMode};
        use openssl::x509::X509;

        let mut ctx = openssl::ssl::SslContext::builder(ssl::SslMethod::tls())?;
        ctx.set_cipher_list("DEFAULT")?;

        let certificate_chain = X509::stack_from_pem(&certificates)?;
        if certificate_chain.is_empty() {
            return Err("Couldn't extract certificate chain from config.".into());
        }
        // The leaf certificate must always be first in the PEM file
        ctx.set_certificate(&certificate_chain[0])?;
        for chain_cert in certificate_chain.into_iter().skip(1) {
            ctx.add_extra_chain_cert(chain_cert)?;
        }
        let key = PKey::private_key_from_pem(&private_key)?;
        ctx.set_private_key(&key)?;
        ctx.set_verify(SslVerifyMode::NONE);
        ctx.check_private_key()?;

        Ok(Self(ctx.build()))
    }

    pub fn accept(
        &self,
        stream: Connection,
    ) -> Result<OpenSslStream, Box<dyn Error + Send + Sync + 'static>> {
        use openssl::ssl::Ssl;
        let session =
            Ssl::new(&self.0).map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync + 'static>)?;
        let stream = session.accept(stream)?;
        Ok(OpenSslStream::ready(stream))
    }
}

#[must_use]
pub fn default_ssl_connector(
) -> Result<Arc<openssl::ssl::SslConnector>, Box<dyn Error + Send + Sync + 'static>> {
    let connector = openssl::ssl::SslConnector::builder(openssl::ssl::SslMethod::tls())?.build();
    Ok(Arc::new(connector))
}

#[derive(Clone)]
pub struct OpenSslConnector(Arc<openssl::ssl::SslConnector>);

impl OpenSslConnector {
    pub fn new() -> Self {
        Self(default_ssl_connector().expect("should get ssl connector successfully"))
    }

    pub fn create(endpoint: &Endpoint<Arc<openssl::ssl::SslConnector>>) -> Self {
        match &endpoint {
            Endpoint::WithIdentity(_, identity) => {
                Self(identity.clone())
            }
            _ => unreachable!("You generally won't call this method with Endpoint::NoIdentity since its left to you to generate")
        }
    }

    pub fn client_tls_from_endpoint(
        endpoint: &Endpoint<Arc<openssl::ssl::SslConnector>>,
        ctx: &Ctx,
    ) -> Result<(SplitOpenSslStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let connector = Self::create(endpoint);
        connector.from_endpoint(endpoint, ctx)
    }

    /// Wrap a plain connection and start the TLS handshake.
    ///
    /// WHY: OpenSSL has no separate "start" call; the connector's `connect`
    /// both builds the session and runs the handshake until it needs readiness.
    /// On a non-blocking socket it returns `WouldBlock`, and the stream keeps
    /// the pending `MidHandshakeSslStream` for [`SplitOpenSslStream::drive_handshake`]
    /// (issue #277).
    ///
    /// The plain connection must be non-blocking for the call not to wait in
    /// the kernel. [`Self::from_tcp_stream_complete`] drives the handshake to
    /// completion for a synchronous caller.
    pub fn from_tcp_stream(
        &self,
        sni: String,
        plain: Connection,
    ) -> Result<(SplitOpenSslStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        use openssl::error::ErrorCode;
        use openssl::ssl::HandshakeError;

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

        let ssl = self
            .0
            .configure()
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?
            .into_ssl(sni.as_str())
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        let (state, want) = match ssl.connect(plain) {
            Ok(stream) => (OpenSslState::Ready(stream), DEFAULT_WANT),
            Err(HandshakeError::WouldBlock(mid)) => {
                let want = if mid.error().code() == ErrorCode::WANT_WRITE {
                    crate::native::readiness::IoInterest::Writable
                } else {
                    crate::native::readiness::IoInterest::Readable
                };
                (OpenSslState::Handshaking(mid), want)
            }
            Err(HandshakeError::Failure(mid)) => return Err(Box::new(mid.into_error())),
            Err(HandshakeError::SetupFailure(err)) => return Err(Box::new(err)),
        };

        let stream = OpenSslStream {
            state: Some(state),
            want,
            #[cfg(unix)]
            fd,
        };
        let split_stream = SplitOpenSslStream(Arc::new(Mutex::new(stream)));

        Ok((split_stream, addr))
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
        ctx: &Ctx,
    ) -> Result<(SplitOpenSslStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let (stream, addr) = self.from_tcp_stream(sni, plain)?;
        finish_handshake(&stream, timeout, ctx)
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
        endpoint: &Endpoint<Arc<openssl::ssl::SslConnector>>,
        ctx: &Ctx,
    ) -> Result<(SplitOpenSslStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let host = endpoint.host();
        let host_socket_addr: core::net::SocketAddr = host.parse()?;
        let timeout = super::endpoint_timeout(endpoint);

        let plain_stream = super::connect_plain(host_socket_addr, timeout, ctx)?;
        self.from_tcp_stream_complete(host, Connection::Tcp(plain_stream), timeout, ctx)
    }
}

/// The direction a handshake may ask for before OpenSSL reports one.
const DEFAULT_WANT: crate::native::readiness::IoInterest =
    crate::native::readiness::IoInterest::Readable;

/// Drive the handshake to completion on readiness, blocking the calling thread.
#[cfg(unix)]
fn finish_handshake(
    stream: &SplitOpenSslStream,
    timeout: Option<std::time::Duration>,
    ctx: &Ctx,
) -> std::io::Result<()> {
    crate::native::readiness::complete_handshake_blocking(
        || stream.drive_handshake(),
        || stream.handshake_poll_target(),
        timeout,
        ctx,
    )
}

/// Drive the handshake on a blocking socket (non-unix platforms have no
/// `poll(2)` readiness here).
#[cfg(not(unix))]
fn finish_handshake(
    stream: &SplitOpenSslStream,
    _timeout: Option<std::time::Duration>,
    _ctx: &Ctx,
) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.drive_handshake()
}
