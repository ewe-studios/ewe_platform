//! HTTP/3 server — drives a QUIC endpoint and dispatches request streams to
//! [`H3Serve`] handlers (F35 / spec-53 F19).
//!
//! WHY: `foundation_netio` provides the QUIC substrate ([`QuicDriver`] — one
//! `quinn_proto::Endpoint` per UDP socket) and the HTTP/3 mapping
//! ([`H3Connection`]/[`H3Request`]), but nothing binds a listener and serves an
//! `H3Serve` app. This module is that glue, the H3 analogue of the TCP accept
//! loop + [`H2ConnectionHandler`](super::h2_connection).
//!
//! HOW: two valtron tasks share the endpoint's accept queue. The **driver task**
//! is the `QuicDriver` itself — a `TaskIterator` that does all UDP I/O, routes
//! datagrams to connections, and parks on socket readiness. The **accept task**
//! drains newly handshaked connections, finishes the H3 control-stream setup on
//! each, and for every inbound request stream spawns the handler's `serve_h3`
//! future. Request/response stream I/O rides the connection state the driver
//! keeps pumping, so handler futures run concurrently with the driver.

use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_core::synca::OnSignal;
use foundation_core::valtron::{
    BoxedSendExecutionAction, FutureTask, Stream, TaskIterator, TaskStatus,
};
use foundation_netio::http3::H3Connection;
use foundation_netio::netcap::ConnectionContext;
use foundation_netio::quic::{server_config_from_pem, QuicDriver, QuinnConnection};
use foundation_netio::shared::http::SimpleMethod;

use crate::native::serve::H3Serve;
use crate::shared::app::HttpApp;

type H3App = Arc<HttpApp<Arc<dyn H3Serve>>>;

/// Bind a UDP socket at `bind_addr` and serve `app` over HTTP/3 (QUIC), using the
/// PEM cert+key for the TLS handshake. Returns the actual bound address. Spawns
/// the endpoint driver and the accept/dispatch task on the valtron pool; both
/// stop when `shutdown` fires.
///
/// # Errors
/// Returns an error if the TLS config is invalid, the socket cannot bind, or the
/// tasks cannot be spawned.
pub fn serve_h3(
    bind_addr: SocketAddr,
    cert_pem: &[u8],
    key_pem: &[u8],
    app: H3App,
    shutdown: Arc<OnSignal>,
) -> io::Result<SocketAddr> {
    let cfg = server_config_from_pem(cert_pem, key_pem)?;
    let driver = QuicDriver::server(bind_addr, cfg)?;
    let local = driver.local_addr()?;
    let accept_queue = driver.accept_queue();

    // The driver is the endpoint pump: run it as its own task.
    foundation_core::valtron::send(driver)
        .map_err(|e| io::Error::other(format!("spawn quic driver: {e}")))?;
    foundation_core::valtron::send(H3AcceptTask::new(accept_queue, app, shutdown))
        .map_err(|e| io::Error::other(format!("spawn h3 accept task: {e}")))?;
    Ok(local)
}

/// A live H3 connection being driven by the accept task.
struct ActiveConn {
    h3: H3Connection<QuinnConnection>,
    ctx: Arc<ConnectionContext>,
    /// Our control stream + SETTINGS have been sent.
    setup_done: bool,
    /// The peer's SETTINGS have been read.
    settings_done: bool,
}

/// Drains handshaked QUIC connections and dispatches their request streams.
struct H3AcceptTask {
    accept_queue: Arc<Mutex<VecDeque<QuinnConnection>>>,
    app: H3App,
    conns: Vec<ActiveConn>,
    shutdown: Arc<OnSignal>,
}

impl H3AcceptTask {
    fn new(
        accept_queue: Arc<Mutex<VecDeque<QuinnConnection>>>,
        app: H3App,
        shutdown: Arc<OnSignal>,
    ) -> Self {
        Self { accept_queue, app, conns: Vec::new(), shutdown }
    }
}

impl TaskIterator for H3AcceptTask {
    type Ready = ();
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
        if self.shutdown.probe() {
            return None;
        }

        // Adopt newly handshaked connections.
        loop {
            let next = self.accept_queue.lock().unwrap().pop_front();
            match next {
                Some(conn) => {
                    let ctx = Arc::new(conn.connection_context());
                    self.conns.push(ActiveConn {
                        h3: H3Connection::new(conn),
                        ctx,
                        setup_done: false,
                        settings_done: false,
                    });
                }
                None => break,
            }
        }

        // Drive each connection: send our SETTINGS, read the peer's, accept and
        // dispatch request streams.
        for c in &mut self.conns {
            if !c.setup_done {
                if let Stream::Next(Ok(())) = c.h3.poll_setup() {
                    c.setup_done = true;
                }
            }
            if c.setup_done && !c.settings_done {
                if let Stream::Next(Ok(true)) = c.h3.poll_peer_settings() {
                    c.settings_done = true;
                }
            }
            if c.setup_done {
                loop {
                    match c.h3.poll_accept() {
                        Stream::Next(Ok(request)) => {
                            // The proxy registers a single catch-all H3 handler
                            // (`route_any_h3`); it reads the request's own
                            // headers to route, so any (method, path) resolves it.
                            if let Some(handler) =
                                self.app.router().dispatch(&SimpleMethod::GET, "/")
                            {
                                let fut = handler.serve_h3(
                                    self.app.context().clone(),
                                    Arc::clone(&c.ctx),
                                    request,
                                );
                                let _ = foundation_core::valtron::send(FutureTask::new(fut));
                            }
                        }
                        // No more request streams ready this tick, or the peer
                        // reset/closed — either way stop accepting for now.
                        _ => break,
                    }
                }
            }
        }

        Some(TaskStatus::Delayed(Duration::from_millis(5)))
    }
}
