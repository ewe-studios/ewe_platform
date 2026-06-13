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
//! HOW: We use foundation_http's SSE writer API — [`SseStream`] (`message`/`send`/
//! `comment`, built on `foundation_netio::event_source::EventWriter` +
//! [`SseEvent`](foundation_netio::event_source::SseEvent)) — for the response
//! head AND the `data:` lines. (`SseStream` writes a streaming head with no
//! `Content-Length`, which is correct now that the response builder no longer
//! auto-adds `Content-Length: 0` for an absent body.) `StreamHandler` upgrades
//! the owned connection, hands the `SseStream` to the [`Broadcaster`], and
//! returns `ConnectionResult::Take`; the connection is `Arc`-shared, so the
//! worker dropping its handle leaves the broadcaster's `SseStream` holding the
//! socket open — no blocked worker per stream. A push (from the test thread)
//! `message`s every stream; teardown drops them (closing the sockets). The
//! backlog/replay kills the connect/push race.

use std::sync::{Arc, Mutex};

use base64::Engine as _;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::native::upgrade::SseStream;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve, ServeFactory};
use foundation_netio::netcap::RawStream;
use foundation_netio::simple_http::shared::SimpleIncomingRequest;

/// The stream route the browser's runtime connects to.
pub const STREAM_PATH: &str = "/__primal/stream";

/// Owned SSE streams + the frame backlog replayed to late joiners.
struct Inner {
    streams: Vec<SseStream>,
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
        let ok = inner.backlog.iter().all(|frame| sse.message(encode(frame)).is_ok());
        if ok {
            inner.streams.push(sse);
        }
    }

    /// Drop all connections (teardown closes the sockets).
    pub fn close(&self) {
        self.inner.lock().expect("broadcaster poisoned").streams.clear();
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
        let data = encode(frame);
        inner.streams.retain_mut(|sse| sse.message(data.clone()).is_ok());
    }
}

/// Encode a frame for an SSE `data:` line. Frames are protocol bytes; we ship
/// them base64 so binary encoders (columnar/arrow) survive the text channel and
/// the browser runtime can base64-decode. (UTF-8 JSON frames base64 fine too.)
fn encode(frame: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(frame)
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
