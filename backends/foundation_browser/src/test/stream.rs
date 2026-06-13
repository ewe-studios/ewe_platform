//! # Channel A — the broadcaster + SSE stream route (spec-43 feature 01)
//!
//! WHY: Mode 1 streams a native `foundation_wasm_ui` App's protocol-encoded DOM
//! frames to the real browser, where `foundation-wasm-ui.js` applies them. The
//! broadcaster is a PROTOCOL-AGNOSTIC byte pipe: whatever encoder the App uses
//! (json/columnar/arrow) produces a framed `Vec<u8>`, and the broadcaster just
//! delivers it over SSE.
//!
//! WHAT: [`Broadcaster`] (owned SSE connections + a replayed backlog),
//! [`BroadcastTx`] (the `Send` write handle the App's sink / a test pushes
//! frames to), and [`StreamHandler`] (a `Serve` route that upgrades + detaches).
//!
//! HOW: `StreamHandler` writes a STREAMING SSE response head (NO `Content-Length`
//! — otherwise the browser reads zero bytes and treats the body as complete),
//! then clones the owned connection into the [`Broadcaster`] and returns
//! `ConnectionResult::Take`. The connection is `Arc`-shared (`SharedByteBufferStream`),
//! so the worker dropping its handle doesn't close the socket — the broadcaster's
//! clone keeps it live. A push from the test thread writes `data:` frames to
//! every connection; teardown drops them (closing the sockets). Backlog/replay
//! kills the connect/push race.

use std::io::Write;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve, ServeFactory};
use foundation_netio::netcap::RawStream;
use foundation_netio::simple_http::shared::SimpleIncomingRequest;

/// The stream route the browser's runtime connects to.
pub const STREAM_PATH: &str = "/__primal/stream";

/// A streaming-SSE response head — deliberately NO `Content-Length` so the body
/// is open-ended (terminated by connection close), not zero-length.
const SSE_HEAD: &[u8] = b"HTTP/1.1 200 OK\r\n\
Content-Type: text/event-stream\r\n\
Cache-Control: no-cache\r\n\
Connection: keep-alive\r\n\
X-Accel-Buffering: no\r\n\r\n";

type Conn = SharedByteBufferStream<RawStream>;

/// Owned SSE connections + the frame backlog replayed to late joiners.
struct Inner {
    conns: Vec<Conn>,
    backlog: Vec<Vec<u8>>,
}

/// Shared fan-out of channel-A connections.
#[derive(Clone)]
pub struct Broadcaster {
    inner: Arc<Mutex<Inner>>,
}

impl Broadcaster {
    /// Empty broadcaster.
    #[must_use]
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(Inner { conns: Vec::new(), backlog: Vec::new() })) }
    }

    /// A `Send` handle for pushing frames (held by the App's sink / the test).
    #[must_use]
    pub fn sender(&self) -> BroadcastTx {
        BroadcastTx { inner: self.inner.clone() }
    }

    /// Register a freshly-upgraded SSE connection, replaying the backlog into it.
    fn register(&self, mut conn: Conn) {
        let mut inner = self.inner.lock().expect("broadcaster poisoned");
        let ok = inner.backlog.iter().all(|frame| write_sse(&mut conn, frame));
        if ok {
            inner.conns.push(conn);
        }
    }

    /// Drop all connections (teardown closes the sockets).
    pub fn close(&self) {
        self.inner.lock().expect("broadcaster poisoned").conns.clear();
    }
}

impl Default for Broadcaster {
    fn default() -> Self {
        Self::new()
    }
}

/// A `Send` write handle: push a protocol frame to every connected browser.
#[derive(Clone)]
pub struct BroadcastTx {
    inner: Arc<Mutex<Inner>>,
}

impl BroadcastTx {
    /// Deliver one framed batch to all connections (and the backlog).
    pub fn send(&self, frame: &[u8]) {
        let mut inner = self.inner.lock().expect("broadcaster poisoned");
        inner.backlog.push(frame.to_vec());
        let frame = frame.to_vec();
        inner.conns.retain_mut(|conn| write_sse(conn, &frame));
    }
}

/// Write one frame as an SSE `data:` line (base64 so binary encoders survive the
/// text channel; the browser runtime base64-decodes). Returns `false` on write
/// error so the dead connection is dropped.
fn write_sse(conn: &mut Conn, frame: &[u8]) -> bool {
    let b64 = base64::engine::general_purpose::STANDARD.encode(frame);
    let line = format!("data: {b64}\n\n");
    conn.write_all(line.as_bytes()).and_then(|()| conn.flush()).is_ok()
}

/// `Serve` route: upgrade the owned connection to a streaming SSE response, hand
/// it to the [`Broadcaster`], and detach.
pub struct StreamHandler;

impl ServeFactory for StreamHandler {
    fn create(_bag: &ContextBag) -> Self {
        StreamHandler
    }
}

impl Serve for StreamHandler {
    fn serve(
        &self,
        bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        mut conn: Conn,
    ) -> ConnectionResult {
        let Some(bc) = bag.get::<Broadcaster>() else {
            return ConnectionResult::Close(None);
        };
        if conn.write_all(SSE_HEAD).and_then(|()| conn.flush()).is_err() {
            return ConnectionResult::Close(None);
        }
        // The connection is Arc-shared; the broadcaster's clone keeps it open
        // after the worker drops its handle on Take.
        bc.register(conn.clone());
        ConnectionResult::Take
    }
}
