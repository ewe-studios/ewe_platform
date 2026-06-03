/// TCP networking types using the poll layer for readiness tracking.

use std::io;
use std::net::{self, SocketAddr, ToSocketAddrs};
use std::os::unix::io::{AsRawFd, RawFd};

use crate::native::poll::{Interest, Registry, SourceFd, Token};

/// A TCP stream connected to a remote address.
///
/// Wraps `std::net::TcpStream` and integrates with our poll layer
/// for readiness tracking.
pub struct TcpStream {
    inner: net::TcpStream,
}

impl TcpStream {
    /// Connect to a remote address.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let inner = net::TcpStream::connect(addr)?;
        inner.set_nonblocking(true)?;
        Ok(Self { inner })
    }

    /// Register this stream with the poll selector.
    pub fn register(
        &self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        registry.register(&mut SourceFd(self.inner.as_raw_fd()), token, interest)
    }

    /// Get a reference to the inner TcpStream.
    pub fn get_ref(&self) -> &net::TcpStream {
        &self.inner
    }
}

impl AsRawFd for TcpStream {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl std::io::Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl std::io::Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// A TCP socket server, listening for connections.
///
/// Wraps `std::net::TcpListener` and integrates with our poll layer
/// for readiness tracking.
pub struct TcpListener {
    inner: net::TcpListener,
}

impl TcpListener {
    /// Create a new TcpListener bound to the given address.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let inner = net::TcpListener::bind(addr)?;
        inner.set_nonblocking(true)?;
        Ok(Self { inner })
    }

    /// Accept a new incoming connection.
    ///
    /// Returns `WouldBlock` if no connections are pending.
    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let (inner, addr) = self.inner.accept()?;
        inner.set_nonblocking(true)?;
        Ok((TcpStream { inner }, addr))
    }

    /// Register this listener with the poll selector.
    pub fn register(
        &self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        registry.register(&mut SourceFd(self.inner.as_raw_fd()), token, interest)
    }

    /// Get a reference to the inner TcpListener.
    pub fn get_ref(&self) -> &net::TcpListener {
        &self.inner
    }
}

impl AsRawFd for TcpListener {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}
