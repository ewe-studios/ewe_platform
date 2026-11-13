/// UDP networking using the poll layer for readiness tracking.
use std::io;
use std::net::{self, SocketAddr, ToSocketAddrs};
use std::os::unix::io::{AsRawFd, RawFd};

use crate::native::poll::{Interest, Registry, SourceFd, Token};

/// A UDP socket.
///
/// Wraps `std::net::UdpSocket` and integrates with our poll layer
/// for readiness tracking.
pub struct UdpSocket {
    inner: net::UdpSocket,
}

impl UdpSocket {
    /// Create a new `UdpSocket` bound to the given address.
    ///
    /// # Errors
    /// Returns an I/O error if the operation fails.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let inner = net::UdpSocket::bind(addr)?;
        inner.set_nonblocking(true)?;
        Ok(Self { inner })
    }

    /// Receive a single datagram from the socket.
    ///
    /// Returns `WouldBlock` if no data is available.
    ///
    /// # Errors
    /// Returns an I/O error if the operation fails.
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.inner.recv_from(buf)
    }

    /// Send data on the socket to the given address.
    ///
    /// # Errors
    /// Returns an I/O error if the operation fails.
    pub fn send_to<A: ToSocketAddrs>(&self, buf: &[u8], addr: A) -> io::Result<usize> {
        self.inner.send_to(buf, addr)
    }

    /// Connect this UDP socket to a remote address.
    ///
    /// # Errors
    /// Returns an I/O error if the operation fails.
    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()> {
        self.inner.connect(addr)
    }

    /// Receive a single datagram from the connected address.
    ///
    /// # Errors
    /// Returns an I/O error if the operation fails.
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.recv(buf)
    }

    /// Send data to the connected address.
    ///
    /// # Errors
    /// Returns an I/O error if the operation fails.
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.inner.send(buf)
    }

    /// Register this socket with the poll selector.
    ///
    /// # Errors
    /// Returns an I/O error if the operation fails.
    pub fn register(
        &self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        registry.register(&mut SourceFd(self.inner.as_raw_fd()), token, interest)
    }

    /// Get a reference to the inner `UdpSocket`.
    #[must_use]
    pub fn get_ref(&self) -> &net::UdpSocket {
        &self.inner
    }
}

impl AsRawFd for UdpSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}
