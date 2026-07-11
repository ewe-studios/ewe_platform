//! The accept-path helper: turn an accepted `TcpStream` into the netio
//! `Connection` a given [`ServerIo`] mode asks for.
//!
//! WHY: this is the single line a server's accept loop changes to opt into the
//! completion read path (Feature 48). Seating it here keeps the server code in
//! `foundation_http` free of any direct `foundation_nativeapis` dependency — it
//! calls one iogate function and hands the resulting `Connection` to
//! `RawStream::from_connection` exactly as before.
//!
//! WHAT: [`init_reactor_for`] initialises the shared reactor on the backend a
//! mode requires (and fails loudly when it cannot), and [`accept_connection`]
//! builds the `Connection`.
//!
//! HOW: `Std` never touches the reactor and yields `Connection::Tcp`. The other
//! modes register the fd with the shared reactor and yield
//! `Connection::Completion(Box<CompletionSocket>)`, differing only in whether a
//! multishot `RECV` is armed.

use std::io;
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};

use foundation_nativeapis::native::fd::Reactor;
use foundation_nativeapis::native::poll::BackendPreference;
use foundation_nativeapis::Token;
use foundation_netio::netcap::Connection;

use crate::native::CompletionSocket;
use crate::ServerIo;

/// Per-process token counter for reactor registrations.
///
/// The completion selector reserves the top two bits of a `Token` (`POLL_TAG`),
/// so tokens must stay below `2^62`; the mask in [`next_token`] enforces that.
/// Starts at 1 — `Token(0)` is left free as a conventional sentinel.
static NEXT_TOKEN: AtomicUsize = AtomicUsize::new(1);

/// WHY: every reactor registration needs a distinct `Token`, and the completion
/// selector reserves the two high bits, so a naive counter could eventually
/// collide with the tag space.
///
/// WHAT: the next registration token, wrapped into the low 62 bits.
///
/// HOW: a relaxed fetch-add masked to `2^62 - 1`. Collision requires `2^62`
/// concurrently-live registrations, which is not reachable.
///
/// # Panics
/// Never panics.
fn next_token() -> Token {
    let n = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed) & ((1usize << 62) - 1);
    Token(n)
}

/// Initialise the shared reactor on the backend `mode` requires.
///
/// WHY: an operator asking for [`ServerIo::Completion`] must get io_uring or a
/// hard error at startup — never a silent demotion to epoll discovered later
/// from latency graphs (Feature 48, no-silent-defaults). Call this once before
/// the accept loop begins.
///
/// WHAT: `Std` is a no-op (it never touches the reactor); `Completion` demands
/// [`BackendPreference::Uring`]; `Readiness` and `Auto` accept whatever the
/// probe ladder yields.
///
/// HOW: delegates to [`Reactor::init`] / [`Reactor::get`]. The first initialiser
/// in the process wins the backend; a later conflicting demand errors with
/// `AlreadyInitialised`.
///
/// # Errors
/// The probe's failure if `Completion` is asked for on a host without io_uring,
/// or `AlreadyInitialised` if the reactor is already up on a different backend.
///
/// # Panics
/// Never panics.
pub fn init_reactor_for(mode: ServerIo) -> io::Result<()> {
    match mode {
        ServerIo::Std => Ok(()),
        ServerIo::Completion => Reactor::init(BackendPreference::Uring).map(|_| ()),
        ServerIo::Readiness | ServerIo::Auto => Reactor::get().map(|_| ()),
    }
}

/// Build the netio [`Connection`] for an accepted socket under `mode`.
///
/// WHY: the accept loop should not know about reactors, tokens, or completion
/// sockets — only which `ServerIo` mode it is serving. This is the whole seam.
///
/// WHAT: `Std` yields a plain `Connection::Tcp`; every other mode registers the
/// fd with the shared reactor and yields `Connection::Completion`. `Completion`
/// and `Auto` arm a multishot `RECV`; `Readiness` registers for readiness only.
///
/// HOW: fetches the shared reactor's registry, allocates a token, constructs the
/// [`CompletionSocket`], and boxes it into the `Connection::Completion` trait
/// object. The socket must be dropped before its inner `TcpStream` so the
/// reactor deregisters the token before the fd closes — the enum field order
/// and `RegisteredFd::drop` guarantee this.
///
/// # Errors
/// The reactor's initialisation error, or the socket's registration error.
///
/// # Panics
/// Never panics.
pub fn accept_connection(
    tcp: TcpStream,
    peer: SocketAddr,
    mode: ServerIo,
) -> io::Result<Connection> {
    if mode == ServerIo::Std {
        return Ok(Connection::Tcp(tcp));
    }

    let reactor = match mode {
        ServerIo::Completion => Reactor::init(BackendPreference::Uring)?,
        _ => Reactor::get()?,
    };
    let registry = reactor.registry();
    let token = next_token();

    let socket = match mode {
        ServerIo::Readiness => CompletionSocket::readiness(tcp, registry, token, Some(peer))?,
        _ => CompletionSocket::completion(tcp, registry, token, Some(peer))?,
    };

    tracing::debug!(
        kernel_read = socket.is_kernel_read(),
        %peer,
        mode = %mode,
        "connection accepted through iogate",
    );

    Ok(Connection::Completion(Box::new(socket)))
}

/// Dial `addr` and build the netio [`Connection`] for it under `mode` — the
/// client mirror of [`accept_connection`] (Feature 50 Part A).
///
/// WHY: a proxy's *upstream* leg is an outbound `TcpStream::connect`, never
/// registered with the reactor, so its reads are `read(2)` and there is no inbox
/// to park on. Dialing through here registers the connected socket exactly as the
/// accept path does, so both legs of a relay read from the io_uring inbox. RECV
/// on a freshly connected client socket is as valid as on an accepted one.
///
/// WHAT: `Std` yields a plain `Connection::Tcp`; every other mode registers the
/// fd with the shared reactor and yields `Connection::Completion`, arming a
/// multishot `RECV` for `Completion`/`Auto` and readiness only for `Readiness`.
///
/// HOW: `TcpStream::connect`, set non-blocking (required by `RegisteredFd`), then
/// the same registry/token/`CompletionSocket` path as [`accept_connection`]. The
/// dialed address doubles as the captured peer address.
///
/// # Errors
/// The dial's error, the reactor's initialisation error, or the socket's
/// registration error.
///
/// # Panics
/// Never panics.
pub fn connect_completion(addr: SocketAddr, mode: ServerIo) -> io::Result<Connection> {
    let tcp = TcpStream::connect(addr)?;
    // A dialed upstream is always spliced non-blocking (the reactor read paths
    // require it, and the `Std` splice loop expects `WouldBlock` too).
    tcp.set_nonblocking(true)?;

    if mode == ServerIo::Std {
        return Ok(Connection::Tcp(tcp));
    }

    let reactor = match mode {
        ServerIo::Completion => Reactor::init(BackendPreference::Uring)?,
        _ => Reactor::get()?,
    };
    let registry = reactor.registry();
    let token = next_token();

    let socket = match mode {
        ServerIo::Readiness => CompletionSocket::readiness(tcp, registry, token, Some(addr))?,
        _ => CompletionSocket::completion(tcp, registry, token, Some(addr))?,
    };

    tracing::debug!(
        kernel_read = socket.is_kernel_read(),
        %addr,
        mode = %mode,
        "upstream dialed through iogate",
    );

    Ok(Connection::Completion(Box::new(socket)))
}
