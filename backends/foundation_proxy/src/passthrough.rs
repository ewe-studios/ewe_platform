//! Raw byte-level passthrough.
//!
//! WHY: Decision 14 §"Protocol inference" — a `tcp://` backend is proxied at the
//! byte level with no HTTP semantics (RDP, VNC, noVNC). The same primitive
//! relays a WebSocket connection once its `101` handshake is done: after the
//! upgrade everything is opaque frames. Both cases are "copy bytes both ways
//! until one side closes".
//!
//! WHAT: [`TcpPassthrough`] (a standalone raw TCP listener that splices every
//! accepted client to a fixed backend authority), [`UdpPassthrough`] (the same
//! for datagrams).
//!
//! HOW: The listeners are futures on the valtron pool. The accept loop parks on
//! the listener's read readiness, each client is its own future that runs
//! [`crate::aio::splice`], and the UDP loop parks on the socket's read readiness.
//! No thread is started per client or per listener, and none sleeps. A loop
//! also wakes on a timer every `shutdown_poll` to see the shutdown signal.

use std::net::{SocketAddr, TcpListener, UdpSocket};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use foundation_core::synca::OnSignal;
use foundation_core::valtron::{from_future, send, sleep_async, Clock};
use foundation_http::native::writer::WriterConfig;
use foundation_iogate::ServerIo;
use foundation_netio::shared::client::{DnsResolver, SystemDnsResolver};
use futures_util::future::{select, Either};

use crate::aio::{dial_tcp, splice, wrap_tcp, AsyncFd};

/// Limits and modes for [`TcpPassthrough`] and [`UdpPassthrough`].
#[derive(Debug, Clone, Copy)]
pub struct PassthroughConfig {
    /// How often an idle loop wakes to check for shutdown. A timer wake, not a
    /// thread sleep. Default 50 ms (the control socket's former poll).
    pub shutdown_poll: Duration,
    /// How long the UDP relay waits for the backend to answer one datagram.
    /// Default 5 seconds.
    pub udp_response_timeout: Duration,
    /// How the backend leg is dialed. Default `Std`.
    pub io_mode: ServerIo,
    /// The longest the backend TCP connect may take. Default 60 seconds (nginx
    /// `proxy_connect_timeout`).
    pub connect_timeout: Duration,
    /// Write limits for the spliced connections (`send_timeout` is nginx's 60 s).
    pub writer: WriterConfig,
}

impl Default for PassthroughConfig {
    fn default() -> Self {
        Self {
            shutdown_poll: Duration::from_millis(50),
            udp_response_timeout: Duration::from_secs(5),
            io_mode: ServerIo::default(),
            connect_timeout: Duration::from_secs(60),
            writer: WriterConfig::default(),
        }
    }
}

impl PassthroughConfig {
    /// The defaults documented on each field.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set [`Self::shutdown_poll`].
    #[must_use]
    pub fn with_shutdown_poll(mut self, poll: Duration) -> Self {
        self.shutdown_poll = poll;
        self
    }

    /// Set [`Self::udp_response_timeout`].
    #[must_use]
    pub fn with_udp_response_timeout(mut self, timeout: Duration) -> Self {
        self.udp_response_timeout = timeout;
        self
    }

    /// Set [`Self::io_mode`].
    #[must_use]
    pub fn with_io_mode(mut self, mode: ServerIo) -> Self {
        self.io_mode = mode;
        self
    }

    /// Set [`Self::connect_timeout`].
    #[must_use]
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set [`Self::writer`].
    #[must_use]
    pub fn with_writer(mut self, writer: WriterConfig) -> Self {
        self.writer = writer;
        self
    }
}

/// Wait for `fut`, or give up after `tick`: `None` means the tick came first.
async fn or_tick<T>(fut: impl std::future::Future<Output = T>, tick: Duration) -> Option<T> {
    match select(Box::pin(fut), Box::pin(sleep_async(tick))).await {
        Either::Left((value, _)) => Some(value),
        Either::Right(((), _)) => None,
    }
}

/// Start `task` on the valtron pool and return a channel that gets one message
/// when it ends.
fn spawn_loop(
    task: impl std::future::Future<Output = ()> + Send + 'static,
    done: Sender<()>,
) -> std::io::Result<()> {
    send(from_future(async move {
        task.await;
        let _ = done.send(());
    }))
    .map_err(|e| std::io::Error::other(format!("cannot start the passthrough loop: {e}")))
}

/// Stop a loop: set its signal and wait (bounded) until it has ended and
/// dropped its socket. A blocking wait on the caller's thread.
fn stop_loop(shutdown: &OnSignal, done: &Receiver<()>, poll: Duration) {
    shutdown.turn_on();
    let _ = done.recv_timeout(poll.saturating_mul(4) + Duration::from_secs(1));
}

/// A raw TCP passthrough listener: every accepted client is spliced to
/// `backend_authority`.
///
/// WHY: The HTTP front end parses HTTP before dispatch, so a genuinely raw
/// protocol (`tcp://` service) cannot ride it — it needs its own listener that
/// hands the bytes straight to the backend.
///
/// The valtron pool must be initialised before [`TcpPassthrough::start`].
#[derive(Debug)]
pub struct TcpPassthrough {
    local_addr: SocketAddr,
    shutdown: Arc<OnSignal>,
    done: Receiver<()>,
    poll: Duration,
}

impl TcpPassthrough {
    /// Bind `listen_addr` and splice every connection to `backend_authority`,
    /// with the default [`PassthroughConfig`].
    ///
    /// Returns immediately with a handle; the accept loop runs as a future until
    /// [`TcpPassthrough::shutdown`] (or drop).
    ///
    /// # Errors
    /// Returns the bind error if `listen_addr` cannot be bound, or an error if
    /// the loop cannot be started on the pool.
    pub fn start(listen_addr: &str, backend_authority: &str) -> std::io::Result<Self> {
        Self::start_with(listen_addr, backend_authority, PassthroughConfig::default())
    }

    /// Like [`Self::start`], with explicit limits.
    ///
    /// # Errors
    /// As [`Self::start`].
    pub fn start_with(
        listen_addr: &str,
        backend_authority: &str,
        config: PassthroughConfig,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(listen_addr)?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        let listener = AsyncFd::new(listener)?;
        let shutdown = Arc::new(OnSignal::new());
        let backend = backend_authority.to_string();
        let (done_tx, done_rx) = std::sync::mpsc::channel();

        spawn_loop(
            accept_loop(listener, backend, Arc::clone(&shutdown), config),
            done_tx,
        )?;

        Ok(Self {
            local_addr,
            shutdown,
            done: done_rx,
            poll: config.shutdown_poll,
        })
    }

    /// The actual bound address (useful when binding to port 0).
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Stop accepting. Clients already spliced run until they close.
    pub fn shutdown(self) {
        stop_loop(&self.shutdown, &self.done, self.poll);
    }
}

impl Drop for TcpPassthrough {
    fn drop(&mut self) {
        self.shutdown.turn_on();
    }
}

/// Accept loop for [`TcpPassthrough`]: one splice future per client.
async fn accept_loop(
    listener: AsyncFd<TcpListener>,
    backend: String,
    shutdown: Arc<OnSignal>,
    config: PassthroughConfig,
) {
    while !shutdown.probe() {
        let accepted = or_tick(
            listener.read_with(std::net::TcpListener::accept),
            config.shutdown_poll,
        )
        .await;
        match accepted {
            None => {}
            Some(Ok((client, _addr))) => {
                let backend = backend.clone();
                let task = async move {
                    if let Err(e) = splice_client_to_backend(client, &backend, config).await {
                        tracing::warn!(%backend, "tcp passthrough failed: {e}");
                    }
                };
                if let Err(e) = send(from_future(task)) {
                    tracing::error!("tcp passthrough cannot start a client relay: {e}");
                }
            }
            Some(Err(e)) => {
                tracing::error!("tcp passthrough accept error: {e}");
                // Back off one poll so a persistent error does not spin.
                sleep_async(config.shutdown_poll).await;
            }
        }
    }
}

/// Connect to the backend and splice a single client connection to it.
async fn splice_client_to_backend(
    client: std::net::TcpStream,
    backend_authority: &str,
    config: PassthroughConfig,
) -> std::io::Result<()> {
    let client = wrap_tcp(client)?;
    let backend = dial_tcp(backend_authority, config.io_mode, config.connect_timeout).await?;
    splice(client, backend, config.writer).await;
    Ok(())
}

/// A raw UDP passthrough: datagrams arriving on `listen_addr` are forwarded to
/// `backend_authority` and responses are sent back to the originating client.
///
/// WHY: Decision 17 — `udp://` backends need byte-level passthrough with no HTTP
/// semantics (DNS, QUIC, game servers, syslog, STUN).
///
/// HOW: Binds a single `UdpSocket`. Each datagram is forwarded to the backend;
/// the backend's response is sent back to the client address that sent the
/// original datagram.  Connectionless — one socket serves all clients. The relay
/// is one future that parks on the socket's read readiness.
///
/// The valtron pool must be initialised before [`UdpPassthrough::start`].
#[derive(Debug)]
pub struct UdpPassthrough {
    local_addr: SocketAddr,
    shutdown: Arc<OnSignal>,
    done: Receiver<()>,
    poll: Duration,
}

impl UdpPassthrough {
    /// Bind `listen_addr` and relay every datagram to `backend_authority`, with
    /// the default [`PassthroughConfig`]. `clock` times the response wait.
    ///
    /// Returns immediately with a handle; the relay runs as a future until
    /// [`UdpPassthrough::shutdown`] (or drop).
    ///
    /// # Errors
    /// Returns the bind error if `listen_addr` cannot be bound, or an error if
    /// the loop cannot be started on the pool.
    pub fn start(
        listen_addr: &str,
        backend_authority: &str,
        clock: Arc<dyn Clock>,
    ) -> std::io::Result<Self> {
        Self::start_with(
            listen_addr,
            backend_authority,
            PassthroughConfig::default(),
            clock,
        )
    }

    /// Like [`Self::start`], with explicit limits.
    ///
    /// # Errors
    /// As [`Self::start`].
    pub fn start_with(
        listen_addr: &str,
        backend_authority: &str,
        config: PassthroughConfig,
        clock: Arc<dyn Clock>,
    ) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(listen_addr)?;
        socket.set_nonblocking(true)?;
        let local_addr = socket.local_addr()?;
        let socket = AsyncFd::new(socket)?;
        let shutdown = Arc::new(OnSignal::new());
        let backend = backend_authority.to_string();
        let (done_tx, done_rx) = std::sync::mpsc::channel();

        spawn_loop(
            relay_udp(socket, backend, Arc::clone(&shutdown), config, clock),
            done_tx,
        )?;

        Ok(Self {
            local_addr,
            shutdown,
            done: done_rx,
            poll: config.shutdown_poll,
        })
    }

    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Stop relaying.
    pub fn shutdown(self) {
        stop_loop(&self.shutdown, &self.done, self.poll);
    }
}

impl Drop for UdpPassthrough {
    fn drop(&mut self) {
        self.shutdown.turn_on();
    }
}

/// Relay loop: client datagram → backend, response → client.
async fn relay_udp(
    socket: AsyncFd<UdpSocket>,
    backend_authority: String,
    shutdown: Arc<OnSignal>,
    config: PassthroughConfig,
    clock: Arc<dyn Clock>,
) {
    let Some(backend) = resolve_first(&backend_authority).await else {
        tracing::error!(backend_addr = %backend_authority, "UDP relay: cannot resolve backend address");
        return;
    };
    let mut buf = vec![0u8; 65535];

    while !shutdown.probe() {
        let received = or_tick(
            socket.read_with(|s| s.recv_from(&mut buf)),
            config.shutdown_poll,
        )
        .await;
        let (n, client_addr) = match received {
            None => continue,
            Some(Ok(pair)) => pair,
            Some(Err(e)) => {
                tracing::error!("UDP relay recv error: {e}");
                return;
            }
        };
        // Forward to the backend.
        if let Err(e) = socket.write_with(|s| s.send_to(&buf[..n], backend)).await {
            tracing::warn!(%backend, "UDP relay send to backend failed: {e}");
            continue;
        }
        // Wait for the response, in slices so shutdown stays responsive.
        let deadline = clock.now() + config.udp_response_timeout;
        let mut responded = false;
        while !responded && clock.now() < deadline && !shutdown.probe() {
            let slice = config
                .shutdown_poll
                .min(deadline.saturating_duration_since(clock.now()));
            match or_tick(socket.read_with(|s| s.recv_from(&mut buf)), slice).await {
                None => {}
                Some(Ok((m, _from))) => {
                    if let Err(e) = socket
                        .write_with(|s| s.send_to(&buf[..m], client_addr))
                        .await
                    {
                        tracing::warn!(%client_addr, "UDP relay send to client failed: {e}");
                    }
                    responded = true;
                }
                Some(Err(e)) => {
                    tracing::warn!("UDP relay recv from backend failed: {e}");
                    break;
                }
            }
        }
        if !responded {
            tracing::trace!(
                "UDP relay: no response from backend within {:?}",
                config.udp_response_timeout
            );
        }
    }
}

/// Resolve the first socket address for `host:port` without blocking a thread.
async fn resolve_first(authority: &str) -> Option<SocketAddr> {
    let (host, port) = authority.rsplit_once(':')?;
    let port = port.parse::<u16>().ok()?;
    SystemDnsResolver
        .resolve(host.trim_matches(['[', ']']), port)
        .await
        .ok()?
        .into_iter()
        .next()
}
