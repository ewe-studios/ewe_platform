//! # Channel A — the SSE transport glue for the broadcaster (spec-43 feature 01)
//!
//! WHY: Mode 1 streams a native `foundation_wasm_ui` App's protocol-encoded DOM
//! frames to the real browser. The PROTOCOL-AGNOSTIC fan-out (backlog/replay,
//! `Send` push handle, the App sink) is a server capability and lives in
//! `foundation_wasm_ui::server` ([`Broadcaster`]/[`BroadcastTx`]/[`BroadcastSink`]).
//! What stays HERE is only the foundation_http-specific glue: how an SSE stream
//! satisfies the broadcaster's [`FrameTransport`] contract, and the `Serve` route
//! that upgrades a connection and registers it.
//!
//! WHAT: [`SseTransport`] (an [`FrameTransport`] over foundation_http's
//! [`SseStream`]) and [`StreamHandler`] (a `Serve` route that upgrades + detaches).
//!
//! HOW: [`SseStream::binary`] is the binary-over-SSE machinery (base64 so
//! columnar/arrow/json bytes survive the text `data:` line; the runtime's
//! `streamEventResult` decodes it); [`SseStream::message`] is a plain `data:`
//! line for the HTML protocol. `StreamHandler` upgrades the owned connection,
//! wraps it in `SseTransport`, registers it with the broadcaster, and returns
//! `ConnectionResult::Take`; the connection is `Arc`-shared, so the worker
//! dropping its handle leaves the broadcaster's clone holding the socket open —
//! no blocked worker per stream. (`SseStream` writes a streaming head with no
//! `Content-Length`, correct now that the response builder no longer auto-adds
//! `Content-Length: 0` for an absent body.)

use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::native::upgrade::SseStream;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve, ServeFactory};
use foundation_netio::netcap::RawStream;
use foundation_netio::simple_http::shared::SimpleIncomingRequest;
use foundation_wasm_ui::server::{Broadcaster, FrameTransport};

/// The stream route the browser's runtime connects to.
pub const STREAM_PATH: &str = "/__primal/stream";

/// A [`FrameTransport`] over a foundation_http SSE stream — the SSE half of the
/// binary/text-over-SSE contract the browser runtime decodes.
struct SseTransport(SseStream);

impl FrameTransport for SseTransport {
    fn send_binary(&mut self, frame: &[u8]) -> bool {
        self.0.binary(frame).is_ok()
    }
    fn send_text(&mut self, text: &str) -> bool {
        self.0.message(text.to_string()).is_ok()
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
                // The connection is Arc-shared; the broadcaster's clone keeps it
                // open after the worker drops its handle on Take.
                bc.register(SseTransport(sse));
                ConnectionResult::Take
            }
            Err(e) => {
                tracing::warn!("SSE upgrade failed: {e}");
                ConnectionResult::Close(None)
            }
        }
    }
}
