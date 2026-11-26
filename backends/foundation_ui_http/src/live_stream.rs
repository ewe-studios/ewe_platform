//! Serve live frames through server-sent events.
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
    channel: Option<LiveChannel>,
    native: bool,
    frame_channel: bool,
    config: SseTransportConfig,
}

impl ServeFactory for StreamHandler {
    fn create(ctx: &Ctx) -> Self {
        Self {
            frame_channel: true,
            channel: None,
            native: false,
            config: ctx
                .get::<SseTransportConfig>()
                .map_or_default(|config| *config),
        }
    }
}

impl StreamHandler {
    /// Use the text and binary broadcaster from the request context.
    #[must_use]
    pub fn broadcaster(ctx: &Ctx) -> Self {
        Self {
            frame_channel: false,
            ..Self::create(ctx)
        }
    }

    /// Uses this channel for every subscription on this route.
    #[must_use]
    pub fn with_channel(mut self, channel: LiveChannel) -> Self {
        self.channel = Some(channel);
        self
    }

    /// Opens a native frame stream with the transport settings from this host.
    #[must_use]
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
        if let Some(channel) = self
            .channel
            .clone()
            .map(Arc::new)
            .or_else(|| ctx.get::<LiveChannel>())
            .filter(|_| self.frame_channel)
        {
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
