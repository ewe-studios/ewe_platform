//! `CompletionSocket` — transport-facing byte source for the io_uring completion
//! read path (Decision 14 F4 / Feature 48).
//!
//! WHY: the transport-facing end of Decision 14 F4. A transport that reads through
//! here issues no `read(2)` when the kernel's completion inbox has bytes; on every
//! other backend it transparently delegates to `read(2)` — same API, just not free.
//!
//! WHAT: wraps a [`RegisteredFd<T>`] so callers get a single type that is correct
//! on every backed. The `inner` is registered with `with_completion` when the
//! io_uring backend is available; otherwise with a plain `try_new`.
//!
//! HOW: `read` uses the unified `read_bytes` path when compiled with uring (which
//! itself falls back to `read(2)` on non-completion backeds), and `get_ref().read()`
//! on builds without the uring feature. `write` always delegates.

use std::fmt;
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, RawFd};

use crate::native::fd::RegisteredFd;
use crate::native::poll::{Registry, Token};

/// A socket whose reads come from the io_uring completion inbox when available.
///
/// Constructed once per accepted connection. On an io_uring completion backend,
/// `read` pops the per-token inbox with no syscall. Everywhere else it falls
/// back to `read(2)` — this type is always correct, just not always free.
///
/// The peer address is captured at accept time rather than recovered with
/// `getpeername(2)` on every `peer_addr()` call.
pub struct CompletionSocket<T: AsRawFd + Read + Write> {
    inner: RegisteredFd<T>,
    /// The accepted peer address, captured at `accept()`.
    peer: Option<SocketAddr>,
}

impl<T: AsRawFd + Read + Write + fmt::Debug> fmt::Debug for CompletionSocket<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompletionSocket")
            .field("fd", &self.inner.as_raw_fd())
            .field("peer", &self.peer)
            .finish()
    }
}

impl<T: AsRawFd + Read + Write> CompletionSocket<T> {
    /// Register `inner` with `registry`, opting into completion mode on backends
    /// that support it.
    ///
    /// # Errors
    /// The selector's registration error.
    pub fn new(
        inner: T,
        registry: &Registry,
        token: Token,
        peer: Option<SocketAddr>,
    ) -> io::Result<Self> {
        Self::register(inner, registry, token, peer)
    }

    /// The per-backend registration. On the uring completion backend, arms
    /// multishot `RECV` so the kernel reads for the transport. Elsewhere uses
    /// a plain registration — reads stay `read(2)`.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    fn register(
        inner: T,
        registry: &Registry,
        token: Token,
        peer: Option<SocketAddr>,
    ) -> io::Result<Self> {
        let fd = RegisteredFd::with_completion(
            inner,
            registry,
            token,
            Interest::READABLE | Interest::WRITABLE,
        )?;
        Ok(Self { inner: fd, peer })
    }

    #[cfg(not(all(target_os = "linux", feature = "uring")))]
    fn register(
        inner: T,
        registry: &Registry,
        token: Token,
        _peer: Option<SocketAddr>,
    ) -> io::Result<Self> {
        let fd = RegisteredFd::try_new(inner, registry, token)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        Ok(Self { inner: fd, peer: _peer })
    }

    /// Access the inner registration. Used by guard types that need to clear
    /// latched readiness.
    pub fn inner(&self) -> &RegisteredFd<T> {
        &self.inner
    }

    /// Mutably access the inner registration.
    pub fn inner_mut(&mut self) -> &mut RegisteredFd<T> {
        &mut self.inner
    }

    /// The peer address captured at accept time.
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.peer
    }

    /// Whether `read` here costs no syscall. Observational: a transport's read
    /// loop is identical either way, because `Ok(0)` is EOF and `WouldBlock`
    /// means park.
    pub fn is_kernel_read(&self) -> bool {
        #[cfg(all(target_os = "linux", feature = "uring"))]
        {
            <RegisteredFd<T> as crate::native::fd::CompletionSource>::is_completion_source(&self.inner)
        }
        #[cfg(not(all(target_os = "linux", feature = "uring")))]
        {
            false
        }
    }
}

impl<T: AsRawFd + Read + Write> Read for CompletionSocket<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(all(target_os = "linux", feature = "uring"))]
        {
            self.inner.read_bytes(buf)
        }
        #[cfg(not(all(target_os = "linux", feature = "uring")))]
        {
            self.inner.get_mut().read(buf)
        }
    }
}

impl<T: AsRawFd + Read + Write> Write for CompletionSocket<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.get_mut().flush()
    }
}

impl<T: AsRawFd + Read + Write> AsRawFd for CompletionSocket<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl<T: AsRawFd + Read + Write + AsFd> AsFd for CompletionSocket<T> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}
