//! `ProtocolDetectHandler` — decides HTTP/1.1 vs h2c before a handler is chosen (F47).
//!
//! WHY: The listener sets every accepted socket non-blocking, so at accept time
//! the client has usually sent nothing yet. Peeking in the accept loop would
//! either spin on `WouldBlock` or stall every other pending connection. Protocol
//! detection therefore has to be a task, not a step in the accept loop.
//!
//! WHAT: A valtron `TaskIterator` that peeks (never consumes) the first 24 bytes,
//! compares them against the h2c client preface, and then spawns either
//! `ConnectionHandler` or `H2ConnectionHandler` — handing along the same drain
//! guard so the shutdown drain still accounts for this connection.
//!
//! HOW: Each poll peeks. Fewer than 24 bytes means "undecided" — park on a timer
//! and retry, up to [`DETECT_TIMEOUT`]. Once decided, the task spawns the real
//! handler and completes. A protocol this server does not speak is refused
//! outright, never downgraded to an empty router.

use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::io::ioutils::{PeekError, PeekableReadStream, SharedByteBufferStream};
use foundation_core::synca::{OnSignal, WaitGroupGuard};
use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use foundation_netio::http2::conn::H2Conn;
use foundation_netio::http2::detect::{detect_protocol, DetectedProtocol, H2C_PREFACE_PEEK_LEN};
use foundation_netio::netcap::{ConnectionContext, RawStream};
use foundation_netio::simple_http::shared::HTTPStreams;

use super::connection::ConnectionHandler;
use super::h2_connection::H2ConnectionHandler;
use super::KeepAliveConfig;
use crate::shared::app::ServerApp;
use crate::shared::serve::respond;

const POLL_DELAY: Duration = Duration::from_millis(5);

/// How long to wait for the client's first 24 bytes before giving up. A peer
/// that connects and says nothing must not hold a slot forever.
const DETECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Peeks the connection preamble, then hands off to the protocol's handler.
pub struct ProtocolDetectHandler {
    stream: SharedByteBufferStream<RawStream>,
    app: ServerApp,
    client_ip: String,
    connection: Arc<ConnectionContext>,
    shutdown: Arc<OnSignal>,
    /// Moved into whichever handler we spawn; `None` after hand-off.
    drain_guard: Option<WaitGroupGuard>,
    keep_alive: KeepAliveConfig,
    started: Instant,
}

impl ProtocolDetectHandler {
    #[must_use]
    pub fn new(
        stream: SharedByteBufferStream<RawStream>,
        app: ServerApp,
        client_ip: String,
        connection: Arc<ConnectionContext>,
        shutdown: Arc<OnSignal>,
        drain_guard: WaitGroupGuard,
        keep_alive: KeepAliveConfig,
    ) -> Self {
        Self {
            stream,
            app,
            client_ip,
            connection,
            shutdown,
            drain_guard: Some(drain_guard),
            keep_alive,
            started: Instant::now(),
        }
    }
}

impl TaskIterator for ProtocolDetectHandler {
    type Ready = ();
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
        if self.shutdown.probe() {
            return None;
        }

        let mut buf = [0u8; H2C_PREFACE_PEEK_LEN];
        let peeked = match self.stream.peek(&mut buf) {
            // `peek` reports how much is buffered, which may exceed what it
            // copied into `buf` when the peer sent more than the preface in one
            // segment. Only the bytes actually written to `buf` are readable.
            Ok(n) => n.min(buf.len()),
            // Nothing readable yet — the client has not spoken.
            Err(PeekError::IOError(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => 0,
            Err(e) => {
                tracing::debug!(client = %self.client_ip, err = %e, "protocol peek failed");
                return None;
            }
        };

        match detect_protocol(&buf[..peeked]) {
            DetectedProtocol::NeedMore => {
                if self.started.elapsed() >= DETECT_TIMEOUT {
                    tracing::debug!(client = %self.client_ip, "protocol detection timed out");
                    return None;
                }
                Some(TaskStatus::Delayed(POLL_DELAY))
            }
            DetectedProtocol::H2 => {
                self.spawn_h2();
                None
            }
            DetectedProtocol::Http11 => {
                self.spawn_h1();
                None
            }
        }
    }
}

impl ProtocolDetectHandler {
    /// Hand the connection to `H2ConnectionHandler`. The preface bytes are left
    /// in the stream: `server_handshake()` consumes them itself.
    fn spawn_h2(&mut self) {
        let Some(app) = self.app.get_h2().cloned() else {
            // This server speaks HTTP/1.1 only. Refusing is the honest answer;
            // an h2 peer cannot read an HTTP/1.1 error body, so just close.
            tracing::debug!(
                client = %self.client_ip,
                "h2c preface on an HTTP/1.1-only server; closing"
            );
            return;
        };

        let Some(guard) = self.drain_guard.take() else {
            return;
        };
        let conn = H2Conn::new_server(self.stream.clone());
        let handler = H2ConnectionHandler::new(
            conn,
            app,
            self.connection.clone(),
            self.shutdown.clone(),
            guard,
        );

        if let Err(e) = foundation_core::valtron::send(handler) {
            tracing::error!(client = %self.client_ip, err = ?e, "failed to submit h2 connection");
        }
    }

    /// Hand the connection to the HTTP/1.1 `ConnectionHandler`.
    fn spawn_h1(&mut self) {
        let Some(app) = self.app.get_h1().cloned() else {
            // This server speaks HTTP/2 only. Say so in the protocol the peer
            // is actually speaking, rather than 404-ing from an empty router.
            tracing::debug!(
                client = %self.client_ip,
                "HTTP/1.1 request on an HTTP/2-only server; refusing with 505"
            );
            let _ = respond::text(&mut self.stream.clone(), 505, "HTTP Version Not Supported");
            return;
        };

        let Some(guard) = self.drain_guard.take() else {
            return;
        };
        let streams = HTTPStreams::new(self.stream.clone());
        let handler = ConnectionHandler::new(
            app,
            streams,
            self.stream.clone(),
            self.client_ip.clone(),
            self.connection.clone(),
            self.shutdown.clone(),
            guard,
            &self.keep_alive,
        );

        match foundation_core::valtron::send(handler) {
            Ok(()) => tracing::trace!(client = %self.client_ip, "submitted h1 connection"),
            Err(e) => {
                tracing::error!(client = %self.client_ip, err = ?e, "failed to submit connection");
                let _ = respond::text(&mut self.stream.clone(), 503, "Service Unavailable");
            }
        }
    }
}
