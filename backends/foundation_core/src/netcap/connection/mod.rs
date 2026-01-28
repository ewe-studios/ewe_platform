//! Taken from the tiny-http project <https://github.com/tiny-http/tiny-http>/
//! Abstractions of Tcp and Unix socket types

use crate::io::ioutils::{PeekError, PeekableReadStream};
#[cfg(unix)]
use std::os::unix::net as unix_net;
use std::{
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs},
    ops::{Deref, DerefMut},
    path::PathBuf,
    time::Duration,
};

use derive_more::derive::From;

use super::{DataStreamError, DataStreamResult};

#[derive(From, Debug, Clone)]
pub enum SocketAddr {
    Tcp(core::net::SocketAddr),

    #[cfg(unix)]
    Unix(std::os::unix::net::SocketAddr),
}

#[derive(From, Debug)]
pub enum EndpointError {
    ParseUrlFailed(url::ParseError),
}

impl std::error::Error for EndpointError {}

impl core::fmt::Display for EndpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, Debug)]
pub enum EndpointConfig {
    NoTimeout(url::Url),
    WithTimeout(url::Url, Duration),
}

#[allow(unused)]
impl EndpointConfig {
    /// Returns a copy of the url of the target endpoint.
    #[inline]
    #[must_use]
    pub fn url(&self) -> url::Url {
        match self {
            Self::NoTimeout(inner) => inner.clone(),
            Self::WithTimeout(inner, _) => inner.clone(),
        }
    }
}

/// Endpoint represents a target endpoint to be connected
/// to communication.
#[derive(Clone, Debug)]
pub enum Endpoint<I: Clone> {
    WithDefault(EndpointConfig),
    WithIdentity(EndpointConfig, I),
}

#[allow(unused)]
impl Endpoint<()> {
    #[inline]
    #[must_use]
    pub fn with_default(target: url::Url) -> Self {
        Endpoint::WithDefault(EndpointConfig::NoTimeout(target))
    }

    #[inline]
    #[must_use]
    pub fn with_timeout(target: url::Url, timeout: Duration) -> Self {
        Endpoint::WithDefault(EndpointConfig::WithTimeout(target, timeout))
    }

    #[inline]
    pub fn with_string<S: Into<String>>(target: S) -> std::result::Result<Self, EndpointError> {
        match url::Url::parse(&target.into()) {
            Ok(url) => Ok(Endpoint::WithDefault(EndpointConfig::NoTimeout(url))),
            Err(err) => Err(EndpointError::ParseUrlFailed(err)),
        }
    }

    #[inline]
    pub fn with_string_timeout<S: Into<String>>(
        target: S,
        timeout: Duration,
    ) -> std::result::Result<Self, EndpointError> {
        match url::Url::parse(&target.into()) {
            Ok(url) => Ok(Endpoint::WithDefault(EndpointConfig::WithTimeout(
                url, timeout,
            ))),
            Err(err) => Err(EndpointError::ParseUrlFailed(err)),
        }
    }
}

#[allow(unused)]
impl<T: Clone> Endpoint<T> {
    #[inline]
    pub fn with_identity(target: url::Url, identity: T) -> Self {
        Endpoint::WithIdentity(EndpointConfig::NoTimeout(target), identity)
    }

    #[inline]
    pub fn with_identity_timeout(target: url::Url, timeout: Duration, identity: T) -> Self {
        Endpoint::WithIdentity(EndpointConfig::WithTimeout(target, timeout), identity)
    }
}

// --- Custom methods / Helper methods

#[allow(unused)]
impl<T: Clone> Endpoint<T> {
    /// Returns a copy of the url of the target endpoint.
    #[inline]
    pub fn url(&self) -> url::Url {
        match self {
            Self::WithDefault(inner) => inner.url(),
            Self::WithIdentity(inner, _) => inner.url(),
        }
    }

    #[inline]
    pub fn host(&self) -> String {
        self.get_host_from(&self.url())
    }

    #[inline]
    pub(crate) fn get_host_from(&self, endpoint_url: &url::Url) -> String {
        let mut host = match endpoint_url.host_str() {
            Some(h) => String::from(h),
            None => String::from("localhost"),
        };

        if let Some(port) = endpoint_url.port_or_known_default() {
            host = format!("{host}:{port}");
        }

        host
    }

    #[inline]
    pub fn scheme(&self) -> String {
        let url = self.url();
        url.scheme().to_owned()
    }

    #[inline]
    pub fn query(&self) -> Option<String> {
        self.get_query_params(&self.url())
    }

    #[inline]
    pub(crate) fn get_query_params(&self, endpoint_url: &url::Url) -> Option<String> {
        endpoint_url.query().map(|q| String::from(q))
    }

    #[inline]
    pub fn path_and_query(&self) -> String {
        self.get_path_with_query_params(&self.url())
    }

    #[inline]
    pub fn path(&self) -> String {
        String::from(self.url())
    }

    #[inline]
    pub(crate) fn get_path_with_query_params(&self, endpoint_url: &url::Url) -> String {
        match endpoint_url.query() {
            Some(query) => format!("{}?{}", endpoint_url.path(), query),
            None => endpoint_url.path().to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DataStreamAddr(SocketAddr, Option<SocketAddr>);

// --- Constructors

impl DataStreamAddr {
    #[must_use]
    pub fn new(local_addr: SocketAddr, remote_addr: Option<SocketAddr>) -> Self {
        Self(local_addr, remote_addr)
    }
}

// --- Methods

impl DataStreamAddr {
    #[inline]
    #[must_use]
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.1.clone()
    }

    #[inline]
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.0.clone()
    }
}

/// Unified listener. Either a [`TcpListener`] or [`std::os::unix::net::UnixListener`]
pub enum Listener {
    Tcp(TcpListener),

    #[cfg(unix)]
    Unix(unix_net::UnixListener),
}

impl Listener {
    pub fn peer_addr(&self) -> std::io::Result<Option<ListenAddr>> {
        Ok(None)
    }

    pub fn local_addr(&self) -> std::io::Result<ListenAddr> {
        match self {
            Self::Tcp(l) => l.local_addr().map(ListenAddr::from),
            #[cfg(unix)]
            Self::Unix(l) => l.local_addr().map(ListenAddr::from),
        }
    }

    pub fn accept(&self) -> std::io::Result<(Connection, Option<SocketAddr>)> {
        match self {
            Self::Tcp(l) => l
                .accept()
                .map(|(conn, addr)| (Connection::from(conn), Some(SocketAddr::from(addr)))),

            #[cfg(unix)]
            Self::Unix(l) => l
                .accept()
                .map(|(conn, addr)| (Connection::from(conn), Some(SocketAddr::from(addr)))),
        }
    }
}

impl From<TcpListener> for Listener {
    fn from(s: TcpListener) -> Self {
        Self::Tcp(s)
    }
}

#[cfg(unix)]
impl From<unix_net::UnixListener> for Listener {
    fn from(s: unix_net::UnixListener) -> Self {
        Self::Unix(s)
    }
}

/// [`Connection`] is a unified connection. Either
/// a [`TcpStream`] or [`std::os::unix::net::UnixStream`].
#[derive(Debug)]
pub enum Connection {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(unix_net::UnixStream),
}

impl Connection {
    pub fn without_timeout(
        addr: core::net::SocketAddr,
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(Self::Tcp(TcpStream::connect(addr)?))
    }

    pub fn with_timeout(
        addr: core::net::SocketAddr,
        timeout: Duration,
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(Self::Tcp(TcpStream::connect_timeout(&addr, timeout)?))
    }
}

impl Connection {
    pub fn read_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        match self {
            Self::Tcp(t) => t.read_timeout(),
            #[cfg(unix)]
            Self::Unix(u) => u.read_timeout(),
        }
    }

    pub fn write_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        match self {
            Self::Tcp(t) => t.write_timeout(),
            #[cfg(unix)]
            Self::Unix(u) => u.write_timeout(),
        }
    }

    pub fn set_write_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        match self {
            Self::Tcp(t) => t.set_write_timeout(dur),
            #[cfg(unix)]
            Self::Unix(u) => u.set_write_timeout(dur),
        }
    }

    pub fn set_read_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        match self {
            Self::Tcp(t) => t.set_read_timeout(dur),
            #[cfg(unix)]
            Self::Unix(u) => u.set_read_timeout(dur),
        }
    }
}

impl PeekableReadStream for Connection {
    fn peek(&mut self, buf: &mut [u8]) -> std::result::Result<usize, PeekError> {
        match self {
            Self::Tcp(inner) => match inner.peek(buf) {
                Ok(count) => Ok(count),
                Err(err) => Err(PeekError::IOError(err)),
            },

            #[cfg(all(unix, not(feature = "nightly")))]
            Self::Unix(_) => Err(PeekError::NotSupported),

            #[cfg(all(feature = "nightly", unix))]
            Self::Unix(inner) => match inner.peek(buf) {
                Ok(count) => Ok(count),
                Err(err) => Err(PeekError::IOError(err)),
            },
        }
    }
}

impl std::io::Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(s) => s.read(buf),
            #[cfg(unix)]
            Self::Unix(s) => s.read(buf),
        }
    }
}

impl std::io::Write for Connection {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(s) => s.write(buf),
            #[cfg(unix)]
            Self::Unix(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Tcp(s) => s.flush(),
            #[cfg(unix)]
            Self::Unix(s) => s.flush(),
        }
    }
}

impl Connection {
    /// Gets the peer's address. Some for TCP, None for Unix sockets.
    pub fn peer_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        match self {
            Self::Tcp(s) => s.peer_addr().map(SocketAddr::from).map(Some),
            #[cfg(unix)]
            Self::Unix(_) => Ok(None),
        }
    }

    /// Gets the local's address. Some for TCP, None for Unix sockets.
    pub fn local_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        match self {
            Self::Tcp(s) => s.local_addr().map(SocketAddr::from).map(Some),
            #[cfg(unix)]
            Self::Unix(u) => u.local_addr().map(SocketAddr::from).map(Some),
        }
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

    pub fn shutdown(&self, how: Shutdown) -> std::io::Result<()> {
        match self {
            Self::Tcp(s) => s.shutdown(how),
            #[cfg(unix)]
            Self::Unix(s) => s.shutdown(how),
        }
    }

    pub fn try_clone(&self) -> std::io::Result<Self> {
        match self {
            Self::Tcp(s) => s.try_clone().map(Self::from),
            #[cfg(unix)]
            Self::Unix(s) => s.try_clone().map(Self::from),
        }
    }

    /// Reads bytes until a delimiter is found, returning the bytes including the delimiter.
    pub fn take_until(&mut self, delimiter: &[u8], output: &mut Vec<u8>) -> std::io::Result<usize> {
        let mut buf = [0u8; 4096];
        let mut total_read = 0;

        loop {
            let bytes_read = self.read(&mut buf)?;
            if bytes_read == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Connection closed before delimiter found",
                ));
            }

            output.extend_from_slice(&buf[..bytes_read]);
            total_read += bytes_read;

            // Check if delimiter is in the output
            if let Some(pos) = output.windows(delimiter.len()).position(|w| w == delimiter) {
                // Remove the delimiter from output
                output.truncate(pos);
                return Ok(total_read);
            }
        }
    }

    /// Consumes the first `n` bytes from the read buffer.
    pub fn consume(&mut self, n: usize) -> std::io::Result<()> {
        // Since Connection doesn't have a buffer, we need to read and discard
        let mut buf = vec![0u8; n];
        if n > 0 {
            self.read_exact(&mut buf)?;
        }
        Ok(())
    }
}

impl From<TcpStream> for Connection {
    fn from(s: TcpStream) -> Self {
        Self::Tcp(s)
    }
}

#[cfg(unix)]
impl From<unix_net::UnixStream> for Connection {
    fn from(s: unix_net::UnixStream) -> Self {
        Self::Unix(s)
    }
}

#[derive(From, Debug, Clone)]
pub enum ConfigListenAddr {
    IP(Vec<std::net::SocketAddr>),

    // TODO: use SocketAddr when bind_addr is stabilized
    #[cfg(unix)]
    Unix(std::path::PathBuf),
}

impl ConfigListenAddr {
    pub fn from_socket_addrs<A: ToSocketAddrs>(addrs: A) -> std::io::Result<Self> {
        addrs.to_socket_addrs().map(|it| Self::IP(it.collect()))
    }

    #[cfg(unix)]
    pub fn unix_from_path<P: Into<PathBuf>>(path: P) -> Self {
        Self::Unix(path.into())
    }

    pub fn bind(&self) -> std::io::Result<Listener> {
        match self {
            Self::IP(a) => TcpListener::bind(a.as_slice()).map(Listener::from),
            #[cfg(unix)]
            Self::Unix(a) => unix_net::UnixListener::bind(a).map(Listener::from),
        }
    }
}

/// Unified listen socket address. Either a [`SocketAddr`] or [`std::os::unix::net::SocketAddr`].
#[derive(From, Debug, Clone)]
pub enum ListenAddr {
    IP(core::net::SocketAddr),

    #[cfg(unix)]
    Unix(unix_net::SocketAddr),
}

impl ListenAddr {
    #[must_use]
    pub fn to_addr(self) -> Option<SocketAddr> {
        match self {
            Self::IP(s) => Some(SocketAddr::from(s)),
            #[cfg(unix)]
            Self::Unix(s) => Some(SocketAddr::from(s)),
        }
    }

    #[must_use]
    pub fn to_ip(self) -> Option<SocketAddr> {
        match self {
            Self::IP(s) => Some(SocketAddr::from(s)),
            #[cfg(unix)]
            Self::Unix(_) => None,
        }
    }

    /// Gets the Unix socket address.
    ///
    /// This is also available on non-Unix platforms, for ease of use, but always returns `None`.
    #[cfg(unix)]
    #[must_use]
    pub fn to_unix(self) -> Option<unix_net::SocketAddr> {
        match self {
            Self::IP(_) => None,
            Self::Unix(s) => Some(s),
        }
    }
    #[cfg(not(unix))]
    pub fn to_unix(self) -> Option<SocketAddr> {
        None
    }
}

impl std::fmt::Display for ListenAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IP(s) => s.fmt(f),
            #[cfg(unix)]
            Self::Unix(s) => std::fmt::Debug::fmt(s, f),
        }
    }
}

/// A wrapper around `TcpStream`.
pub struct TcpStreamWrapper {
    inner: TcpStream,
}

impl TcpStreamWrapper {
    /// Creates a new `TcpStreamWrapper` from a `TcpStream`
    #[must_use]
    pub fn new(stream: TcpStream) -> Self {
        TcpStreamWrapper { inner: stream }
    }

    /// Consumes the wrapper and returns the inner `TcpStream`
    #[must_use]
    pub fn into_inner(self) -> TcpStream {
        self.inner
    }

    /// Sets the value of the `TCP_NODELAY` option on this socket.
    ///
    /// If set, this disables Nagle's algorithm, meaning segments are sent as soon as possible.
    pub fn set_nodelay(&self, nodelay: bool) -> std::io::Result<()> {
        self.inner.set_nodelay(nodelay)
    }

    /// Gets the value of the `TCP_NODELAY` option for this socket.
    pub fn nodelay(&self) -> std::io::Result<bool> {
        self.inner.nodelay()
    }

    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This specifies the time-to-live for IP packets.
    pub fn set_ttl(&self, ttl: u32) -> std::io::Result<()> {
        self.inner.set_ttl(ttl)
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    pub fn ttl(&self) -> std::io::Result<u32> {
        self.inner.ttl()
    }

    /// Sets the read timeout to the timeout specified.
    ///
    /// If the value specified is None, then read operations will not timeout.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.inner.set_read_timeout(timeout)
    }

    /// Sets the write timeout to the timeout specified.
    ///
    /// If the value specified is None, then write operations will not timeout.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.inner.set_write_timeout(timeout)
    }

    /// Returns the read timeout of this socket.
    pub fn read_timeout(&self) -> std::io::Result<Option<Duration>> {
        self.inner.read_timeout()
    }

    /// Returns the write timeout of this socket.
    pub fn write_timeout(&self) -> std::io::Result<Option<Duration>> {
        self.inner.write_timeout()
    }

    /// Receives data on the socket without removing it from the input queue.
    ///
    /// On success, returns the number of bytes peeked.
    pub fn peek(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.peek(buf)
    }

    /// Gets the socket error and clears it.
    ///
    /// Returns None if no error is pending.
    pub fn take_error(&self) -> std::io::Result<Option<std::io::Error>> {
        self.inner.take_error()
    }

    /// Shuts down the read, write, or both halves of this connection.
    pub fn shutdown(&self, how: Shutdown) -> std::io::Result<()> {
        self.inner.shutdown(how)
    }

    /// Creates a new TCP stream and issues a non-blocking connect to the specified address.
    pub fn connect<A: std::net::ToSocketAddrs>(addr: A) -> std::io::Result<Self> {
        TcpStream::connect(addr).map(Self::new)
    }

    /// Creates a new TCP stream and issues a connect with a timeout to the specified address.
    pub fn connect_timeout(
        addr: &core::net::SocketAddr,
        timeout: Duration,
    ) -> std::io::Result<Self> {
        TcpStream::connect_timeout(addr, timeout).map(Self::new)
    }

    /// Sets the linger duration for this TCP stream.
    ///
    /// When linger is set, the stream will wait for the specified duration
    /// for data to be sent when closing. If duration is None, the socket
    /// will close immediately.
    #[cfg(feature = "nightly")]
    #[cfg_attr(docsrs, doc(cfg(feature = "nightly")))]
    pub fn set_linger(&self, linger: Option<Duration>) -> std::io::Result<()> {
        self.inner.set_linger(linger)
    }

    /// Gets the linger duration for this TCP stream.
    #[cfg(feature = "nightly")]
    #[cfg_attr(docsrs, doc(cfg(feature = "nightly")))]
    pub fn linger(&self) -> std::io::Result<Option<Duration>> {
        self.inner.linger()
    }

    /// Gets the local socket address of this stream.
    #[cfg(feature = "nightly")]
    #[cfg_attr(docsrs, doc(cfg(feature = "nightly")))]
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Gets the remote socket address of this stream.
    #[cfg(feature = "nightly")]
    #[cfg_attr(docsrs, doc(cfg(feature = "nightly")))]
    pub fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    /// Sets the value for the SO_REUSEADDR socket option.
    #[cfg(feature = "nightly")]
    #[cfg_attr(docsrs, doc(cfg(feature = "nightly")))]
    pub fn set_reuse_address(&self, reuse: bool) -> std::io::Result<()> {
        self.inner.set_reuse_address(reuse)
    }

    /// Gets the value of the SO_REUSEADDR socket option.
    #[cfg(feature = "nightly")]
    #[cfg_attr(docsrs, doc(cfg(feature = "nightly")))]
    pub fn reuse_address(&self) -> std::io::Result<bool> {
        self.inner.reuse_address()
    }
}

// Implement Deref to allow accessing other TcpStream methods
impl Deref for TcpStreamWrapper {
    type Target = TcpStream;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// Implement DerefMut to allow accessing mutable TcpStream methods
impl DerefMut for TcpStreamWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// Forward Read implementation
impl Read for TcpStreamWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

// Forward Write implementation
impl Write for TcpStreamWrapper {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

// Example usage and tests
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::time::Duration;

    #[test]
    fn test_wrapper_standard() {
        // Set up a simple server
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        // Test connect
        let mut wrapper = TcpStreamWrapper::connect(addr).unwrap();

        // Test writing
        wrapper.write_all(b"test").unwrap();
        wrapper.flush().unwrap();

        // Test nodelay
        wrapper.set_nodelay(true).unwrap();
        assert!(wrapper.nodelay().unwrap());

        // Test ttl
        wrapper.set_ttl(100).unwrap();
        assert_eq!(wrapper.ttl().unwrap(), 100);

        // Test timeouts
        let timeout = Some(Duration::from_secs(1));
        wrapper.set_read_timeout(timeout).unwrap();
        wrapper.set_write_timeout(timeout).unwrap();
        assert_eq!(wrapper.read_timeout().unwrap(), timeout);
        assert_eq!(wrapper.write_timeout().unwrap(), timeout);

        // Test peek
        let mut buf = [0u8; 128];
        let result = wrapper.peek(&mut buf);
        assert!(result.is_ok() || result.unwrap_err().kind() == std::io::ErrorKind::WouldBlock);

        // Test take_error
        assert!(wrapper.take_error().unwrap().is_none());

        // Test shutdown
        wrapper.shutdown(Shutdown::Both).unwrap();
    }

    #[test]
    fn test_connect_timeout() {
        // Test connect_timeout with an unreachable address
        let addr = "127.0.0.1:54321".parse().expect("generate addrs"); // Assuming no server is running here
        let result = TcpStreamWrapper::connect_timeout(&addr, Duration::from_millis(100));
        assert!(result.is_err());
    }

    #[cfg(feature = "nightly")]
    #[test]
    fn test_wrapper_nightly() {
        // Set up a simple server
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        // Connect to server
        let mut wrapper = TcpStreamWrapper::connect(addr).unwrap();

        // Test linger
        wrapper.set_linger(Some(Duration::from_secs(1))).unwrap();
        assert_eq!(wrapper.linger().unwrap(), Some(Duration::from_secs(1)));

        // Test addresses
        let _local_addr = wrapper.local_addr().expect("as local address");
        let _peer_addr = wrapper.peer_addr().expect("expect peer addr");

        // Test reuse address
        wrapper.set_reuse_address(true).unwrap();
        assert!(wrapper.reuse_address().unwrap());
    }
}
