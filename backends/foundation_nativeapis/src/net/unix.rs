/// Unix domain socket types using the poll layer for readiness tracking.

use std::io;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::{self, SocketAddr};

use crate::poll::{Interest, Registry, SourceFd, Token};

/// A Unix domain stream socket.
pub struct UnixStream {
    inner: net::UnixStream,
}

impl UnixStream {
    /// Connect to a Unix socket at the given path.
    pub fn connect<P: AsRef<std::path::Path>>(path: P) -> io::Result<Self> {
        let inner = net::UnixStream::connect(path)?;
        inner.set_nonblocking(true)?;
        Ok(Self { inner })
    }

    /// Register with the poll selector.
    pub fn register(&self, registry: &Registry, token: Token, interest: Interest) -> io::Result<()> {
        registry.register(&mut SourceFd(self.inner.as_raw_fd()), token, interest)
    }

    pub fn get_ref(&self) -> &net::UnixStream { &self.inner }
}

impl AsRawFd for UnixStream { fn as_raw_fd(&self) -> RawFd { self.inner.as_raw_fd() } }
impl std::io::Read for UnixStream { fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> { self.inner.read(buf) } }
impl std::io::Write for UnixStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> { self.inner.write(buf) }
    fn flush(&mut self) -> io::Result<()> { self.inner.flush() }
}

/// A Unix domain socket server.
pub struct UnixListener {
    inner: net::UnixListener,
}

impl UnixListener {
    /// Bind to a Unix socket at the given path.
    pub fn bind<P: AsRef<std::path::Path>>(path: P) -> io::Result<Self> {
        let inner = net::UnixListener::bind(path)?;
        inner.set_nonblocking(true)?;
        Ok(Self { inner })
    }

    /// Accept a new connection.
    pub fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        let (inner, addr) = self.inner.accept()?;
        inner.set_nonblocking(true)?;
        Ok((UnixStream { inner }, addr))
    }

    /// Register with the poll selector.
    pub fn register(&self, registry: &Registry, token: Token, interest: Interest) -> io::Result<()> {
        registry.register(&mut SourceFd(self.inner.as_raw_fd()), token, interest)
    }

    pub fn get_ref(&self) -> &net::UnixListener { &self.inner }
}

impl AsRawFd for UnixListener { fn as_raw_fd(&self) -> RawFd { self.inner.as_raw_fd() } }

/// A Unix domain datagram socket.
pub struct UnixDatagram {
    inner: net::UnixDatagram,
}

impl UnixDatagram {
    /// Bind to a Unix socket at the given path.
    pub fn bind<P: AsRef<std::path::Path>>(path: P) -> io::Result<Self> {
        let inner = net::UnixDatagram::bind(path)?;
        inner.set_nonblocking(true)?;
        Ok(Self { inner })
    }

    /// Unnamed socket for sending only.
    pub fn unbound() -> io::Result<Self> {
        let inner = net::UnixDatagram::unbound()?;
        inner.set_nonblocking(true)?;
        Ok(Self { inner })
    }

    /// Receive a datagram.
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.inner.recv_from(buf)
    }

    /// Send a datagram.
    pub fn send_to<P: AsRef<std::path::Path>>(&self, buf: &[u8], path: P) -> io::Result<usize> {
        self.inner.send_to(buf, path)
    }

    /// Register with the poll selector.
    pub fn register(&self, registry: &Registry, token: Token, interest: Interest) -> io::Result<()> {
        registry.register(&mut SourceFd(self.inner.as_raw_fd()), token, interest)
    }

    pub fn get_ref(&self) -> &net::UnixDatagram { &self.inner }
}

impl AsRawFd for UnixDatagram { fn as_raw_fd(&self) -> RawFd { self.inner.as_raw_fd() } }
