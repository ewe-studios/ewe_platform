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
//! `Connection::Completion` can delegate through it. **Explicit read/write mode
//! flags** (F50 config gate) replace the previous auto-detection so both paths
//! — io_uring completion and standard syscalls — are visible, independently
//! selectable, and trivially testable.
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

// ── Read / Write mode enums (F50 config gate) ────────────────────────────

/// How reads are performed on this socket.
///
/// WHY: the previous auto-detection (`is_completion_source()`) worked but hid
/// the two paths from tests. An explicit flag means a test can construct a
/// `CompletionSocket` with `ReadMode::Std` on an io_uring-capable kernel and
/// verify the fallback path directly — without recompiling or switching the
/// global reactor.
///
/// WHAT: `Std` issues `read(2)`; `Completion` pops the kernel-filled RECV inbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadMode {
    /// Standard `read(2)` — no io_uring, no reactor inbox.
    Std,
    /// io_uring completion: pop from the kernel's multishot-RECV inbox.
    Completion,
}

/// How writes are performed on this socket.
///
/// WHY: io_uring SEND (F49) and SEND_ZC (F50) are independent of the RECV mode.
/// A relay may want completion reads on both legs but std writes (e.g. small
/// responses that don't benefit from SEND_ZC), or completion reads + SEND_ZC
/// writes for maximum throughput. Explicit modes make every combination
/// constructable and testable.
///
/// WHAT: three tiers — `Std` (`write(2)`), `Send` (`IORING_OP_SEND` with a pool
/// buffer copy), and `SendZc` (`IORING_OP_SEND_ZC`, two-phase completion, no
/// kernel copy of the buffer — and, with [`CompletionSocket::write_from_provided_buf`],
/// no user-memory copy either).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    /// Standard `write(2)` — no io_uring, no send pool.
    Std,
    /// io_uring `IORING_OP_SEND`: copy into a pool buffer, submit SQE.
    /// One `memcpy` per write; zero `write(2)` syscalls.
    Send,
    /// io_uring `IORING_OP_SEND_ZC`: zero-copy send with two-phase
    /// completion (F_MORE → F_NOTIF). The buffer stays pinned until the notif.
    SendZc,
}

// ── CompletionSocket ─────────────────────────────────────────────────────

/// A socket whose reads come from the io_uring completion inbox when available.
///
/// Constructed once per accepted connection. On an io_uring completion backend,
/// `read` pops the per-token inbox with no syscall. Everywhere else it falls
/// back to `read(2)` — this type is always correct, just not always free.
///
/// The peer address is captured at accept time rather than recovered with
/// `getpeername(2)` on every `peer_addr()` call.
///
/// ## Read / write mode (F50 config gate)
///
/// The `read_mode` and `write_mode` fields control which path each operation
/// takes. They are set at construction from the `ReadMode`/`WriteMode` enums
/// and can be inspected (but not changed) afterward. Tests construct sockets
/// with explicit modes to exercise both paths on the same kernel.
pub struct CompletionSocket<T: AsRawFd + Read + Write> {
    inner: RegisteredFd<T>,
    /// The accepted peer address, captured at `accept()`.
    peer: Option<SocketAddr>,
    /// How `read()` acquires bytes.
    read_mode: ReadMode,
    /// How `write()`/`flush()` push bytes out.
    write_mode: WriteMode,
}

impl<T: AsRawFd + Read + Write + fmt::Debug> fmt::Debug for CompletionSocket<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompletionSocket")
            .field("fd", &self.inner.as_raw_fd())
            .field("peer", &self.peer)
            .field("read_mode", &self.read_mode)
            .field("write_mode", &self.write_mode)
            .finish()
    }
}

impl<T: AsRawFd + Read + Write> CompletionSocket<T> {
    /// Register `inner` with `registry`, arming completion mode (multishot
    /// `RECV`) on backends that support it. Read/write modes default to
    /// `Completion` + `Send` (the F48+F49 combined path).
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
        Self::register(inner, registry, token, peer, true, ReadMode::Completion, WriteMode::Send)
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
        Self::register(inner, registry, token, peer, false, ReadMode::Std, WriteMode::Std)
    }

    /// Register with explicit read and write modes (F50 config gate).
    ///
    /// WHY: the caller — not the auto-detection — decides which paths to use.
    /// A relay can have completion reads on both legs but std writes, or vice
    /// versa. Tests can force `Std` on an io_uring kernel and verify the
    /// fallback.
    ///
    /// WHAT: `arm_completion` arms a multishot `RECV` (needed when `read_mode`
    /// is `Completion`). `read_mode`/`write_mode` are stored and branched on
    /// at every `read()`/`write()`/`flush()`.
    ///
    /// # Errors
    /// The selector's registration error.
    ///
    /// # Panics
    /// Never panics.
    pub fn with_modes(
        inner: T,
        registry: &Registry,
        token: Token,
        peer: Option<SocketAddr>,
        arm_completion: bool,
        read_mode: ReadMode,
        write_mode: WriteMode,
    ) -> io::Result<Self> {
        Self::register(inner, registry, token, peer, arm_completion, read_mode, write_mode)
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
        read_mode: ReadMode,
        write_mode: WriteMode,
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
        Ok(Self { inner, peer, read_mode, write_mode })
    }

    #[cfg(not(target_os = "linux"))]
    fn register(
        inner: T,
        registry: &Registry,
        token: Token,
        peer: Option<SocketAddr>,
        _arm_completion: bool,
        read_mode: ReadMode,
        write_mode: WriteMode,
    ) -> io::Result<Self> {
        let _ = Interest::READABLE;
        let inner = RegisteredFd::new(inner, registry, token)?;
        // Off Linux, force modes to Std since there's no io_uring.
        Ok(Self { inner, peer, read_mode: ReadMode::Std, write_mode: WriteMode::Std })
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

    /// How reads are performed.
    pub fn read_mode(&self) -> ReadMode {
        self.read_mode
    }

    /// How writes are performed.
    pub fn write_mode(&self) -> WriteMode {
        self.write_mode
    }

    /// Whether `read` here costs no syscall. Observational: a transport's read
    /// loop is identical either way, because `Ok(0)` is EOF and `WouldBlock`
    /// means park.
    pub fn is_kernel_read(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.read_mode == ReadMode::Completion
                && foundation_nativeapis::native::fd::CompletionSource::is_completion_source(&self.inner)
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    // ── ProvidedBuf-direct zero-copy handoff (F50 Part C tail) ────────────

    /// Write a [`ProvidedBuf`] from the RECV ring directly via `IORING_OP_SEND_ZC`
    /// with **no user-memory copy**.
    ///
    /// WHY: this is the true zero-copy relay — the buffer the kernel filled (via
    /// multishot `RECV` on the inbound leg) is the same buffer handed to the
    /// kernel to send (via `SEND_ZC` on the outbound leg). No `memcpy` into a
    /// `SendPool` buffer, no intermediate allocation. The `ProvidedBuf` lives
    /// until the kernel's `F_NOTIF` CQE says it is done, at which point it is
    /// dropped — returning it to the RECV ring.
    ///
    /// WHAT: takes ownership of `buf` (a kernel-filled buffer from another
    /// socket's RECV ring), submits `IORING_OP_SEND_ZC` for it on *this* socket's
    /// fd, and holds it in the in-flight map until the `F_NOTIF` CQE.
    ///
    /// HOW: the [`ProvidedBuf`]'s backing memory is in the RECV ring's mmap'd
    /// region — its address is stable and the kernel can DMA from it directly.
    /// The send-id → `ProvidedBuf` mapping in the selector survives until
    /// `drain_completions` processes the `F_NOTIF`.
    ///
    /// # Errors
    /// The kernel's error if the SQE cannot be submitted (e.g. SQ full).
    ///
    /// # Panics
    /// Panics if an internal lock is poisoned, or if `write_mode` is not
    /// `WriteMode::SendZc`.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn write_from_provided_buf(
        &mut self,
        buf: foundation_nativeapis::native::fd::completion::ProvidedBuf,
    ) -> io::Result<usize> {
        assert_eq!(
            self.write_mode,
            WriteMode::SendZc,
            "write_from_provided_buf requires WriteMode::SendZc"
        );
        self.inner.send_bytes_zc_direct(buf)
    }

    // ── split_read_write (F49 Phase C) ────────────────────────────────────

    /// Split this socket into independent read and write halves, each on its own
    /// `dup(2)`'d fd with its own reactor registration.
    ///
    /// WHY: some transports want to move the read half and write half to
    /// different tasks. A plain `dup(2)` + `write(2)` write-half already exists
    /// in `ReadWriteStream::split_read_write`; this is the completion-backed
    /// analogue — the write half is a full `CompletionSocket` with its own
    /// token, its own registration, and its own SEND/SEND_ZC path.
    ///
    /// WHAT: `dup(2)` the fd, create a [`ReadHalf`] (this socket, with writes
    /// disabled) and a [`WriteHalf`] (a new `CompletionSocket` on the dup'd fd,
    /// with reads disabled, registered for SEND only).
    ///
    /// HOW: the dup'd fd gets a fresh token and an independent registration.
    /// Dropping either half shuts down its direction and deregisters its fd
    /// from the reactor. The halves share no state — the dup makes them
    /// independent kernel objects, each with its own buffer.
    ///
    /// # Errors
    /// `dup(2)` or registration failure.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn split(mut self) -> io::Result<(ReadHalf<T>, WriteHalf<T>)> {
        use std::os::unix::io::FromRawFd;

        let read_fd = self.inner.as_raw_fd();
        // SAFETY: dup(2) of a valid, open fd — the only failure is EMFILE/ENFILE.
        let write_raw = unsafe { libc::dup(read_fd) };
        if write_raw < 0 {
            return Err(io::Error::last_os_error());
        }

        // Shut down the read direction on the write half and the write direction
        // on the read half so each half only does what it is meant to.
        let _ = self.inner.get_mut().shutdown(Shutdown::Write);

        // SAFETY: write_raw is a fresh owned fd from dup(2).
        let write_stream = unsafe { T::from_raw_fd(write_raw) };
        let _ = write_stream.shutdown(Shutdown::Read);

        // The write half needs its own reactor registration. Use the same
        // registry as the read half (the shared reactor).
        let reactor = foundation_nativeapis::native::fd::Reactor::get()?;
        let registry = reactor.registry();
        // Allocate a fresh token for the write half.
        let write_token = {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT_SPLIT_TOKEN: AtomicUsize = AtomicUsize::new(1 << 30);
            let n = NEXT_SPLIT_TOKEN.fetch_add(1, Ordering::Relaxed) & ((1usize << 62) - 1);
            Token(n)
        };

        let write_socket = CompletionSocket::<T>::with_modes(
            write_stream,
            registry,
            write_token,
            self.peer,
            false, // no RECV on the write half
            ReadMode::Std,
            self.write_mode,
        )?;

        let read_half = ReadHalf { socket: self };
        let write_half = WriteHalf { socket: write_socket };

        Ok((read_half, write_half))
    }
}

// ── Read / Write impls (explicit mode branches) ──────────────────────────

impl<T: AsRawFd + Read + Write> Read for CompletionSocket<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(target_os = "linux")]
        {
            match self.read_mode {
                ReadMode::Completion => self.inner.read_bytes(buf),
                ReadMode::Std => self.inner.get_mut().read(buf),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.inner.get_mut().read(buf)
        }
    }
}

impl<T: AsRawFd + Read + Write> Write for CompletionSocket<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        #[cfg(target_os = "linux")]
        {
            match self.write_mode {
                WriteMode::SendZc => self.inner.send_bytes_zc(buf),
                WriteMode::Send => self.inner.send_bytes(buf),
                WriteMode::Std => self.inner.get_mut().write(buf),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.inner.get_mut().write(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            match self.write_mode {
                WriteMode::SendZc | WriteMode::Send => self.inner.drain_sends(),
                WriteMode::Std => self.inner.get_mut().flush(),
            }
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

// ── Split halves ─────────────────────────────────────────────────────────

/// The read half of a split [`CompletionSocket`] (F49 Phase C).
///
/// Dropping this shuts down the read direction and deregisters the fd.
pub struct ReadHalf<T: AsRawFd + Read + Write> {
    socket: CompletionSocket<T>,
}

impl<T: AsRawFd + Read + Write> ReadHalf<T> {
    /// The peer address captured at accept time.
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.socket.peer_addr()
    }
}

impl<T: AsRawFd + Read + Write> Read for ReadHalf<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.socket.read(buf)
    }
}

impl<T: AsRawFd + Read + Write> AsRawFd for ReadHalf<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}

/// The write half of a split [`CompletionSocket`] (F49 Phase C).
///
/// Dropping this shuts down the write direction and deregisters the dup'd fd.
pub struct WriteHalf<T: AsRawFd + Read + Write> {
    socket: CompletionSocket<T>,
}

impl<T: AsRawFd + Read + Write> WriteHalf<T> {
    /// The peer address captured at accept time.
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.socket.peer_addr()
    }

    /// How writes are performed on this half.
    pub fn write_mode(&self) -> WriteMode {
        self.socket.write_mode()
    }
}

impl<T: AsRawFd + Read + Write> Write for WriteHalf<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.socket.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.socket.flush()
    }
}

impl<T: AsRawFd + Read + Write> AsRawFd for WriteHalf<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}

// ── CompletionReadWrite impl ─────────────────────────────────────────────

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
