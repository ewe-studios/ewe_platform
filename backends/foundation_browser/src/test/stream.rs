//! # Channel A — the broadcaster + SSE stream route (spec-43 feature 01)
//!
//! WHY: Mode 1 streams a native `foundation_wasm_ui` App's protocol-encoded DOM
//! frames to the real browser, where `foundation-wasm-ui.js` applies them. The
//! broadcaster is a PROTOCOL-AGNOSTIC byte pipe: whatever encoder the App uses
//! (json/columnar/arrow) produces a framed `Vec<u8>`, and the broadcaster just
//! delivers it over SSE.
//!
//! WHAT: [`Broadcaster`] (owned SSE streams + a replayed backlog), [`BroadcastTx`]
//! (the `Send` push handle the App's sink / a test pushes frames to), and
//! [`StreamHandler`] (a `Serve` route that upgrades + detaches).
//!
//! HOW: We use foundation_http's SSE writer API — [`SseStream`], whose
//! [`binary`](SseStream::binary) method IS the binary-over-SSE machinery
//! (base64 so columnar/arrow bytes survive the text `data:` line; the browser
//! runtime's `streamEventResult("arrow", …)` decodes it). Nothing bespoke lives
//! here — the broadcaster just hands frames to `SseStream::binary`. (`SseStream`
//! writes a streaming head with no `Content-Length`, correct now that the
//! response builder no longer auto-adds `Content-Length: 0` for an absent body.)
//! `StreamHandler` upgrades the owned connection, hands the `SseStream` to the
//! [`Broadcaster`], and returns `ConnectionResult::Take`; the connection is
//! `Arc`-shared, so the worker dropping its handle leaves the broadcaster's
//! `SseStream` holding the socket open — no blocked worker per stream. A push
//! (from the test thread) writes every stream; teardown drops them (closing the
//! sockets). The backlog/replay kills the connect/push race.

use std::sync::{Arc, Mutex};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::native::upgrade::SseStream;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve, ServeFactory};
use foundation_netio::netcap::RawStream;
use foundation_netio::simple_http::shared::SimpleIncomingRequest;

/// The stream route the browser's runtime connects to.
pub const STREAM_PATH: &str = "/__primal/stream";

/// One channel-A frame. Binary frames are envelope-encoded DomOp batches
/// (columnar/arrow/json) delivered base64 over SSE; text frames are raw markup
/// (the HTML protocol), delivered as a plain SSE `data:` line.
enum Frame {
    Binary(Vec<u8>),
    Text(String),
}

impl Frame {
    /// Write this frame to an SSE stream the way its protocol expects.
    fn write(&self, sse: &mut SseStream) -> bool {
        match self {
            // `SseStream::binary` is foundation_http machinery for the
            // binary-over-SSE contract (base64); `message` is a plain data line.
            Frame::Binary(bytes) => sse.binary(bytes).is_ok(),
            Frame::Text(text) => sse.message(text.clone()).is_ok(),
        }
    }
}

/// Owned SSE streams + the frame backlog replayed to late joiners.
struct Inner {
    streams: Vec<SseStream>,
    backlog: Vec<Frame>,
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
        Self { inner: Arc::new(Mutex::new(Inner { streams: Vec::new(), backlog: Vec::new() })) }
    }

    /// A `Send` handle for pushing frames (held by the App's sink / the test).
    #[must_use]
    pub fn sender(&self) -> BroadcastTx {
        BroadcastTx { inner: self.inner.clone() }
    }

    /// Register a freshly-upgraded SSE stream, replaying the backlog into it.
    fn register(&self, mut sse: SseStream) {
        let mut inner = self.inner.lock().expect("broadcaster poisoned");
        let ok = inner.backlog.iter().all(|frame| frame.write(&mut sse));
        if ok {
            inner.streams.push(sse);
        }
    }

    /// Drop all connections (teardown closes the sockets).
    pub fn close(&self) {
        self.inner.lock().expect("broadcaster poisoned").streams.clear();
    }

    /// Number of frames pushed so far (diagnostics).
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.inner.lock().expect("broadcaster poisoned").backlog.len()
    }

    /// Number of currently-registered SSE connections (diagnostics).
    #[must_use]
    pub fn conn_count(&self) -> usize {
        self.inner.lock().expect("broadcaster poisoned").streams.len()
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
    /// Deliver one BINARY (envelope-encoded) frame to all connections (and the
    /// backlog) — the DomOp wires: columnar, arrow IPC, JSON.
    pub fn send(&self, frame: &[u8]) {
        self.push(Frame::Binary(frame.to_vec()));
    }

    /// Deliver one TEXT frame (raw markup) — the HTML protocol, written as a
    /// plain SSE `data:` line that the runtime routes through `routeHtml`.
    pub fn send_text(&self, markup: &str) {
        self.push(Frame::Text(markup.to_string()));
    }

    fn push(&self, frame: Frame) {
        let mut inner = self.inner.lock().expect("broadcaster poisoned");
        inner.streams.retain_mut(|sse| frame.write(sse));
        inner.backlog.push(frame);
    }
}

/// `Serve` route: upgrade the owned connection to SSE, hand it to the
/// [`Broadcaster`], and detach.
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
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let Some(bc) = bag.get::<Broadcaster>() else {
            return ConnectionResult::Close(None);
        };
        match SseStream::new(&mut conn) {
            Ok(sse) => {
                // The connection is Arc-shared; the broadcaster's SseStream clone
                // keeps it open after the worker drops its handle on Take.
                bc.register(sse);
                ConnectionResult::Take
            }
            Err(e) => {
                tracing::warn!("SSE upgrade failed: {e}");
                ConnectionResult::Close(None)
            }
        }
    }
}
