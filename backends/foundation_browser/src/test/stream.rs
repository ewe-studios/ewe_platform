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
//! WHAT: [`SseTransport`] (a [`FrameTransport`] over `foundation_http`'s
//! [`SseStream`]), [`SseTransportConfig`] (the per-client queue bound), and
//! [`StreamHandler`] (a `Serve` route that upgrades + detaches).
//!
//! HOW: [`FrameTransport`] is synchronous because the broadcaster pushes from
//! the App's (synchronous) `stabilize`. [`SseStream`] is async (spec 70 feature
//! 82): every event goes through `ResponseWriter`, which parks on write
//! readiness instead of truncating an event. A queue joins the two. `SseTransport`
//! pushes frames into a bounded valtron [`Pipe`]; one driver task per client
//! awaits the pipe and writes each frame with `SseStream::binary`/`message`.
//! [`SseStream::binary`] is the binary-over-SSE machinery (base64 so
//! columnar/arrow/json bytes survive the text `data:` line; the runtime's
//! `streamEventResult` decodes it); [`SseStream::message`] is a plain `data:`
//! line for the HTML protocol. `StreamHandler` upgrades the owned connection,
//! starts the driver, registers the transport with the broadcaster, and returns
//! `ConnectionResult::Take`; the connection is `Arc`-shared, so the worker
//! dropping its handle leaves the driver's clone holding the socket open — no
//! blocked worker per stream. (`SseStream` writes a streaming head with no
//! `Content-Length`, correct now that the response builder no longer auto-adds
//! `Content-Length: 0` for an absent body.)

use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{self, FutureTask, Pipe, PipeReceiver, PipeSender};
use foundation_http::native::serve::{Serve, ServeFactory};
use foundation_http::native::upgrade::SseStream;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, ServeOutcome};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::SimpleIncomingRequest;
use foundation_wasm_ui::server::{Broadcaster, FrameTransport};

/// The stream route the browser's runtime connects to.
pub const STREAM_PATH: &str = "/__primal/stream";

/// One frame waiting to be written to one SSE client.
enum OutboundFrame {
    /// An envelope-encoded DOM batch; base64-encoded on the wire.
    Binary(Vec<u8>),
    /// Raw markup (the HTML protocol).
    Text(String),
}

/// Limits for the per-client frame queue.
///
/// WHY: The broadcaster pushes synchronously; the SSE writer is async. The
/// queue absorbs that hand-off without blocking the App's thread. It is
/// bounded so a client that stops reading cannot grow server memory without
/// limit.
/// WHAT: `queue_depth` is the number of frames that may wait for one client.
///
/// When the queue is full, the transport refuses the frame (`FrameTransport`
/// returns `false`), the broadcaster drops that client, and the browser
/// reconnects and replays the backlog. No frame is silently lost for a client
/// that stays registered.
#[derive(Clone, Copy, Debug)]
pub struct SseTransportConfig {
    /// Frames that may wait for one client. Default 256 frames: a small
    /// per-connection bound in the spirit of nginx's `proxy_buffers` (8 buffers
    /// per connection), large enough to hold the broadcaster's backlog burst on
    /// connect while the first socket writes drain.
    pub queue_depth: usize,
}

impl Default for SseTransportConfig {
    fn default() -> Self {
        Self { queue_depth: 256 }
    }
}

impl SseTransportConfig {
    /// The default limits. See [`Self::queue_depth`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set [`Self::queue_depth`].
    #[must_use]
    pub fn with_queue_depth(mut self, depth: usize) -> Self {
        self.queue_depth = depth;
        self
    }
}

/// A [`FrameTransport`] over a bounded frame queue drained by a driver task.
///
/// The driver owns the [`SseStream`] and awaits each queued frame, so the
/// synchronous broadcaster never touches the socket.
struct SseTransport {
    frames: PipeSender<OutboundFrame>,
}

impl FrameTransport for SseTransport {
    fn send_binary(&mut self, frame: &[u8]) -> bool {
        self.frames
            .try_send(OutboundFrame::Binary(frame.to_vec()))
            .is_ok()
    }

    fn send_text(&mut self, text: &str) -> bool {
        self.frames
            .try_send(OutboundFrame::Text(text.to_string()))
            .is_ok()
    }
}

/// Write every queued frame to the client, in order, until the pipe closes or a
/// write fails.
async fn drive_sse(mut sse: SseStream, frames: PipeReceiver<OutboundFrame>) {
    while let Some(frame) = frames.receive().await {
        let result = match frame {
            OutboundFrame::Binary(bytes) => sse.binary(&bytes).await,
            OutboundFrame::Text(text) => sse.message(text).await,
        };
        if let Err(e) = result {
            tracing::debug!("SSE frame write failed, closing the client: {e}");
            return;
        }
    }
}

/// `Serve` route: upgrade the owned connection to SSE, hand it to the
/// [`Broadcaster`], and detach.
pub struct StreamHandler {
    config: SseTransportConfig,
}

impl ServeFactory for StreamHandler {
    fn create(bag: &ContextBag) -> Self {
        Self {
            config: bag
                .get::<SseTransportConfig>()
                .map(|config| *config)
                .unwrap_or_default(),
        }
    }
}

impl Serve for StreamHandler {
    fn serve(
        &self,
        bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let config = self.config;

        // The SSE head write is asynchronous (spec 70 feature 63): the upgrade
        // parks on socket readiness instead of blocking the worker, so the
        // whole upgrade runs as the handler's async continuation. The frame
        // driver is a separate valtron task because the broadcaster pushes
        // synchronously (spec 70 feature 82).
        ConnectionResult::Async(Box::pin(async move {
            let Some(bc) = bag.get::<Broadcaster>() else {
                return ServeOutcome::Close(None);
            };
            let sse = match SseStream::new(&conn).await {
                Ok(sse) => sse,
                Err(e) => {
                    tracing::warn!("SSE upgrade failed: {e}");
                    return ServeOutcome::Close(None);
                }
            };

            let (frames, receiver) = Pipe::with_depth(config.queue_depth.max(1));
            let driver = FutureTask::new(drive_sse(sse, receiver));
            if let Err(e) = valtron::send(driver) {
                tracing::warn!("Cannot start the SSE frame driver: {e}");
                return ServeOutcome::Close(None);
            }

            // The connection is Arc-shared; the driver's clone keeps it
            // open after the worker drops its handle on Take.
            bc.register(SseTransport { frames });
            ServeOutcome::Take
        }))
    }
}
