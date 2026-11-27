//! The WebSocket transport for Durable Streams.
//!
//! WHY: The protocol core is transport-neutral. A WebSocket server needs one
//! adapter that upgrades the connection and maps each JSON request to
//! [`DurableService`]. The core keeps no WebSocket type.
//!
//! WHAT: [`DurableWsHandler`] upgrades a GET request, then serves `create`,
//! `append`, `close`, `head`, `delete`, `fork` and `read` over JSON text
//! frames. A live `read` pushes one frame per page until the client sends
//! `cancel` or the stream closes. [`DurableWsConfig`] holds the limits.
//!
//! HOW: The 101 response and every frame go out through the async
//! [`ResponseWriter`], so a non-blocking socket cannot truncate a frame. The
//! inbound path uses the resumable [`WebSocketFrameDecoder`] and parks on the
//! connection's read readiness through [`ReadWait`]. A live read also parks on
//! the stream notifier, so an append wakes it at once.
//!
//! # Message schema
//!
//! The `message` module gives the full request and reply forms. A request
//! carries an `id` and an `op`. A reply repeats the `id`. A live read sends
//! `read` for its first page, `data` for each later page and `end` when it
//! stops.

use std::future::{poll_fn, Future};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, BadRequestKind, Chunk, CloseRequest, ContentType, CreateSpec, DurableLimits,
    DurableService, DurableStream, Expiry, ForkSpec, ReadFrom, ReadLimit, ReadRequest, StoreError,
    StreamName, StreamSeq,
};
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::io::{DecodeStep, IncrementalDecoder};
use foundation_core::valtron::timer;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{SimpleHeader, SimpleIncomingRequest, SimpleMethod};
use foundation_netio::websocket::{Opcode, WebSocketFrame, WebSocketFrameDecoder};

use crate::native::serve::{Serve, ServeFactory};
use crate::native::upgrade::accept_websocket;
use crate::native::writer::{render_head, write_response, ReadWait, ResponseWriter, WriterConfig};
use crate::shared::durable::parse::{decode_offset, parse_offset};
use crate::shared::serve::{ConnectionResult, ServeOutcome};

use super::message::{decode, WsError, WsOp, WsReply, WsRequest};

/// The limits of the WebSocket transport.
///
/// Every value is configuration, never a constant. The host stores one in
/// [`Ctx`] before it mounts [`DurableWsHandler`].
#[derive(Clone, Copy, Debug)]
pub struct DurableWsConfig {
    /// The largest inbound request or outbound reply frame, in bytes.
    ///
    /// A larger inbound frame closes the connection with WebSocket code 1009.
    /// A page that would exceed it answers a `frame_too_large` error. Default
    /// 8 MiB: the `DurableLimits::max_body_bytes` default and the wasm bridge
    /// default, below the Cloudflare 100 MB cap and above the nginx 1 MiB
    /// request default. Keep it above `DurableLimits::page_bytes` with room for
    /// base64 expansion.
    pub max_frame_bytes: usize,
    /// How long a wait parks on a transport that cannot report read readiness.
    ///
    /// Default 10 ms, the same fallback the response writer and the
    /// `ConnectRPC` server use.
    pub read_fallback_interval: Duration,
    /// The limits for frame writes: the send timeout and the fallback interval.
    pub writer: WriterConfig,
}

impl Default for DurableWsConfig {
    fn default() -> Self {
        Self {
            max_frame_bytes: 8 * 1024 * 1024,
            read_fallback_interval: Duration::from_millis(10),
            writer: WriterConfig::default(),
        }
    }
}

impl DurableWsConfig {
    /// Set the frame size cap.
    #[must_use]
    pub fn with_max_frame_bytes(mut self, bytes: usize) -> Self {
        self.max_frame_bytes = bytes;
        self
    }

    /// Set the fallback read-park interval.
    #[must_use]
    pub fn with_read_fallback_interval(mut self, interval: Duration) -> Self {
        self.read_fallback_interval = interval;
        self
    }

    /// Set the frame writer limits.
    #[must_use]
    pub fn with_writer(mut self, writer: WriterConfig) -> Self {
        self.writer = writer;
        self
    }
}

/// The Durable Streams WebSocket handler.
///
/// Mount it on a GET route. It upgrades the connection and serves the protocol
/// on that connection until the client closes it.
pub struct DurableWsHandler {
    service: Option<Arc<DurableService<DurableStream>>>,
    config: DurableWsConfig,
}

impl ServeFactory for DurableWsHandler {
    fn create(ctx: &Ctx) -> Self {
        let limits = ctx
            .get::<DurableLimits>()
            .map_or_else(DurableLimits::default, |limits| *limits);
        let service = ctx
            .get::<DurableStream>()
            .map(|stream| Arc::new(DurableService::with_limits((*stream).clone(), limits)));
        let config = ctx
            .get::<DurableWsConfig>()
            .map_or_else(DurableWsConfig::default, |config| *config);
        Self { service, config }
    }
}

impl Serve for DurableWsHandler {
    fn serve(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let Some(service) = self.service.clone() else {
            return crate::native::writer::send_text(
                conn,
                500,
                "durable store is not in the context",
                &ctx,
            );
        };
        let config = self.config;

        ConnectionResult::Async(Box::pin(async move {
            if req.method != SimpleMethod::GET {
                return reject(conn, &ctx, 405, "method not allowed").await;
            }
            if !accept_websocket(&conn, &req, &ctx).await {
                return reject(conn, &ctx, 400, "expected a WebSocket upgrade request").await;
            }
            serve_connection(service, config, ctx, conn).await
        }))
    }
}

/// Write one plain-text refusal and close the connection.
async fn reject(
    conn: SharedByteBufferStream<RawStream>,
    ctx: &Ctx,
    status: u16,
    body: &str,
) -> ServeOutcome {
    let headers = [(
        SimpleHeader::CONTENT_TYPE,
        "text/plain; charset=utf-8".to_string(),
    )];
    let head = match render_head(status, &headers, body.len()) {
        Ok(head) => head,
        Err(error) => {
            tracing::error!("durable websocket: response build failed: {error}");
            return ServeOutcome::Close(None);
        }
    };
    write_response(
        ResponseWriter::new(conn, WriterConfig::default(), ctx.clone()),
        head,
        body.as_bytes(),
    )
    .await
}

/// One open live read on a connection.
struct LiveRead {
    /// The `id` of the read request.
    id: u64,
    /// The stream name.
    name: StreamName,
    /// Where the next page starts.
    next: ReadFrom,
    /// The page limit the client asked for.
    limit: ReadLimit,
    /// True until the first page frame goes out.
    first: bool,
}

/// The outcome of one inbound frame.
enum FrameFlow {
    /// Keep serving.
    Continue,
    /// The client sent a Close frame.
    Close,
}

/// What one live push found.
enum LiveStep {
    /// The page had no new chunks and the stream is open.
    Idle,
    /// The live read ended.
    Closed,
}

/// What one non-blocking decode found.
enum TryFrame {
    /// One complete frame.
    Frame(WebSocketFrame),
    /// No complete frame is available.
    Empty,
    /// The peer closed the connection.
    Eof,
}

/// The connection I/O for one WebSocket session.
struct WsIo {
    /// The read half: the decoder consumes bytes from here.
    reader: SharedByteBufferStream<RawStream>,
    /// The write half: a frame writer is built from this for each send.
    writer_conn: SharedByteBufferStream<RawStream>,
    /// The resumable frame decoder.
    decoder: WebSocketFrameDecoder,
    /// The limits.
    config: DurableWsConfig,
    /// The context whose clock measures the send timeout.
    ctx: Ctx,
}

impl WsIo {
    /// Build the I/O over one upgraded connection.
    fn new(conn: SharedByteBufferStream<RawStream>, config: DurableWsConfig, ctx: Ctx) -> Self {
        Self {
            reader: conn.clone(),
            writer_conn: conn,
            decoder: WebSocketFrameDecoder::with_limit(config.max_frame_bytes),
            config,
            ctx,
        }
    }

    /// Decode one frame without waiting.
    ///
    /// # Errors
    /// Returns [`WsError`] when the connection fails or the frame is invalid.
    fn try_next_frame(&mut self) -> Result<TryFrame, WsError> {
        match self.reader.probe() {
            Ok(0) => Ok(TryFrame::Eof),
            Ok(_) => match self.decoder.step(&mut self.reader) {
                Ok(DecodeStep::Frame(frame)) => Ok(TryFrame::Frame(frame)),
                Ok(DecodeStep::Pending) => Ok(TryFrame::Empty),
                Err(error) => Err(WsError::Protocol(error.to_string())),
            },
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(TryFrame::Empty),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => Ok(TryFrame::Empty),
            Err(error) => Err(WsError::Io(error)),
        }
    }

    /// Park until the connection has bytes to read or the peer hangs up.
    ///
    /// # Errors
    /// Returns [`WsError`] when the connection cannot report readiness.
    async fn wait_readable(&self) -> Result<(), WsError> {
        let mut wait = ReadWait::new(&self.reader);
        wait.readable(self.config.read_fallback_interval, &self.ctx)
            .await
            .map_err(WsError::Io)
    }

    /// Park until the connection is readable or the stream appends or closes.
    ///
    /// A store without a notifier parks on the fallback interval instead, so a
    /// live read still sees new pages and a `cancel`.
    ///
    /// # Errors
    /// Returns [`WsError`] when the connection cannot report readiness.
    async fn wait_live(&self, signal: Option<AppendWait>) -> Result<(), WsError> {
        let mut wait = ReadWait::new(&self.reader);
        let readable = wait.readable(self.config.read_fallback_interval, &self.ctx);
        let mut readable = Box::pin(readable);

        if let Some(mut signal) = signal {
            return poll_fn(|cx| {
                if Pin::new(&mut signal).poll(cx).is_ready() {
                    return Poll::Ready(Ok(()));
                }
                match readable.as_mut().poll(cx) {
                    Poll::Ready(result) => Poll::Ready(result),
                    Poll::Pending => Poll::Pending,
                }
            })
            .await
            .map_err(WsError::Io);
        }

        let mut deadline = Box::pin(timer::sleep_for(
            self.config.read_fallback_interval,
            &self.ctx,
        ));
        poll_fn(|cx| {
            match readable.as_mut().poll(cx) {
                Poll::Ready(result) => return Poll::Ready(result),
                Poll::Pending => {}
            }
            match deadline.as_mut().poll(cx) {
                Poll::Ready(()) => Poll::Ready(Ok(())),
                Poll::Pending => Poll::Pending,
            }
        })
        .await
        .map_err(WsError::Io)
    }

    /// Send one frame through the async response writer.
    ///
    /// # Errors
    /// Returns [`WsError::Io`] when the connection fails or times out.
    async fn send_frame(&self, frame: &WebSocketFrame) -> Result<(), WsError> {
        let mut writer = ResponseWriter::new(
            self.writer_conn.clone(),
            self.config.writer,
            self.ctx.clone(),
        );
        writer.write_all(&frame.encode()).await?;
        writer.flush().await?;
        Ok(())
    }

    /// Send one text frame with a JSON reply.
    ///
    /// # Errors
    /// Returns [`WsError`] when the reply is too large or the connection fails.
    async fn send_reply(&self, reply: &WsReply) -> Result<(), WsError> {
        let text = reply.to_text()?;
        if text.len() > self.config.max_frame_bytes {
            return Err(WsError::FrameTooLarge {
                size: text.len(),
                cap: self.config.max_frame_bytes,
            });
        }
        self.send_frame(&WebSocketFrame {
            fin: true,
            opcode: Opcode::Text,
            mask: None,
            payload: text.into_bytes(),
        })
        .await
    }

    /// Send one text frame with raw text.
    ///
    /// # Errors
    /// As [`Self::send_reply`].
    async fn send_text(&self, text: &str) -> Result<(), WsError> {
        if text.len() > self.config.max_frame_bytes {
            return Err(WsError::FrameTooLarge {
                size: text.len(),
                cap: self.config.max_frame_bytes,
            });
        }
        self.send_frame(&WebSocketFrame {
            fin: true,
            opcode: Opcode::Text,
            mask: None,
            payload: text.as_bytes().to_vec(),
        })
        .await
    }

    /// Answer a Ping frame with a Pong.
    ///
    /// # Errors
    /// Returns [`WsError::Io`] when the connection fails.
    async fn send_pong(&self, payload: Vec<u8>) -> Result<(), WsError> {
        self.send_frame(&WebSocketFrame {
            fin: true,
            opcode: Opcode::Pong,
            mask: None,
            payload,
        })
        .await
    }

    /// Send a Close frame.
    ///
    /// # Errors
    /// Returns [`WsError::Io`] when the connection fails.
    async fn send_close(&self, code: u16, reason: &str) -> Result<(), WsError> {
        let mut payload = code.to_be_bytes().to_vec();
        payload.extend_from_slice(reason.as_bytes());
        self.send_frame(&WebSocketFrame {
            fin: true,
            opcode: Opcode::Close,
            mask: None,
            payload,
        })
        .await
    }

    /// Report a failure to the peer, then let the caller close.
    ///
    /// # Errors
    /// Returns the send error, if there is one to report.
    async fn fail(&self, error: &WsError) -> Result<(), WsError> {
        match error {
            // A dead connection takes no frame.
            WsError::Io(_) => Ok(()),
            WsError::Protocol(message) if message.contains("exceeds the configured limit") => {
                self.send_close(1009, "frame exceeds the configured limit")
                    .await
            }
            WsError::Protocol(message) => self.send_close(1002, close_reason(message)).await,
            WsError::Store(store) => {
                self.send_close(1011, close_reason(&store.to_string()))
                    .await
            }
            WsError::BadRequest(message) => self.send_close(1008, close_reason(message)).await,
            WsError::FrameTooLarge { .. } => {
                self.send_close(1009, "frame exceeds the configured limit")
                    .await
            }
        }
    }
}

/// Shorten a message to the 123 bytes a Close frame has for its reason.
fn close_reason(message: &str) -> &str {
    let mut end = message.len().min(123);
    while end > 0 && !message.is_char_boundary(end) {
        end -= 1;
    }
    &message[..end]
}

/// Serve the protocol on one upgraded connection until it closes.
async fn serve_connection(
    service: Arc<DurableService<DurableStream>>,
    config: DurableWsConfig,
    ctx: Ctx,
    conn: SharedByteBufferStream<RawStream>,
) -> ServeOutcome {
    let mut io = WsIo::new(conn, config, ctx);
    let mut live: Option<LiveRead> = None;

    loop {
        // Handle every frame that is already available.
        loop {
            match io.try_next_frame() {
                Ok(TryFrame::Frame(frame)) => {
                    match handle_frame(&service, &io, &mut live, frame).await {
                        Ok(FrameFlow::Continue) => {}
                        Ok(FrameFlow::Close) => {
                            let _ = io.send_close(1000, "bye").await;
                            return ServeOutcome::Close(None);
                        }
                        Err(error) => {
                            let _ = io.fail(&error).await;
                            return ServeOutcome::Close(None);
                        }
                    }
                }
                Ok(TryFrame::Empty) => break,
                Ok(TryFrame::Eof) => return ServeOutcome::Close(None),
                Err(error) => {
                    let _ = io.fail(&error).await;
                    return ServeOutcome::Close(None);
                }
            }
        }

        // Register the wake before the page read, so an append in between is
        // not lost.
        let signal = live
            .as_ref()
            .and_then(|active| append_wait(&service, &active.name));

        if let Some(active) = &mut live {
            match push_live_pages(&service, &io, active).await {
                Ok(LiveStep::Idle) => {}
                Ok(LiveStep::Closed) => live = None,
                Err(error) => {
                    let _ = io.fail(&error).await;
                    return ServeOutcome::Close(None);
                }
            }
        }

        let result = if live.is_some() {
            io.wait_live(signal).await
        } else {
            io.wait_readable().await
        };
        if let Err(error) = result {
            let _ = io.fail(&error).await;
            return ServeOutcome::Close(None);
        }
    }
}

/// Handle one complete inbound frame.
async fn handle_frame(
    service: &DurableService<DurableStream>,
    io: &WsIo,
    live: &mut Option<LiveRead>,
    frame: WebSocketFrame,
) -> Result<FrameFlow, WsError> {
    // RFC 6455 section 5.3: a client frame is always masked. An unmasked frame
    // is a protocol violation, and the connection closes with code 1002.
    if frame.mask.is_none() {
        return Err(WsError::Protocol(
            "a client frame must be masked".to_string(),
        ));
    }
    match frame.opcode {
        Opcode::Ping => {
            io.send_pong(frame.payload).await?;
            Ok(FrameFlow::Continue)
        }
        Opcode::Pong => Ok(FrameFlow::Continue),
        Opcode::Close => Ok(FrameFlow::Close),
        Opcode::Binary => Err(WsError::Protocol(
            "binary frames are not supported; send a JSON text frame".to_string(),
        )),
        Opcode::Continuation => Err(WsError::Protocol(
            "unexpected continuation frame".to_string(),
        )),
        Opcode::Text => {
            if !frame.fin {
                return Err(WsError::Protocol(
                    "a fragmented text message is not supported".to_string(),
                ));
            }
            let text = String::from_utf8(frame.payload).map_err(|_| {
                WsError::Protocol("a text frame must carry valid UTF-8".to_string())
            })?;
            let request = match WsRequest::parse(&text) {
                Ok(request) => request,
                Err(error) => {
                    io.send_text(
                        &WsReply::transport_error(0, "bad_request", error.to_string()).to_text()?,
                    )
                    .await?;
                    return Ok(FrameFlow::Continue);
                }
            };
            handle_request(service, io, live, request).await
        }
    }
}

/// Handle one decoded request.
async fn handle_request(
    service: &DurableService<DurableStream>,
    io: &WsIo,
    live: &mut Option<LiveRead>,
    request: WsRequest,
) -> Result<FrameFlow, WsError> {
    let id = request.id;

    match request.op {
        WsOp::Cancel { read } => {
            let cancelled = matches!(live, Some(active) if active.id == read);
            if cancelled {
                *live = None;
                io.send_reply(&WsReply::end_cancelled(read)).await?;
            } else {
                io.send_reply(&WsReply::transport_error(
                    id,
                    "bad_request",
                    "no open live read matches this cancel request",
                ))
                .await?;
            }
            Ok(FrameFlow::Continue)
        }
        WsOp::Read {
            stream,
            offset,
            live: true,
            max_bytes,
            max_chunks,
        } => {
            if live.is_some() {
                io.send_reply(&WsReply::transport_error(
                    id,
                    "bad_request",
                    "a live read is already open on this connection",
                ))
                .await?;
                return Ok(FrameFlow::Continue);
            }
            match start_live(
                service,
                id,
                &stream,
                offset.as_deref(),
                max_bytes,
                max_chunks,
            )
            .await
            {
                Ok(active) => *live = Some(active),
                Err(error) => io.send_reply(&WsReply::error(id, &error)).await?,
            }
            Ok(FrameFlow::Continue)
        }
        op => {
            match run_op(service, id, op).await {
                Ok(reply) => io.send_reply(&reply).await?,
                Err(error) => io.send_reply(&WsReply::error(id, &error)).await?,
            }
            Ok(FrameFlow::Continue)
        }
    }
}

/// Run one operation that answers with a single reply.
// One match over every operation keeps the request-to-core mapping in one place.
#[allow(clippy::too_many_lines)]
async fn run_op(
    service: &DurableService<DurableStream>,
    id: u64,
    op: WsOp,
) -> Result<WsReply, StoreError> {
    match op {
        WsOp::Create {
            stream,
            content_type,
            ttl_ms,
            expires_at_ms,
            closed,
            initial,
        } => {
            let name = StreamName::parse(&stream)?;
            let expires = Expiry::from_headers(ttl_ms.map(Duration::from_millis), expires_at_ms)?;
            let content_type = content_type
                .map(|value| ContentType::parse(&value))
                .transpose()?;
            let initial = match initial {
                Some(text) if !text.is_empty() => Some(Chunk::new(decode(&text)?)),
                _ => None,
            };
            let outcome = service
                .create(
                    name,
                    CreateSpec {
                        content_type,
                        expires,
                        closed,
                        initial,
                        fork: None,
                    },
                )
                .await?;
            Ok(WsReply::create("create", id, &outcome))
        }
        WsOp::Append {
            stream,
            data,
            content_type,
            stream_seq,
            close,
        } => {
            let name = StreamName::parse(&stream)?;
            let bytes = match data {
                Some(text) if !text.is_empty() => decode(&text)?,
                _ => Vec::new(),
            };
            if bytes.is_empty() {
                if !close {
                    return Err(StoreError::BadRequest(BadRequestKind::Body));
                }
                let outcome = service
                    .close(
                        &name,
                        CloseRequest {
                            final_chunk: None,
                            producer: None,
                        },
                    )
                    .await?;
                return Ok(WsReply::append("append", id, &outcome));
            }
            let Some(content_type) = content_type else {
                return Err(StoreError::BadRequest(BadRequestKind::Body));
            };
            let content_type = ContentType::parse(&content_type)?;
            let stream_seq = stream_seq.map(|value| StreamSeq::from_bytes(value.into_bytes()));
            let outcome = service
                .append(
                    &name,
                    AppendRequest {
                        chunk: Chunk::new(bytes),
                        content_type: Some(content_type),
                        stream_seq,
                        producer: None,
                        close,
                    },
                )
                .await?;
            Ok(WsReply::append("append", id, &outcome))
        }
        WsOp::Close { stream, data } => {
            let name = StreamName::parse(&stream)?;
            let bytes = match data {
                Some(text) if !text.is_empty() => decode(&text)?,
                _ => Vec::new(),
            };
            let final_chunk = (!bytes.is_empty()).then(|| Chunk::new(bytes));
            let outcome = service
                .close(
                    &name,
                    CloseRequest {
                        final_chunk,
                        producer: None,
                    },
                )
                .await?;
            Ok(WsReply::append("close", id, &outcome))
        }
        WsOp::Head { stream } => {
            let name = StreamName::parse(&stream)?;
            let head = service.head(&name).await?;
            Ok(WsReply::head(id, &head))
        }
        WsOp::Delete { stream } => {
            let name = StreamName::parse(&stream)?;
            let outcome = service.delete(&name).await?;
            Ok(WsReply::delete(id, outcome))
        }
        WsOp::Fork {
            stream,
            from,
            offset,
            sub_offset,
        } => {
            let name = StreamName::parse(&stream)?;
            let source = StreamName::parse(&from)?;
            let at = offset.map(|value| decode_offset(&value)).transpose()?;
            let outcome = service
                .fork(
                    name,
                    ForkSpec {
                        source,
                        at,
                        sub_offset,
                    },
                )
                .await?;
            Ok(WsReply::create("fork", id, &outcome))
        }
        WsOp::Read {
            stream,
            offset,
            live: false,
            max_bytes,
            max_chunks,
        } => {
            let name = StreamName::parse(&stream)?;
            let from = read_from(offset.as_deref())?;
            let page = service
                .read(
                    &name,
                    ReadRequest {
                        from,
                        limit: ReadLimit::new(max_bytes, max_chunks),
                    },
                )
                .await?;
            Ok(WsReply::page("read", id, &page))
        }
        // A live read is started by `handle_request`; it answers with frames,
        // not one reply. A cancel is handled there too.
        WsOp::Read { live: true, .. } | WsOp::Cancel { .. } => Err(StoreError::Unsupported),
    }
}

/// Parse the read start of a request.
fn read_from(offset: Option<&str>) -> Result<ReadFrom, StoreError> {
    match offset {
        None => Ok(ReadFrom::Start),
        Some(value) => parse_offset(value),
    }
}

/// Open one live read.
///
/// # Errors
/// Returns [`StoreError`] for a bad name, offset or a missing stream.
async fn start_live(
    service: &DurableService<DurableStream>,
    id: u64,
    stream: &str,
    offset: Option<&str>,
    max_bytes: Option<usize>,
    max_chunks: Option<usize>,
) -> Result<LiveRead, StoreError> {
    let name = StreamName::parse(stream)?;
    let next = match read_from(offset)? {
        // `now` is resolved once, so the read does not jump ahead on each page.
        ReadFrom::Tail => ReadFrom::Offset(service.head(&name).await?.tail),
        other => other,
    };
    Ok(LiveRead {
        id,
        name,
        next,
        limit: ReadLimit::new(max_bytes, max_chunks),
        first: true,
    })
}

/// Push every page that is already available for one live read.
///
/// # Errors
/// Returns [`WsError`] when the connection fails. A store refusal answers an
/// error frame and ends the read, so it is not an error here.
async fn push_live_pages(
    service: &DurableService<DurableStream>,
    io: &WsIo,
    live: &mut LiveRead,
) -> Result<LiveStep, WsError> {
    loop {
        let page = match service
            .read(
                &live.name,
                ReadRequest {
                    from: live.next.clone(),
                    limit: live.limit,
                },
            )
            .await
        {
            Ok(page) => page,
            Err(error) => {
                io.send_reply(&WsReply::error(live.id, &error)).await?;
                return Ok(LiveStep::Closed);
            }
        };

        let has_chunks = !page.chunks.is_empty();
        if has_chunks || live.first {
            let op = if live.first { "read" } else { "data" };
            match io.send_reply(&WsReply::page(op, live.id, &page)).await {
                Ok(()) => {}
                Err(WsError::FrameTooLarge { size, cap }) => {
                    io.send_reply(&WsReply::transport_error(
                        live.id,
                        "frame_too_large",
                        format!(
                            "the page reply needs {size} bytes and the frame cap is {cap}; \
                             send a read with a smaller max_bytes"
                        ),
                    ))
                    .await?;
                    return Ok(LiveStep::Closed);
                }
                Err(error) => return Err(error),
            }
            live.first = false;
        }

        live.next = ReadFrom::Offset(page.cursor.next.clone());
        if page.cursor.closed {
            io.send_reply(&WsReply::end_closed(live.id)).await?;
            return Ok(LiveStep::Closed);
        }
        if !has_chunks {
            return Ok(LiveStep::Idle);
        }
    }
}

/// A one-shot wake when a stream appends or closes.
///
/// The Hypertext Transfer Protocol (HTTP) adapter uses the same one-shot wait
/// for its long poll and Server-Sent Events loop.
struct AppendWait {
    fired: Arc<AtomicBool>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl std::future::Future for AppendWait {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<()> {
        if self.fired.load(Ordering::SeqCst) {
            return Poll::Ready(());
        }
        *self
            .waker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(cx.waker().clone());
        if self.fired.load(Ordering::SeqCst) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

/// Register a wake on the stream notifier.
///
/// Returns `None` when the store has no notifier, so the caller parks on the
/// read readiness and the fallback timer instead.
fn append_wait(service: &DurableService<DurableStream>, name: &StreamName) -> Option<AppendWait> {
    let fired = Arc::new(AtomicBool::new(false));
    let waker: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));
    let notifier = service.notifier(name)?;
    let fired_cb = fired.clone();
    let slot = waker.clone();
    notifier.register(Box::new(move || {
        fired_cb.store(true, Ordering::SeqCst);
        if let Some(waker) = slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            waker.wake();
        }
    }));
    Some(AppendWait { fired, waker })
}
