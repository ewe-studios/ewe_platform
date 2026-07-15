//! The gRPC protocol (Decision 05 §Protocol 2) — strict HTTP/2 framing.
//!
//! WHY: gRPC is the most widely deployed RPC wire format and requires HTTP/2:
//! status always 200, errors in `grpc-status` trailing headers, `grpc-timeout`
//! deadline propagation, and all request/response bodies use envelope framing
//! (even unary: a single envelope). Completes the three-protocol matrix
//! (Connect / gRPC-Web / gRPC).
//!
//! WHAT: [`GrpcHandler`] (server) and [`GrpcClient`] — content-type
//! `application/grpc+{codec}`, gRPC timeout encode/decode, status-trailer
//! build/parse, and the reader/writer tasks bridging transport byte pipes and
//! the seam's `FramePipe`s.
//!
//! HOW: Reuses the gRPC-Web infrastructure for envelope framing, compression,
//! and status trailers — the difference is a simpler wire model: no text mode,
//! no in-body trailer frame (real HTTP/2 trailing HEADERS instead), and the
//! `application/grpc+` content-type prefix. The capability floor enforces
//! `h2_trailers` (Decision 11).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::{Bytes, BytesMut};
use foundation_core::valtron::{PipeReceiver, PipeSender};
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::{
    SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};
use futures::StreamExt;

use crate::shared::client::code_from_http_status;
use crate::shared::compression::{negotiate_compression, CompressionRegistry, Compressor};
use crate::shared::context::{CancelSignal, Peer, Spec, StreamType};
use crate::shared::error::{Code, ConnectError, ConnectResult};
use crate::shared::envelope::{Envelope, EnvelopeWriter, ENVELOPE_HEADER_LEN};
use crate::shared::transport::{
    body_stream_from_pipe, BodyStream, ByteSink, ByteSource, Frame, HeadStream, PipeClientConn,
    PipeHandlerConn, SendBody, TransportStream, DEFAULT_PIPE_DEPTH,
};

use super::grpc_web::build_status_trailers;

use super::{
    BoxedTask, ClientExchange, HandlerExchange, ProtocolClient, ProtocolHandler, UnaryOutcome,
};

// ── gRPC constants ──────────────────────────────────────────────────────────

/// gRPC protocol-specific constants (Decision 05 §Protocol 2).
pub mod constants {
    /// `grpc-timeout` header.
    pub const HEADER_TIMEOUT: &str = "grpc-timeout";
    /// `grpc-encoding` header.
    pub const HEADER_ENCODING: &str = "grpc-encoding";
    /// `grpc-accept-encoding` header.
    pub const HEADER_ACCEPT_ENCODING: &str = "grpc-accept-encoding";
    /// gRPC binary content-type prefix (`application/grpc+`).
    pub const CONTENT_TYPE_PREFIX: &str = "application/grpc";
    /// The `Te` header value gRPC clients send.
    pub const TE_TRAILERS: &str = "trailers";
}

// ── Content-Type ────────────────────────────────────────────────────────────

/// The gRPC content-type: `application/grpc+{codec}`.
#[must_use]
pub fn content_type(codec: &str) -> String {
    format!("{}+{codec}", constants::CONTENT_TYPE_PREFIX)
}

/// Parse a gRPC content-type into `(codec)`, or `None` if not gRPC.
///
/// `application/grpc-web+proto` is **not** gRPC: the match must end at the
/// subtype boundary, or this handler claims every gRPC-Web request before the
/// gRPC-Web handler is ever offered it. See [`super::codec_for_content_type`].
#[must_use]
pub fn parse_content_type(content_type: &str) -> Option<String> {
    let canonical = super::canonicalize_content_type(content_type);
    super::codec_for_content_type(&canonical, constants::CONTENT_TYPE_PREFIX)
        .map(ToString::to_string)
}

// ── reader / writer tasks ───────────────────────────────────────────────────

async fn read_frames(
    mut body: BodyStream,
    tx: PipeSender<Frame>,
    decompressor: Option<Arc<dyn Compressor>>,
    read_max: usize,
) -> ConnectResult<()> {
    let mut buf = BytesMut::new();
    loop {
        while buf.len() >= ENVELOPE_HEADER_LEN {
            let len = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
            if read_max != 0 && len > read_max {
                return Err(ConnectError::resource_exhausted(format!(
                    "envelope length {len} exceeds read_max_bytes {read_max}"
                ))
                .into());
            }
            if buf.len() < ENVELOPE_HEADER_LEN + len {
                break;
            }
            let flags = buf[0];
            let frame = buf.split_to(ENVELOPE_HEADER_LEN + len).freeze();
            let payload = frame.slice(ENVELOPE_HEADER_LEN..);
            let data = decompress_if(flags, payload, &decompressor, read_max)?;
            if tx.send(Frame::Message(data)).await.is_err() {
                return Ok(());
            }
        }
        match body.next().await {
            Some(Ok(chunk)) => buf.extend_from_slice(&chunk),
            // Return WITHOUT closing `tx` in either terminal case: the gRPC
            // client reader holds a clone and appends a terminal
            // `Frame::EndStream` (trailers / error) after this returns —
            // closing here would swallow it. Callers that moved `tx` in close
            // the pipe implicitly when their task ends and drops it.
            Some(Err(err)) => return Err(err.into()),
            None => return Ok(()),
        }
    }
}

async fn write_frames(
    resp_rx: PipeReceiver<Frame>,
    sink: Arc<dyn SendBody>,
    writer: EnvelopeWriter,
) -> ConnectResult<()> {
    loop {
        match resp_rx.receive().await {
            Some(Frame::Message(frame)) => {
                let enveloped = writer.write(frame)?;
                if !enveloped.is_empty() && sink.send_async(Bytes::from(enveloped)).await.is_err() {
                    return Ok(());
                }
            }
            Some(Frame::EndStream { error: _, trailers: _ }) => {
                sink.close();
                return Ok(());
            }
            None => {
                sink.close();
                return Ok(());
            }
        }
    }
}

fn decompress_if(
    flags: u8,
    payload: Bytes,
    decompressor: &Option<Arc<dyn Compressor>>,
    read_max: usize,
) -> ConnectResult<Bytes> {
    if flags & Envelope::FLAG_COMPRESSED != 0 {
        match decompressor {
            Some(c) => Ok(Bytes::from(c.decompress(&payload, read_max)?)),
            None => Err(ConnectError::internal(
                "compressed frame received but no decompressor was negotiated",
            )
            .into()),
        }
    } else {
        Ok(payload)
    }
}

// ── handler ─────────────────────────────────────────────────────────────────

/// The gRPC protocol handler (server).
///
/// Status is always HTTP 200; errors ride `grpc-status` trailing headers.
/// All message bodies are envelope-framed including unary (single envelope).
#[derive(Debug, Default)]
pub struct GrpcHandler;

const ALLOWED_METHODS: [SimpleMethod; 1] = [SimpleMethod::POST];

impl ProtocolHandler for GrpcHandler {
    fn kind(&self) -> crate::shared::transport::ProtocolKind {
        crate::shared::transport::ProtocolKind::Grpc
    }
    fn allowed_methods(&self) -> &[SimpleMethod] {
        &ALLOWED_METHODS
    }
    fn content_types(&self) -> Vec<String> {
        vec![content_type("proto"), content_type("json")]
    }
    fn parse_timeout(&self, headers: &SimpleHeaders) -> Option<Duration> {
        let v = headers
            .get(&SimpleHeader::from(constants::HEADER_TIMEOUT.to_string()))?
            .first()?;
        crate::shared::envelope::decode_grpc_timeout(v.trim()).ok()
    }
    fn can_handle(&self, request: &SimpleIncomingRequest) -> bool {
        request
            .headers
            .get(&SimpleHeader::CONTENT_TYPE)
            .and_then(|v| v.first())
            .and_then(|ct| parse_content_type(ct))
            .is_some()
    }
    fn codec_name(&self, request: &SimpleIncomingRequest) -> Option<String> {
        request
            .headers
            .get(&SimpleHeader::CONTENT_TYPE)
            .and_then(|v| v.first())
            .and_then(|ct| parse_content_type(ct))
    }

    fn new_conn(
        &self,
        request: &SimpleIncomingRequest,
        spec: Spec,
        body: ByteSource,
        responder: ByteSink,
        compression: &CompressionRegistry,
    ) -> ConnectResult<HandlerExchange> {
        let negotiated = negotiate_compression(
            compression,
            header(&request.headers, constants::HEADER_ENCODING).as_deref(),
            header(&request.headers, constants::HEADER_ACCEPT_ENCODING).as_deref(),
        )?;

        let peer = Peer {
            addr: request
                .connection
                .peer_addr
                .as_ref()
                .map(|a| format!("{a:?}"))
                .unwrap_or_default(),
            protocol: "grpc".to_string(),
        };

        let (conn, ends) = PipeHandlerConn::new(
            spec,
            peer,
            request.headers.clone(),
            CancelSignal::new(),
            DEFAULT_PIPE_DEPTH,
        );

        let reader: BoxedTask = Box::pin(read_frames(
            body_stream_from_pipe(body),
            ends.request_tx,
            negotiated.request_decompressor,
            0,
        ));
        // The compressed envelope flag is only valid when the response HEADERS
        // advertise the encoding — dispatch fetches it via
        // `streaming_response_encoding` (same negotiation as here) and sends
        // `grpc-encoding` alongside the content-type. Without that header,
        // grpc-go kills the call with "compressed flag set with identity or
        // empty encoding" (this broke BuildKit FileSync).
        let writer: BoxedTask = Box::pin(write_frames(
            ends.response_rx,
            Arc::new(responder),
            EnvelopeWriter::new(negotiated.response_compressor, 0, 0),
        ));

        Ok(HandlerExchange {
            conn: Box::new(conn),
            reader_task: reader,
            writer_task: writer,
        })
    }

    fn streaming_response_content_type(
        &self,
        _request: &SimpleIncomingRequest,
        codec_name: &str,
    ) -> String {
        content_type(codec_name)
    }

    fn streaming_response_encoding(
        &self,
        request: &SimpleIncomingRequest,
        compression: &CompressionRegistry,
    ) -> Option<(String, String)> {
        let negotiated = negotiate_compression(
            compression,
            header(&request.headers, constants::HEADER_ENCODING).as_deref(),
            header(&request.headers, constants::HEADER_ACCEPT_ENCODING).as_deref(),
        )
        .ok()?;
        negotiated
            .response_compressor
            .map(|c| (constants::HEADER_ENCODING.to_string(), c.name().to_string()))
    }

    fn decode_unary_request(
        &self,
        request: &SimpleIncomingRequest,
        body: Bytes,
        compression: &CompressionRegistry,
    ) -> ConnectResult<Bytes> {
        // gRPC unary: body is a single envelope frame.
        if body.is_empty() {
            return Ok(Bytes::new());
        }
        if body.len() < ENVELOPE_HEADER_LEN {
            return Err(ConnectError::invalid_argument("truncated gRPC frame").into());
        }
        let flags = body[0];
        let len = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
        let end = (ENVELOPE_HEADER_LEN + len).min(body.len());
        let payload = body.slice(ENVELOPE_HEADER_LEN..end);
        if flags & Envelope::FLAG_COMPRESSED != 0 {
            let name = header(&request.headers, constants::HEADER_ENCODING).unwrap_or_default();
            let c = compression.get(&name).ok_or_else(|| {
                ConnectError::unimplemented(format!("unsupported grpc-encoding {name:?}"))
            })?;
            Ok(Bytes::from(c.decompress(&payload, 0)?))
        } else {
            Ok(payload)
        }
    }

    fn encode_unary_response(
        &self,
        response: &mut SimpleOutgoingResponse,
        request: &SimpleIncomingRequest,
        codec_name: &str,
        outcome: UnaryOutcome,
        compression: &CompressionRegistry,
    ) -> ConnectResult<()> {
        // gRPC status is always 200; errors land in trailers.
        response.status = Status::OK;

        let negotiated = negotiate_compression(
            compression,
            header(&request.headers, constants::HEADER_ENCODING).as_deref(),
            header(&request.headers, constants::HEADER_ACCEPT_ENCODING).as_deref(),
        )?;
        let response_encoding = negotiated
            .response_compressor
            .as_ref()
            .map(|c| c.name().to_string());
        let writer = EnvelopeWriter::new(negotiated.response_compressor, 0, 0);

        // Single message envelope.
        let out = writer.write(outcome.frame)?;

        // gRPC trailers carry grpc-status (success: 0, error: code + message).
        let status_trailers = build_status_trailers(None, &outcome.trailers);

        let mut headers = outcome.headers;
        headers.insert(SimpleHeader::CONTENT_TYPE, vec![content_type(codec_name)]);
        if let Some(enc) = response_encoding {
            headers.insert(
                SimpleHeader::from(constants::HEADER_ENCODING.to_string()),
                vec![enc],
            );
        }
        response.headers = headers;
        response.trailers = status_trailers;
        response.body = Some(SendSafeBody::Bytes(out));
        Ok(())
    }
}

// ── client ──────────────────────────────────────────────────────────────────

/// The gRPC protocol client.
///
/// Always sends `Te: trailers` and returns status from the response's trailing
/// headers (real HTTP/2 trailing HEADERS, not in-body frames).
#[derive(Debug, Default)]
pub struct GrpcClient;

impl ProtocolClient for GrpcClient {
    fn write_request_headers(
        &self,
        _stream_type: StreamType,
        headers: &mut SimpleHeaders,
        codec_name: &str,
        compression: Option<&str>,
    ) {
        headers.insert(
            SimpleHeader::CONTENT_TYPE,
            vec![content_type(codec_name)],
        );
        headers.insert(
            SimpleHeader::from("te".to_string()),
            vec![constants::TE_TRAILERS.to_string()],
        );
        if let Some(algo) = compression {
            headers.insert(
                SimpleHeader::from(constants::HEADER_ENCODING.to_string()),
                vec![algo.to_string()],
            );
            headers.insert(
                SimpleHeader::from(constants::HEADER_ACCEPT_ENCODING.to_string()),
                vec![algo.to_string()],
            );
        }
    }

    fn encode_unary_request(&self, body: &[u8], is_compressed: bool) -> Bytes {
        // gRPC unary: single envelope frame. flags bit 0x01 marks compression;
        // the payload is the (optionally compressed) marshaled message bytes.
        let flags: u8 = if is_compressed {
            Envelope::FLAG_COMPRESSED
        } else {
            0x00
        };
        let len = body.len() as u32;
        let mut envelope = Vec::with_capacity(ENVELOPE_HEADER_LEN + body.len());
        envelope.push(flags);
        envelope.extend_from_slice(&len.to_be_bytes());
        envelope.extend_from_slice(body);
        Bytes::from(envelope)
    }

    fn decode_unary_response(&self, body: Bytes) -> ConnectResult<Bytes> {
        // gRPC unary response: single envelope frame. Strip the 5-byte header,
        // return the payload. Compression is negotiated via grpc-encoding; if a
        // response arrives compressed, the caller must decompress separately
        // (or thread the CompressionRegistry through — the streaming path
        // handles this in the reader task).
        if body.is_empty() {
            return Ok(Bytes::new());
        }
        if body.len() < ENVELOPE_HEADER_LEN {
            return Err(ConnectError::invalid_argument("truncated gRPC response frame").into());
        }
        let flags = body[0];
        let len = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
        let end = (ENVELOPE_HEADER_LEN + len).min(body.len());
        let payload = body.slice(ENVELOPE_HEADER_LEN..end);
        if flags & Envelope::FLAG_COMPRESSED != 0 {
            return Err(ConnectError::internal(
                "compressed gRPC unary response — decode_unary_response does not yet thread the \
                 CompressionRegistry (streaming handles this in the reader task); \
                 set response_compression: None on the server or add registry threading here",
            )
            .into());
        }
        Ok(payload)
    }

    fn new_conn(
        &self,
        spec: &Spec,
        headers: SimpleHeaders,
        stream: TransportStream,
        cancel: CancelSignal,
    ) -> ConnectResult<ClientExchange> {
        let (conn, ends) =
            PipeClientConn::new(spec.clone(), headers, cancel, DEFAULT_PIPE_DEPTH);

        let writer: BoxedTask = Box::pin(write_frames(
            ends.request_rx,
            Arc::clone(&stream.send_body),
            EnvelopeWriter::new(None, 0, 0),
        ));
        let reader: BoxedTask = Box::pin(read_grpc_response(
            stream.head,
            stream.recv_body,
            stream.trailers,
            ends.response_tx,
            ends.response_headers,
        ));

        Ok(ClientExchange {
            conn: Box::new(conn),
            reader_task: reader,
            writer_task: writer,
        })
    }
}

/// Client-side gRPC response reader: response head → envelopes → status trailers.
///
/// gRPC delivers its outcome in trailing metadata, in one of two shapes:
/// - **Trailers-Only**: a single HEADERS frame (END_STREAM) carrying
///   `grpc-status` alongside the response headers — used for errors that occur
///   before any message is sent.
/// - **Normal**: response HEADERS, DATA envelopes, then trailing HEADERS with
///   `grpc-status`.
///
/// Both MUST be surfaced as a terminal [`Frame::EndStream`] — closing the pipe
/// on body EOF (the old behavior) silently converted every server error into a
/// clean EOF, which for unary calls decoded as `Ok(Default::default())`.
async fn read_grpc_response(
    mut head: HeadStream,
    body: BodyStream,
    trailers: PipeReceiver<SimpleHeaders>,
    tx: PipeSender<Frame>,
    response_headers: Arc<Mutex<Option<SimpleHeaders>>>,
) -> ConnectResult<()> {
    // 1. Response head. Publish the headers so `response_headers()` works.
    // Errors are delivered through the pipe (that's what the caller observes);
    // the task itself still returns Ok — its result is only logged.
    let (status, resp_headers) = match head.next().await {
        Some(Ok(v)) => v,
        Some(Err(e)) => {
            let err: ErrorTrace<ConnectError> = ConnectError::from(e).into();
            let _ = tx
                .send(Frame::EndStream { error: Some(err), trailers: SimpleHeaders::new() })
                .await;
            tx.close();
            return Ok(());
        }
        None => {
            let err: ErrorTrace<ConnectError> =
                ConnectError::unavailable("transport closed without response head").into();
            let _ = tx
                .send(Frame::EndStream { error: Some(err), trailers: SimpleHeaders::new() })
                .await;
            tx.close();
            return Ok(());
        }
    };
    *response_headers.lock().unwrap_or_else(|e| e.into_inner()) = Some(resp_headers.clone());

    if status != Status::OK {
        let err: ErrorTrace<ConnectError> =
            ConnectError::new(code_from_http_status(&status), format!("HTTP {status}")).into();
        let _ = tx
            .send(Frame::EndStream { error: Some(err), trailers: resp_headers })
            .await;
        tx.close();
        return Ok(());
    }

    // 2. Trailers-Only: grpc-status riding the response headers.
    if grpc_status_of(&resp_headers).is_some() {
        let error = parse_grpc_error_trailer(&resp_headers);
        let _ = tx
            .send(Frame::EndStream { error, trailers: resp_headers })
            .await;
        tx.close();
        return Ok(());
    }

    // 3. Message envelopes until body EOF. A mid-stream failure (truncated
    //    envelope, body error, oversized frame) must reach the receiver as a
    //    terminal error, not vanish into a clean-looking EOF.
    if let Err(e) = read_frames(body, tx.clone(), None, 0).await {
        let _ = tx
            .send(Frame::EndStream { error: Some(e), trailers: SimpleHeaders::new() })
            .await;
        tx.close();
        return Ok(());
    }

    // 4. Trailing HEADERS carry the call's outcome.
    let trls = trailers.receive().await.unwrap_or_default();
    let error = parse_grpc_error_trailer(&trls);
    let _ = tx.send(Frame::EndStream { error, trailers: trls }).await;
    tx.close();
    Ok(())
}

/// The raw `grpc-status` value, if present.
fn grpc_status_of(headers: &SimpleHeaders) -> Option<u32> {
    headers
        .get(&SimpleHeader::from("grpc-status".to_string()))
        .and_then(|v| v.first())
        .and_then(|s| s.trim().parse().ok())
}

/// Parse `grpc-status`/`grpc-message` metadata into an error (`None` when the
/// status is 0/absent). The message is percent-decoded per the gRPC spec.
pub(crate) fn parse_grpc_error_trailer(
    trailers: &SimpleHeaders,
) -> Option<ErrorTrace<ConnectError>> {
    let status = grpc_status_of(trailers)?;
    if status == 0 {
        return None;
    }
    let message = trailers
        .get(&SimpleHeader::from("grpc-message".to_string()))
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();
    let decoded = percent_decode(&message);
    // SAFETY: Code is #[repr(u32)] and its discriminants match gRPC status
    // codes exactly (Decision 03). Out-of-range values are clamped to Unknown
    // first, so the transmute stays within the enum's variants.
    let status = if (1..=16).contains(&status) { status } else { 2 };
    let code: Code = unsafe { std::mem::transmute(status) };
    Some(ErrorTrace::new(ConnectError::new(code, decoded)))
}

/// Percent-decode a `grpc-message` value (spec: space and non-ASCII are
/// %HH-escaped).
fn percent_decode(message: &str) -> String {
    let mut out = String::new();
    let bytes = message.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(&String::from_utf8_lossy(&bytes[i + 1..i + 3]), 16)
            {
                out.push(hex as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn header(headers: &SimpleHeaders, name: &str) -> Option<String> {
    headers
        .get(&SimpleHeader::from(name.to_string()))
        .and_then(|v| v.first())
        .cloned()
}

// ── tests ───────────────────────────────────────────────────────────────────

