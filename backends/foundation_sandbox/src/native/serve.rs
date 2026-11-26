//! The `SandboxService` server on a Unix domain socket or on Transmission
//! Control Protocol (TCP) (feature 06).
//!
//! A Unix domain socket is guarded by its file mode (`0600`), so it needs no
//! token. A TCP listener needs one, and the server refuses to start without it
//! (invariant I3). The check runs in an interceptor before the handler, so a
//! call with no token or a wrong token never reaches the control plane.

use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use foundation_connectrpc::shared::interceptor::{
    Interceptor, StreamCall, StreamingClientFunc, StreamingHandlerFunc, UnaryCall, UnaryFunc,
};
use foundation_connectrpc::{ConnectRpcServe, Router};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use foundation_http::native::serve::Serve;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::ShutdownSignal;
use foundation_netio::native::connection::{Acceptor, Connection};
use foundation_netio::shared::http::SimpleHeader;

use crate::shared::control::service::{authorize, ControlService};
use crate::shared::control::SandboxProvider;
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::ids::MAX_SOCKET_PATH_BYTES;
use crate::shared::limits::SandboxLimits;
use crate::shared::listen::ListenAddress;
use crate::shared::rpc::sandbox::{procedure, register_sandbox_service};
use crate::shared::Secret;

/// The configuration of a served listener.
#[derive(Clone, Debug)]
pub struct ServeConfig {
    /// The limits of the control plane that the server wraps.
    pub limits: SandboxLimits,
    /// The longest Unix domain socket path in bytes, without the end zero byte.
    ///
    /// Default 103: the kernel limit is 104 bytes on macOS and 108 on Linux, and
    /// the end zero byte takes one.
    pub max_socket_path: usize,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            limits: SandboxLimits::default(),
            max_socket_path: MAX_SOCKET_PATH_BYTES,
        }
    }
}

impl ServeConfig {
    /// Replaces [`Self::limits`].
    #[must_use]
    pub fn with_limits(mut self, limits: SandboxLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Replaces [`Self::max_socket_path`].
    #[must_use]
    pub fn with_max_socket_path(mut self, bytes: usize) -> Self {
        self.max_socket_path = bytes;
        self
    }
}

/// Why a listener did not start.
#[derive(Debug)]
pub enum ServeError {
    /// The Unix domain socket path is too long for the kernel.
    SocketPathTooLong {
        /// The path.
        path: String,
        /// The limit in bytes.
        max: usize,
    },
    /// Another server listens on this Unix domain socket.
    AddressInUse,
    /// This target has no listener of the kind.
    Unsupported(&'static str),
    /// The bind or socket-file call failed.
    Bind(io::Error),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SocketPathTooLong { path, max } => {
                write!(
                    formatter,
                    "the socket path {path} is longer than {max} bytes"
                )
            }
            Self::AddressInUse => formatter.write_str("address in use"),
            Self::Unsupported(what) => write!(formatter, "{what} is not supported on this host"),
            Self::Bind(error) => write!(formatter, "the bind failed: {error}"),
        }
    }
}

impl std::error::Error for ServeError {}

/// A bound listener, ready to serve.
pub struct Bound {
    acceptor: Arc<dyn Acceptor>,
    shutdown: Arc<ShutdownSignal>,
    socket_path: Option<PathBuf>,
    local: Option<SocketAddr>,
}

impl Bound {
    /// The signal that stops the server.
    #[must_use]
    pub fn shutdown(&self) -> Arc<ShutdownSignal> {
        Arc::clone(&self.shutdown)
    }

    /// The TCP address the kernel gave the listener, or `None` for a Unix
    /// domain socket.
    #[must_use]
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local
    }
}

/// Binds a listener. `shutdown` stops the server.
///
/// # Errors
/// [`ServeError::SocketPathTooLong`], [`ServeError::AddressInUse`],
/// [`ServeError::Unsupported`] or [`ServeError::Bind`].
pub fn bind_control(
    listen: &ListenAddress,
    shutdown: Arc<ShutdownSignal>,
    config: &ServeConfig,
) -> Result<Bound, ServeError> {
    // The server parks its accept loop on the shared reactor; the signal wakes
    // it at once instead of after the poll time.
    let _ = shutdown.register_wake();
    match listen {
        ListenAddress::Unix { path } => bind_unix(path, shutdown, config),
        ListenAddress::Tcp { addr } => {
            let listener = std::net::TcpListener::bind(addr).map_err(ServeError::Bind)?;
            let local = listener.local_addr().ok();
            Ok(Bound {
                acceptor: Arc::new(listener),
                shutdown,
                socket_path: None,
                local,
            })
        }
        ListenAddress::Vsock { .. } => {
            Err(ServeError::Unsupported("a vsock control-plane listener"))
        }
    }
}

/// Binds a Unix domain socket, removes a stale file and sets mode `0600`.
#[cfg(unix)]
fn bind_unix(
    path: &std::path::Path,
    shutdown: Arc<ShutdownSignal>,
    config: &ServeConfig,
) -> Result<Bound, ServeError> {
    use std::os::unix::fs::PermissionsExt as _;
    use std::os::unix::net::{UnixListener, UnixStream};

    let length = path.as_os_str().as_encoded_bytes().len();
    if length > config.max_socket_path {
        return Err(ServeError::SocketPathTooLong {
            path: path.display().to_string(),
            max: config.max_socket_path,
        });
    }
    if path.exists() {
        match UnixStream::connect(path) {
            Ok(_) => return Err(ServeError::AddressInUse),
            // A socket file with no server behind it is stale.
            Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                let _ = std::fs::remove_file(path);
            }
            Err(_) => {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    let listener = UnixListener::bind(path).map_err(ServeError::Bind)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(ServeError::Bind)?;
    Ok(Bound {
        acceptor: Arc::new(UnixAcceptor { listener }),
        shutdown,
        socket_path: Some(path.to_path_buf()),
        local: None,
    })
}

/// A Unix domain socket has no listener on a non-Unix host.
#[cfg(not(unix))]
fn bind_unix(
    _path: &std::path::Path,
    _shutdown: Arc<ShutdownSignal>,
    _config: &ServeConfig,
) -> Result<Bound, ServeError> {
    Err(ServeError::Unsupported("a Unix domain socket"))
}

/// Serves `SandboxService` on a bound listener until `shutdown` turns on.
pub fn serve_bound(
    ctx: Ctx,
    control: Arc<dyn SandboxProvider>,
    bound: &Bound,
    auth: Option<Secret>,
    limits: &SandboxLimits,
) {
    let service = Arc::new(ControlService::new(control, limits.clone()));
    let mut router = Router::new();
    if let Some(token) = auth {
        router = router.with_global_interceptors(vec![Arc::new(BearerAuth {
            token,
            max_error_message: limits.max_error_message,
        })]);
    }
    register_sandbox_service(&mut router, service);
    let serve: Arc<dyn Serve> = Arc::new(ConnectRpcServe::new(router.into_handler()));

    let mut app = HttpApp::<Arc<dyn Serve>>::with_ctx(ctx);
    for path in [
        procedure::PROBE,
        procedure::CREATE,
        procedure::GET,
        procedure::LIST,
        procedure::STOP,
        procedure::KILL,
        procedure::EXEC,
        procedure::EXEC_INPUT,
        procedure::EXEC_SIGNAL,
        procedure::FILE_WRITE_BEGIN,
        procedure::FILE_WRITE_CHUNK,
        procedure::FILE_WRITE_COMMIT,
        procedure::FILE_WRITE_ABORT,
        procedure::FILE_READ,
    ] {
        app.router.add_route_any(path, &serve);
    }

    let mut config = ServerConfig::defaults();
    config.max_body_bytes = limits.max_message_bytes;
    let server = HttpServer::with_config(app, "ewe-sandbox-control", config);
    server.serve_with_acceptor(&*bound.acceptor, &bound.shutdown);

    if let Some(path) = &bound.socket_path {
        let _ = std::fs::remove_file(path);
    }
}

/// Binds and serves `SandboxService` with the default configuration.
///
/// # Errors
/// The errors of [`bind_control`].
pub fn serve_control(
    ctx: Ctx,
    control: Arc<dyn SandboxProvider>,
    listen: &ListenAddress,
    auth: Option<Secret>,
    shutdown: Arc<ShutdownSignal>,
) -> Result<(), ServeError> {
    serve_control_with(
        ctx,
        control,
        listen,
        auth,
        shutdown,
        &ServeConfig::default(),
    )
}

/// Binds and serves `SandboxService` with the given configuration.
///
/// # Errors
/// The errors of [`bind_control`].
pub fn serve_control_with(
    ctx: Ctx,
    control: Arc<dyn SandboxProvider>,
    listen: &ListenAddress,
    auth: Option<Secret>,
    shutdown: Arc<ShutdownSignal>,
    config: &ServeConfig,
) -> Result<(), ServeError> {
    let bound = bind_control(listen, shutdown, config)?;
    serve_bound(ctx, control, &bound, auth, &config.limits);
    Ok(())
}

/// Refuses every call that does not carry the served token.
#[derive(Clone)]
struct BearerAuth {
    token: Secret,
    max_error_message: usize,
}

impl BearerAuth {
    /// The `Authorization` header of a call.
    fn check(&self, ctx: &foundation_connectrpc::RpcCtx) -> SandboxResult<()> {
        let header = ctx
            .request
            .headers
            .get(&SimpleHeader::AUTHORIZATION)
            .and_then(|values| values.first())
            .map(String::as_str);
        authorize(header, &self.token)
    }

    /// The error that a refused call returns.
    fn error(&self) -> ErrorTrace<foundation_connectrpc::ConnectError> {
        ErrorTrace::new(SandboxError::Unauthorized.to_connect(self.max_error_message))
    }
}

impl Interceptor for BearerAuth {
    fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc {
        let this = self.clone();
        Arc::new(move |ctx: foundation_connectrpc::RpcCtx, call: UnaryCall| {
            let this = this.clone();
            let next = next.clone();
            Box::pin(async move {
                this.check(&ctx).map_err(|_| this.error())?;
                next(ctx, call).await
            })
        })
    }

    fn wrap_streaming_client(&self, next: StreamingClientFunc) -> StreamingClientFunc {
        next
    }

    fn wrap_streaming_handler(&self, next: StreamingHandlerFunc) -> StreamingHandlerFunc {
        let this = self.clone();
        Arc::new(
            move |ctx: foundation_connectrpc::RpcCtx, call: StreamCall| {
                let this = this.clone();
                let next = next.clone();
                Box::pin(async move {
                    this.check(&ctx).map_err(|_| this.error())?;
                    next(ctx, call).await
                })
            },
        )
    }
}

/// The acceptor of a Unix domain socket.
#[cfg(unix)]
struct UnixAcceptor {
    listener: std::os::unix::net::UnixListener,
}

#[cfg(unix)]
impl Acceptor for UnixAcceptor {
    fn accept_connection(&self) -> io::Result<(Connection, SocketAddr)> {
        use std::os::fd::AsRawFd as _;
        let (stream, _address) = self.listener.accept()?;
        let _ = &self.listener.as_raw_fd();
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
        use std::os::fd::AsRawFd as _;
        Some(self.listener.as_raw_fd())
    }
}

/// An address for a transport that has none.
fn placeholder_address() -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0], 0))
}
