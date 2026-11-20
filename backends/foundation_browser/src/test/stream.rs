//! # Channel A — the SSE transport glue for the broadcaster (spec-43 feature 01)
//!
//! WHY: Mode 1 streams a native `foundation_wasm_ui` App's protocol-encoded DOM
//! frames to the real browser. The PROTOCOL-AGNOSTIC fan-out (backlog/replay,
//! `Send` push handle, the App sink) is a server capability and lives in
//! `foundation_wasm_ui::server` ([`Broadcaster`] and [`BroadcastTx`]).
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

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{self, FutureTask, Pipe, PipeReceiver, PipeSender};
use foundation_http::native::serve::{Serve, ServeFactory};
use foundation_http::native::upgrade::SseStream;
use foundation_http::shared::context::Ctx;
use foundation_http::shared::serve::{ConnectionResult, ServeOutcome};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::SimpleIncomingRequest;
use foundation_wasm_ui::server::{Broadcaster, FrameTransport, LiveChannel, LiveError};
use std::sync::Arc;

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
async fn drive_sse(
    mut sse: SseStream,
    replay: Vec<Arc<[u8]>>,
    frames: PipeReceiver<OutboundFrame>,
) {
    for frame in replay {
        if let Err(error) = sse.binary(&frame).await {
            tracing::debug!("SSE replay ended: {error}");
            return;
        }
    }
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
    native: bool,
    frame_channel: bool,
    config: SseTransportConfig,
}

impl ServeFactory for StreamHandler {
    fn create(ctx: &Ctx) -> Self {
        Self {
            frame_channel: true,
            native: false,
            config: ctx
                .get::<SseTransportConfig>()
                .map_or_default(|config| *config),
        }
    }
}

impl StreamHandler {
    /// Opens a native frame stream with the transport settings from this host.
    pub fn native(ctx: &Ctx) -> Self {
        Self {
            native: true,
            ..Self::create(ctx)
        }
    }
}
impl Serve for StreamHandler {
    fn serve(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let (frames, receiver) = Pipe::with_depth(self.config.queue_depth.max(1));
        let transport = SseTransport { frames };
        let mut replay = Vec::new();
        let mut legacy = None;
        if let Some(channel) = ctx.get::<LiveChannel>().filter(|_| self.frame_channel) {
            let queries = req.request_url.queries.as_ref();
            let param = |name: &str| {
                queries
                    .and_then(|params| params.get(name))
                    .map(|text| text.parse::<u64>())
                    .transpose()
            };
            let (Ok(stream), Ok(cursor)) = (param("stream_id"), param("resume_from")) else {
                return foundation_http::native::writer::send_bytes(
                    conn,
                    400,
                    "text/plain",
                    "invalid stream cursor",
                    &ctx,
                );
            };
            if stream.is_some_and(|id| id != channel.stream_id()) {
                return foundation_http::native::writer::send_bytes(
                    conn,
                    404,
                    "text/plain",
                    "unknown stream",
                    &ctx,
                );
            }
            replay = match if self.native {
                channel.subscribe_native(transport, cursor.unwrap_or(0))
            } else {
                channel.subscribe_web(transport, cursor.unwrap_or(0))
            } {
                Ok(history) => history,
                Err(error) => {
                    let status = if matches!(error, LiveError::ResumeTooOld { .. }) {
                        410
                    } else {
                        409
                    };
                    return foundation_http::native::writer::send_bytes(
                        conn,
                        status,
                        "text/plain",
                        error.to_string(),
                        &ctx,
                    );
                }
            };
        } else if let Some(channel) = ctx.get::<Broadcaster>() {
            legacy = Some((channel, transport));
        } else {
            return ConnectionResult::Close(None);
        }
        ConnectionResult::Async(Box::pin(async move {
            let sse = match SseStream::new(&conn, &ctx).await {
                Ok(sse) => sse,
                Err(e) => {
                    tracing::warn!("SSE upgrade failed: {e}");
                    return ServeOutcome::Close(None);
                }
            };

            let driver = FutureTask::new(drive_sse(sse, replay, receiver));
            if let Err(e) = valtron::send(driver) {
                tracing::warn!("Cannot start the SSE frame driver: {e}");
                return ServeOutcome::Close(None);
            }

            // The connection is Arc-shared; the driver's clone keeps it
            // open after the worker drops its handle on Take.
            if let Some((channel, transport)) = legacy {
                channel.register(transport);
            }
            ServeOutcome::Take
        }))
    }
}

/// Serve raw text and binary events through the test broadcaster.
pub(crate) struct RawStreamHandler(StreamHandler);

impl ServeFactory for RawStreamHandler {
    fn create(ctx: &Ctx) -> Self {
        let mut handler = StreamHandler::create(ctx);
        handler.frame_channel = false;
        Self(handler)
    }
}

impl Serve for RawStreamHandler {
    fn serve(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        self.0.serve(ctx, req, conn)
    }
}
