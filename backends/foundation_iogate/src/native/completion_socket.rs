//! `CompletionSocket` — transport-facing byte source for the io_uring completion
//! read path (Decision 14 F4 / Feature 48).
//!
//! WHY: the transport-facing end of Decision 14 F4. A transport that reads
//! through here issues no `read(2)` when the kernel's completion inbox has
//! bytes; on every other backend it transparently delegates to `read(2)` — same
//! API, just not free. It lives in `foundation_iogate` (not `foundation_netio`)
//! because it owns a `RegisteredFd` from `foundation_nativeapis`, and netio
//! cannot depend on nativeapis without closing a dependency cycle.
//!
//! WHAT: wraps a [`RegisteredFd<T>`] so callers get a single type that is
//! correct on every backend, plus a `CompletionReadWrite` impl so a boxed
//! `Connection::Completion` can delegate through it.
//!
//! HOW: on Linux the inner fd is registered with the shared reactor — armed for
//! multishot `RECV` in [`CompletionSocket::completion`], or plain-registered for
//! readiness only in [`CompletionSocket::readiness`] — and `read` goes through
//! the unified `read_bytes` path (which itself falls back to `read(2)` when the
//! fd is not a completion source). Off Linux there is no io_uring, so both
//! constructors register plainly and `read` is `read(2)`.

use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, RawFd};

use foundation_nativeapis::native::fd::RegisteredFd;
use foundation_nativeapis::{Interest, Registry, Token};

use foundation_netio::netcap::{CompletionReadWrite, SocketAddr as NetcapSocketAddr};

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
    /// Register `inner` with `registry`, arming completion mode (multishot
    /// `RECV`) on backends that support it.
    ///
    /// WHAT: the [`crate::ServerIo::Completion`] / [`crate::ServerIo::Auto`]
    /// construction. On io_uring the kernel starts reading the socket into the
    /// buffer ring; `read` then pops the inbox.
    ///
    /// # Errors
    /// The selector's registration error.
    ///
    /// # Panics
    /// Never panics.
    pub fn completion(
        inner: T,
        registry: &Registry,
        token: Token,
        peer: Option<SocketAddr>,
    ) -> io::Result<Self> {
        Self::register(inner, registry, token, peer, true)
    }

    /// Register `inner` with `registry` for readiness only — no completion RECV
    /// is armed, so `read` stays `read(2)` and the task parks on `Depends`.
    ///
    /// WHAT: the [`crate::ServerIo::Readiness`] construction.
    ///
    /// # Errors
    /// The selector's registration error.
    ///
    /// # Panics
    /// Never panics.
    pub fn readiness(
        inner: T,
        registry: &Registry,
        token: Token,
        peer: Option<SocketAddr>,
    ) -> io::Result<Self> {
        Self::register(inner, registry, token, peer, false)
    }

    /// The per-backend registration. On Linux with io_uring, `arm_completion`
    /// selects multishot `RECV`; otherwise a plain registration. Off Linux the
    /// flag is irrelevant — there is no completion inbox — so both register
    /// plainly and reads stay `read(2)`.
    #[cfg(target_os = "linux")]
    fn register(
        inner: T,
        registry: &Registry,
        token: Token,
        peer: Option<SocketAddr>,
        arm_completion: bool,
    ) -> io::Result<Self> {
        let inner = if arm_completion {
            RegisteredFd::with_completion(
                inner,
                registry,
                token,
                Interest::READABLE | Interest::WRITABLE,
            )?
        } else {
            RegisteredFd::new(inner, registry, token)?
        };
        Ok(Self { inner, peer })
    }

    #[cfg(not(target_os = "linux"))]
    fn register(
        inner: T,
        registry: &Registry,
        token: Token,
        peer: Option<SocketAddr>,
        _arm_completion: bool,
    ) -> io::Result<Self> {
        let _ = Interest::READABLE;
        let inner = RegisteredFd::new(inner, registry, token)?;
        Ok(Self { inner, peer })
    }

    /// Access the inner registration.
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
        #[cfg(target_os = "linux")]
        {
            foundation_nativeapis::native::fd::CompletionSource::is_completion_source(&self.inner)
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }
}

impl<T: AsRawFd + Read + Write> Read for CompletionSocket<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(target_os = "linux")]
        {
            self.inner.read_bytes(buf)
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.inner.get_mut().read(buf)
        }
    }
}

impl<T: AsRawFd + Read + Write> Write for CompletionSocket<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // On an io_uring completion socket this submits `IORING_OP_SEND` with no
        // `write(2)` (F49); on every other fd `send_bytes` degrades to `write(2)`.
        // A `WouldBlock` here means the send pool is momentarily full — the caller
        // parks and retries, exactly as it does on a non-blocking `write(2)`.
        #[cfg(target_os = "linux")]
        {
            self.inner.send_bytes(buf)
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.inner.get_mut().write(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        // Drain the in-flight SENDs: `WouldBlock` while any is unacknowledged, so
        // `flush()` only reports success once every submitted byte is on the wire.
        #[cfg(target_os = "linux")]
        {
            self.inner.drain_sends()
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.inner.get_mut().flush()
        }
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

/// The `Connection::Completion` seam: a boxed completion socket answers the few
/// obligations `Connection` needs beyond `Read + Write + AsRawFd`.
///
/// WHY: `foundation_netio::netcap::Connection::Completion` holds a
/// `Box<dyn CompletionReadWrite>` precisely so it need not name a concrete type
/// from `foundation_nativeapis` (which would re-form the dependency cycle). This
/// impl is the concrete end of that seam, and it lives here — the one crate that
/// owns both the netio trait and the nativeapis socket.
///
/// WHAT: `peer_addr` is the accept-time capture; `local_addr` is a
/// `getsockname(2)` on the owned socket; `shutdown` shuts the socket down;
/// `is_kernel_read` reports whether the reactor gave this fd a completion inbox.
///
/// HOW: delegates to the wrapped `TcpStream` for the addr/shutdown syscalls.
#[cfg(unix)]
impl CompletionReadWrite for CompletionSocket<TcpStream> {
    fn peer_addr(&self) -> Option<NetcapSocketAddr> {
        self.peer.map(NetcapSocketAddr::Tcp)
    }

    fn local_addr(&self) -> io::Result<Option<NetcapSocketAddr>> {
        self.inner
            .get_ref()
            .local_addr()
            .map(|addr| Some(NetcapSocketAddr::Tcp(addr)))
    }

    fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.get_ref().shutdown(how)
    }

    fn is_kernel_read(&self) -> bool {
        CompletionSocket::is_kernel_read(self)
    }
}
