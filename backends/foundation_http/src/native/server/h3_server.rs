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
use std::task::{Wake, Waker};
use std::time::Duration;

use foundation_core::synca::OnSignal;
use foundation_core::valtron::{
    AnyReadiness, BoolSignal, Ctx, EventReadiness, FutureTask, Stream, TaskIterator, TaskStatus,
};
use foundation_netio::http3::connection::error_code;
use foundation_netio::http3::{H3Connection, H3Error};
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
    let accept_wakers = driver.accept_wakers();
    foundation_core::valtron::send(driver)
        .map_err(|e| io::Error::other(format!("spawn quic driver: {e}")))?;
    foundation_core::valtron::send(H3AcceptTask::new(accept_queue, accept_wakers, app, shutdown))
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
///
/// It parks between turns: its waker is registered on the driver's accept list
/// and on every connection, so it runs again when a connection is handshaked
/// or one of them makes progress (SETTINGS, a new request stream, a close) —
/// or when `shutdown` fires. No timer.
struct H3AcceptTask {
    accept_queue: Arc<Mutex<VecDeque<QuinnConnection>>>,
    accept_wakers: Arc<Mutex<Vec<Waker>>>,
    app: H3App,
    conns: Vec<ActiveConn>,
    shutdown: Arc<OnSignal>,
    /// Set by `waker`; the task parks on it.
    woken: BoolSignal,
    /// One waker for the task's life, so `will_wake` dedupes its
    /// registrations.
    waker: Waker,
}

impl H3AcceptTask {
    fn new(
        accept_queue: Arc<Mutex<VecDeque<QuinnConnection>>>,
        accept_wakers: Arc<Mutex<Vec<Waker>>>,
        app: H3App,
        shutdown: Arc<OnSignal>,
    ) -> Self {
        let woken = BoolSignal::new(false);
        Self {
            accept_queue,
            accept_wakers,
            app,
            conns: Vec::new(),
            shutdown,
            waker: Waker::from(Arc::new(SignalWaker(woken.clone()))),
            woken,
        }
    }
}

/// A `Waker` that sets the accept task's signal (and asks the executor to run).
struct SignalWaker(BoolSignal);

impl Wake for SignalWaker {
    fn wake(self: Arc<Self>) {
        self.0.set(true);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.set(true);
    }
}

/// Ready once the server is shutting down.
struct ShutdownRequested(Arc<OnSignal>);

impl EventReadiness for ShutdownRequested {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.0.probe()
    }
}

impl TaskIterator for H3AcceptTask {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<(), ()>> {
        if self.shutdown.probe() {
            return None;
        }

        // Register before looking for work, so progress made while we look
        // still wakes us.
        self.woken.set(false);
        let waker = self.waker.clone();
        if let Ok(mut wakers) = self.accept_wakers.lock() {
            if !wakers.iter().any(|w| w.will_wake(&waker)) {
                wakers.push(waker.clone());
            }
        }

        // Adopt newly handshaked connections.
        let mut progressed = false;
        loop {
            let next = self
                .accept_queue
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .pop_front();
            match next {
                Some(conn) => {
                    let ctx = Arc::new(conn.connection_context());
                    self.conns.push(ActiveConn {
                        h3: H3Connection::new(conn),
                        ctx,
                        setup_done: false,
                        settings_done: false,
                    });
                    progressed = true;
                }
                None => break,
            }
        }

        // Drive each connection: send our SETTINGS, read the peer's, accept and
        // dispatch request streams. A connection whose setup or accept fails is
        // gone (closed, reset, or broken framing), as is one that cannot wake
        // us: drop it rather than keep polling a dead connection for ever.
        let app = &self.app;
        self.conns.retain_mut(|c| {
            if !c.h3.register_waker(&waker) {
                tracing::debug!("h3 connection cannot be waited on; dropping it");
                return false;
            }
            match drive_connection(c, app) {
                Ok(()) => true,
                Err(e) => {
                    tracing::debug!(error = %e, "h3 connection ended; dropping it");
                    false
                }
            }
        });

        if progressed {
            // New connections may already have streams waiting.
            return Some(TaskStatus::Pending(()));
        }
        Some(TaskStatus::Depends(Arc::new(AnyReadiness::either(
            Arc::new(self.woken.clone()),
            Arc::new(ShutdownRequested(Arc::clone(&self.shutdown))),
        ))))
    }
}

/// One accept-task turn for `c`: finish setup, then dispatch every request
/// stream that is ready. `Err` means the connection is gone.
fn drive_connection(c: &mut ActiveConn, app: &H3App) -> Result<(), H3Error> {
    if !c.setup_done {
        match c.h3.poll_setup() {
            Stream::Next(Ok(())) => c.setup_done = true,
            Stream::Next(Err(e)) => return Err(e),
            _ => {}
        }
    }
    if c.setup_done && !c.settings_done {
        match c.h3.poll_peer_settings() {
            Stream::Next(Ok(true)) => c.settings_done = true,
            Stream::Next(Ok(false)) => {}
            Stream::Next(Err(e)) => return Err(e),
            _ => {}
        }
    }
    if !c.setup_done {
        return Ok(());
    }
    loop {
        match c.h3.poll_accept() {
            Stream::Next(Ok(mut request)) => {
                // The proxy registers a single catch-all H3 handler
                // (`route_any_h3`); it reads the request's own headers to
                // route, so any (method, path) resolves it.
                let Some(handler) = app.router().dispatch(&SimpleMethod::GET, "/") else {
                    tracing::debug!("no h3 handler registered; rejecting the request");
                    request.reset(error_code::H3_REQUEST_REJECTED);
                    continue;
                };
                let fut = handler.serve_h3(app.context().clone(), Arc::clone(&c.ctx), request);
                if let Err(e) = foundation_core::valtron::send(FutureTask::new(fut)) {
                    // The request moved into the future, which is dropped with
                    // the failed spawn; the peer sees its stream go away.
                    tracing::error!(error = ?e, "could not spawn the h3 request handler");
                }
            }
            Stream::Next(Err(e)) => return Err(e),
            // No more request streams ready this tick.
            _ => return Ok(()),
        }
    }
}
