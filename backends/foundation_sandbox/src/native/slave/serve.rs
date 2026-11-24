//! The listeners and the HTTP/1.1 Connect server.

use std::io;
use std::net::SocketAddr;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::Arc;

use foundation_connectrpc::{ConnectRpcServe, Router};
use foundation_context::Ctx;
use foundation_http::native::serve::Serve;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::ShutdownSignal;
use foundation_netio::native::connection::{Acceptor, Connection};

use crate::shared::limits::SandboxLimits;
use crate::shared::rpc::slave::{procedure, register_slave_service};

use super::Slave;

/// The address of the slave listener.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListenAddr {
    /// A vsock port of the guest.
    Vsock {
        /// The port.
        port: u32,
    },
    /// A Unix domain socket path.
    Unix {
        /// The path.
        path: PathBuf,
    },
    /// A Transmission Control Protocol (TCP) address.
    Tcp {
        /// The address.
        addr: SocketAddr,
    },
}

/// A bound listener, ready to serve.
pub struct Bound {
    acceptor: Arc<dyn Acceptor>,
    shutdown: Arc<ShutdownSignal>,
    /// The socket file of a Unix listener, removed after the server stops.
    socket_path: Option<PathBuf>,
}

impl Bound {
    /// The signal that stops the server.
    #[must_use]
    pub fn shutdown(&self) -> Arc<ShutdownSignal> {
        Arc::clone(&self.shutdown)
    }
}

/// Binds a listener of one kind. `shutdown` is the signal that stops the
/// server; the slave holds the same value.
///
/// # Errors
/// The operating system error of the bind, listen or socket-file call.
pub fn bind(address: &ListenAddr, shutdown: Arc<ShutdownSignal>) -> io::Result<Bound> {
    // The server parks its accept loop on the shared reactor; the signal wakes
    // it at once instead of after the poll time.
    let _ = shutdown.register_wake();
    let (acceptor, socket_path): (Arc<dyn Acceptor>, Option<PathBuf>) = match address {
        ListenAddr::Unix { path } => {
            let listener = UnixListener::bind(path)?;
            // The socket file carries the right of the caller only.
            super::sys::set_mode(path, 0o600)?;
            (Arc::new(UnixAcceptor { listener }), Some(path.clone()))
        }
        ListenAddr::Tcp { addr } => (Arc::new(std::net::TcpListener::bind(addr)?), None),
        ListenAddr::Vsock { port } => (Arc::new(VsockAcceptor::bind(*port)?), None),
    };
    Ok(Bound {
        acceptor,
        shutdown,
        socket_path,
    })
}

/// Serves the slave service until the shutdown signal turns on.
pub fn serve(bound: &Bound, ctx: Ctx, slave: Arc<Slave>) {
    let mut router = Router::new();
    register_slave_service(&mut router, slave);
    let serve: Arc<dyn Serve> = Arc::new(ConnectRpcServe::new(router.into_handler()));

    let mut app = HttpApp::<Arc<dyn Serve>>::with_ctx(ctx);
    for path in [
        procedure::INFO,
        procedure::PREPARE,
        procedure::EXEC,
        procedure::EXEC_INPUT,
        procedure::EXEC_SIGNAL,
        procedure::FILE_WRITE_BEGIN,
        procedure::FILE_WRITE_CHUNK,
        procedure::FILE_WRITE_COMMIT,
        procedure::FILE_WRITE_ABORT,
        procedure::FILE_READ,
        procedure::SHUTDOWN,
    ] {
        app.router.add_route_any(path, &serve);
    }

    let mut config = ServerConfig::defaults();
    // The HTTP/1.1 server is bound before `Prepare`, so it uses the default
    // message limit. The service rejects a body above the limit of `Prepare`.
    config.max_body_bytes = SandboxLimits::default().max_message_bytes;
    let server = HttpServer::with_config(app, "ewe-slave", config);
    server.serve_with_acceptor(&*bound.acceptor, &bound.shutdown);

    if let Some(path) = &bound.socket_path {
        let _ = std::fs::remove_file(path);
    }
}

/// The acceptor of a Unix domain socket.
struct UnixAcceptor {
    listener: UnixListener,
}

impl Acceptor for UnixAcceptor {
    fn accept_connection(&self) -> io::Result<(Connection, SocketAddr)> {
        let (stream, _address) = self.listener.accept()?;
        Ok((Connection::Unix(stream), placeholder_address()))
    }

    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.listener.set_nonblocking(nonblocking)
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "a Unix domain socket has no Internet address",
        ))
    }

    fn raw_fd(&self) -> Option<std::os::fd::RawFd> {
        Some(self.listener.as_raw_fd())
    }
}

/// The number of connections that the kernel queues for a vsock listener
/// before an accept. 128 bounds the queue and costs little memory; the kernel
/// caps it at `somaxconn`.
const VSOCK_BACKLOG: i32 = 128;

/// The acceptor of an `AF_VSOCK` port.
struct VsockAcceptor {
    socket: OwnedFd,
}

impl VsockAcceptor {
    /// Binds `VMADDR_CID_ANY` and `port`.
    ///
    /// # Errors
    /// The operating system error of the socket, bind or listen call.
    fn bind(port: u32) -> io::Result<Self> {
        Ok(Self {
            socket: super::sys::vsock_listen(port, VSOCK_BACKLOG)?,
        })
    }
}

impl Acceptor for VsockAcceptor {
    fn accept_connection(&self) -> io::Result<(Connection, SocketAddr)> {
        let stream = super::sys::accept_stream(self.socket.as_raw_fd())?;
        Ok((Connection::from_stream_fd(stream), placeholder_address()))
    }

    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        super::sys::set_nonblocking_mode(self.socket.as_raw_fd(), nonblocking)
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "a vsock socket has no Internet address",
        ))
    }

    fn raw_fd(&self) -> Option<std::os::fd::RawFd> {
        Some(self.socket.as_raw_fd())
    }
}

/// An address for a transport that has none. The server uses it for its log
/// line only.
fn placeholder_address() -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0], 0))
}
