//! The Connect protocol (Decision 05 §Protocol 1) — the flagship, on HTTP/1.1
//! from day one.
//!
//! WHY: Connect is browser/curl-friendly: unary is a plain HTTP request/response
//! (bare body; errors are an HTTP status + JSON), and streaming uses the shared
//! envelope framing with an `EndStreamResponse` terminator (flags `0x02`).
//!
//! WHAT: the protocol constants, content-type + GET-query wire helpers (byte
//! conformant), the streaming reader/writer tasks bridging the transport byte
//! pipes and the seam `FramePipe`s, and the [`ConnectHandler`]/[`ConnectClient`]
//! implementations of the protocol traits.
//!
//! HOW: streaming enveloping reuses [`EnvelopeWriter`]/[`Envelope`] (F15); the
//! terminator normalizes to/from [`Frame::EndStream`]. Unary wire helpers are
//! pure functions used by the dispatcher (F22) / client core (F24).

use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use bytes::{Bytes, BytesMut};
use futures::StreamExt;
use foundation_core::valtron::{PipeReceiver, PipeSender};
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::{
    SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};

use crate::shared::compression::{negotiate_compression, CompressionRegistry, Compressor};
use crate::shared::context::{CancelSignal, Peer, Spec, StreamType};
use crate::shared::error::{Code, ConnectError, ConnectResult, EndStreamResponse, WireError};
use crate::shared::envelope::{Envelope, EnvelopeWriter, ENVELOPE_HEADER_LEN};
use crate::shared::transport::{
    fail_stream, race, BodyStream, ByteSink, ByteSource, Frame, HeadStream, PipeClientConn,
    PipeHandlerConn, SendBody, TransportStream, DEFAULT_PIPE_DEPTH,
};

use super::{
    open_response_stream, BoxedTask, ClientExchange, ClientExchangeConfig, ExchangeConfig,
    HandlerExchange, ProtocolClient, ProtocolHandler, UnaryOutcome,
};
use std::sync::Mutex;

/// Connect protocol constants (Decision 05).
pub mod constants {
    /// `connect-protocol-version` header.
    pub const HEADER_PROTOCOL_VERSION: &str = "connect-protocol-version";
    /// The protocol version value.
    pub const PROTOCOL_VERSION: &str = "1";
    /// `connect-timeout-ms` header.
    pub const HEADER_TIMEOUT: &str = "connect-timeout-ms";
    /// `connect-content-encoding` (streaming) header.
    pub const HEADER_STREAMING_CONTENT_ENCODING: &str = "connect-content-encoding";
    /// `connect-accept-encoding` (streaming) header.
    pub const HEADER_STREAMING_ACCEPT_ENCODING: &str = "connect-accept-encoding";

    /// GET query key: codec name.
    pub const QUERY_ENCODING: &str = "encoding";
    /// GET query key: message payload.
    pub const QUERY_MESSAGE: &str = "message";
    /// GET query key: base64 flag.
    pub const QUERY_BASE64: &str = "base64";
    /// GET query key: compression algorithm.
    pub const QUERY_COMPRESSION: &str = "compression";
    /// GET query key: connect version.
    pub const QUERY_CONNECT_VERSION: &str = "connect";
    /// GET query value for the connect version.
    pub const QUERY_CONNECT_VERSION_VALUE: &str = "v1";

    /// Prefix for unary trailing metadata rendered as headers.
    pub const TRAILER_HEADER_PREFIX: &str = "trailer-";
}

/// The unary Content-Type for a codec (`application/{codec}`).
#[must_use]
pub fn unary_content_type(codec: &str) -> String {
    format!("application/{codec}")
}

/// The streaming Content-Type for a codec (`application/connect+{codec}`).
#[must_use]
pub fn streaming_content_type(codec: &str) -> String {
    format!("application/connect+{codec}")
}

/// Unary errors are always JSON regardless of the request codec.
pub const ERROR_CONTENT_TYPE: &str = "application/json";

/// The HTTP status for a unary error (Decision 05 — `Code → HTTP`).
#[must_use]
pub fn unary_error_status(error: &ErrorTrace<ConnectError>) -> u16 {
    error.current_context().code().http_status()
}

/// The Connect JSON body for a unary error (matches the Connect spec).
///
/// # Errors
/// A serialization failure (never in practice).
pub fn unary_error_body(error: &ErrorTrace<ConnectError>) -> ConnectResult<Vec<u8>> {
    error
        .current_context()
        .to_json()
        .map(String::into_bytes)
        .map_err(|e| ConnectError::internal(format!("error serialization failed: {e}")).into())
}

/// Render trailing metadata as `Trailer-`-prefixed response headers (unary).
#[must_use]
pub fn unary_trailer_headers(trailers: &SimpleHeaders) -> SimpleHeaders {
    let mut out = SimpleHeaders::new();
    for (name, values) in trailers {
        let key = format!(
            "{}{}",
            constants::TRAILER_HEADER_PREFIX,
            name.to_string().to_ascii_lowercase()
        );
        out.insert(SimpleHeader::from(key), values.clone());
    }
    out
}

/// Encode an idempotent unary GET query (Decision 05 §Unary GET / Decision 07).
/// The message is compressed (if a compressor is given) **before** base64url — a
/// binary codec (or any compressed payload) is base64url-encoded with `base64=1`.
///
/// # Errors
/// A compression failure as a trace.
pub fn encode_get_query(
    codec: &str,
    message: &[u8],
    is_binary: bool,
    compressor: Option<&Arc<dyn Compressor>>,
) -> ConnectResult<String> {
    let (payload, compression) = match compressor {
        Some(c) => (c.compress(message)?, Some(c.name().to_string())),
        None => (message.to_vec(), None),
    };
    let use_base64 = is_binary || compression.is_some();
    let message_param = if use_base64 {
        URL_SAFE_NO_PAD.encode(&payload)
    } else {
        percent_encode(&payload)
    };

    let mut query = format!(
        "{}={}&{}={}&{}={}",
        constants::QUERY_CONNECT_VERSION,
        constants::QUERY_CONNECT_VERSION_VALUE,
        constants::QUERY_ENCODING,
        codec,
        constants::QUERY_MESSAGE,
        message_param,
    );
    if use_base64 {
        query.push_str(&format!("&{}=1", constants::QUERY_BASE64));
    }
    if let Some(algo) = compression {
        query.push_str(&format!("&{}={}", constants::QUERY_COMPRESSION, algo));
    }
    Ok(query)
}

/// Whether a GET request URL of `url_len` bytes exceeds the fallback threshold
/// (Decision 07 P14) — the **only** precheck that triggers a GET→POST fallback
/// before sending.
#[must_use]
pub fn get_url_exceeds(url_len: usize, get_url_max_bytes: usize) -> bool {
    get_url_max_bytes != 0 && url_len > get_url_max_bytes
}

/// Minimal percent-encoding for the GET `message` param (text codecs). Encodes
/// everything outside the unreserved set (RFC 3986).
fn percent_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// ── streaming reader/writer tasks ─────────────────────────────────────────────

/// Server/reader: de-envelope + decompress request bytes into request frames.
/// Connect request streams end when the body closes (no request end-stream frame).
/// A failure ends the request stream with the error (see [`fail_stream`]).
async fn read_request_frames(
    body: ByteSource,
    req_tx: PipeSender<Frame>,
    decompressor: Option<Arc<dyn Compressor>>,
    read_max: usize,
    cancel: CancelSignal,
) -> ConnectResult<()> {
    let result = read_request_envelopes(body, &req_tx, decompressor, read_max).await;
    if let Err(e) = &result {
        fail_stream(&req_tx, e, &cancel).await;
    }
    result
}

async fn read_request_envelopes(
    body: ByteSource,
    req_tx: &PipeSender<Frame>,
    decompressor: Option<Arc<dyn Compressor>>,
    read_max: usize,
) -> ConnectResult<()> {
    let mut buf = BytesMut::new();
    loop {
        // Drain complete envelopes from the accumulated buffer (zero-copy payload).
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
            if req_tx.send(Frame::Message(data)).await.is_err() {
                return Ok(()); // consumer gone
            }
        }
        match body.receive().await {
            Some(chunk) => buf.extend_from_slice(&chunk),
            None => {
                req_tx.close();
                return Ok(());
            }
        }
    }
}

/// Server/writer: envelope + compress response frames; render `EndStream` as the
/// Connect `EndStreamResponse` envelope (flags `0x02`).
async fn write_response_frames(
    resp_rx: PipeReceiver<Frame>,
    sink: ByteSink,
    writer: EnvelopeWriter,
) -> ConnectResult<()> {
    loop {
        match resp_rx.receive().await {
            Some(Frame::Message(frame)) => {
                let bytes = writer.write(frame)?;
                if sink.send(Bytes::from(bytes)).await.is_err() {
                    return Ok(());
                }
            }
            Some(Frame::EndStream { error, trailers }) => {
                let bytes = writer.write_end_stream(error.as_ref(), &trailers)?;
                let _ = sink.send(Bytes::from(bytes)).await;
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

/// Client/reader: the response head, then the envelopes, then the
/// `EndStreamResponse` that carries the outcome and trailers. A stream that
/// fails, or ends without its `EndStreamResponse`, ends with an error rather
/// than reading like a clean end.
async fn read_connect_response(
    mut head: HeadStream,
    mut body: BodyStream,
    resp_tx: PipeSender<Frame>,
    response_headers: Arc<Mutex<Option<SimpleHeaders>>>,
    cancel: CancelSignal,
    read_max: usize,
) -> ConnectResult<()> {
    if open_response_stream(
        &mut head,
        &mut body,
        &resp_tx,
        &response_headers,
        &cancel,
        read_max,
    )
    .await
    .is_none()
    {
        return Ok(());
    }
    match read_response_frames(&mut body, &resp_tx, None, read_max).await {
        Ok(Some(end)) => {
            // Delivered, the consumer gone, or the call cancelled.
            let _ = race(resp_tx.send(end), cancel.cancelled()).await;
            resp_tx.close();
            Ok(())
        }
        Ok(None) => {
            let e = ConnectError::internal(
                "the response stream ended without an EndStreamResponse",
            )
            .into();
            fail_stream(&resp_tx, &e, &cancel).await;
            Err(e)
        }
        Err(e) => {
            fail_stream(&resp_tx, &e, &cancel).await;
            Err(e)
        }
    }
}

/// De-envelope response bytes into `resp_tx` until the `0x02` end-stream
/// frame, which is returned (parsed to [`Frame::EndStream`], Decision 11
/// normalization); `None` when the body ends first or the consumer is gone.
async fn read_response_frames(
    body: &mut BodyStream,
    resp_tx: &PipeSender<Frame>,
    decompressor: Option<Arc<dyn Compressor>>,
    read_max: usize,
) -> ConnectResult<Option<Frame>> {
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
            if flags & Envelope::FLAG_END_STREAM != 0 {
                return Ok(Some(decode_end_stream(&data)));
            }
            if resp_tx.send(Frame::Message(data)).await.is_err() {
                return Ok(None); // consumer gone
            }
        }
        match body.next().await {
            Some(Ok(chunk)) => buf.extend_from_slice(&chunk),
            // A mid-body transport failure rides the payload as `Err` (F45
            // Part D); the caller ends the stream with it.
            Some(Err(err)) => return Err(err.into()),
            None => return Ok(None),
        }
    }
}

/// Client/writer: envelope + compress request frames onto the request body.
async fn write_request_frames(
    req_rx: PipeReceiver<Frame>,
    sink: Arc<dyn SendBody>,
    writer: EnvelopeWriter,
) -> ConnectResult<()> {
    loop {
        match req_rx.receive().await {
            Some(Frame::Message(frame)) => {
                let bytes = writer.write(frame)?;
                if sink.send_async(Bytes::from(bytes)).await.is_err() {
                    return Ok(());
                }
            }
            // Client requests half-close by dropping the sender; a client-side
            // EndStream is not sent on the Connect wire — just close the body.
            Some(Frame::EndStream { .. }) | None => {
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

/// Parse a Connect `EndStreamResponse` JSON body into [`Frame::EndStream`].
fn decode_end_stream(data: &Bytes) -> Frame {
    let parsed: EndStreamResponse = serde_json::from_slice(data).unwrap_or_default();
    let error = parsed.error.map(wire_error_to_trace);
    let mut trailers = SimpleHeaders::new();
    if let Some(meta) = parsed.metadata {
        for (key, values) in meta {
            trailers.insert(SimpleHeader::from(key), values);
        }
    }
    Frame::EndStream { error, trailers }
}

/// Reconstruct an `ErrorTrace<ConnectError>` from a wire error (server-sent).
fn wire_error_to_trace(wire: WireError) -> ErrorTrace<ConnectError> {
    let code = Code::from_str(&wire.code).unwrap_or(Code::Unknown);
    ConnectError::wire(code, wire.message.unwrap_or_default()).into()
}

// ── protocol handler / client ─────────────────────────────────────────────────

/// The Connect protocol handler (server).
#[derive(Debug, Default)]
pub struct ConnectHandler;

const ALLOWED_METHODS: [SimpleMethod; 2] = [SimpleMethod::POST, SimpleMethod::GET];

impl ProtocolHandler for ConnectHandler {
    fn kind(&self) -> crate::shared::transport::ProtocolKind {
        crate::shared::transport::ProtocolKind::Connect
    }

    fn allowed_methods(&self) -> &[SimpleMethod] {
        &ALLOWED_METHODS
    }

    fn content_types(&self) -> Vec<String> {
        vec![
            "application/json".to_string(),
            "application/proto".to_string(),
            "application/connect+json".to_string(),
            "application/connect+proto".to_string(),
        ]
    }

    fn parse_timeout(&self, headers: &SimpleHeaders) -> Option<Duration> {
        let value = headers
            .get(&SimpleHeader::from(constants::HEADER_TIMEOUT.to_string()))?
            .first()?;
        value.trim().parse::<u64>().ok().map(Duration::from_millis)
    }

    fn can_handle(&self, request: &SimpleIncomingRequest) -> bool {
        // Unary GET carries the codec in the `encoding` query (no request body /
        // Content-Type), so match a Connect GET on its query instead (Decision 05).
        if request.method == SimpleMethod::GET {
            return request
                .request_url
                .queries
                .as_ref()
                .is_some_and(|q| q.contains_key(constants::QUERY_ENCODING));
        }
        request
            .headers
            .get(&SimpleHeader::CONTENT_TYPE)
            .and_then(|v| v.first())
            .and_then(|ct| super::parse_connect_content_type(ct))
            .is_some()
    }

    fn codec_name(&self, request: &SimpleIncomingRequest) -> Option<String> {
        if request.method == SimpleMethod::GET {
            return request
                .request_url
                .queries
                .as_ref()
                .and_then(|q| q.get(constants::QUERY_ENCODING))
                .cloned();
        }
        request
            .headers
            .get(&SimpleHeader::CONTENT_TYPE)
            .and_then(|v| v.first())
            .and_then(|ct| super::parse_connect_content_type(ct))
            .map(|(codec, _)| codec)
    }

    fn new_conn(
        &self,
        request: &SimpleIncomingRequest,
        spec: Spec,
        body: ByteSource,
        responder: ByteSink,
        config: ExchangeConfig<'_>,
    ) -> ConnectResult<HandlerExchange> {
        let (content_encoding, accept_encoding) = streaming_encodings(&request.headers);
        let negotiated = negotiate_compression(
            config.compression,
            content_encoding.as_deref(),
            accept_encoding.as_deref(),
        )?;

        let peer = request_peer(request);

        let (conn, ends) = PipeHandlerConn::new(
            spec,
            peer,
            request.headers.clone(),
            config.cancel.clone(),
            DEFAULT_PIPE_DEPTH,
        );

        let reader: BoxedTask = Box::pin(read_request_frames(
            body,
            ends.request_tx,
            negotiated.request_decompressor,
            config.read_max_bytes,
            config.cancel,
        ));
        let writer_impl = EnvelopeWriter::new(negotiated.response_compressor, 0, 0);
        let writer: BoxedTask =
            Box::pin(write_response_frames(ends.response_rx, responder, writer_impl));

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
        streaming_content_type(codec_name)
    }

    fn encode_stream_error(
        &self,
        _request: &SimpleIncomingRequest,
        error: &ErrorTrace<ConnectError>,
    ) -> ConnectResult<Option<Bytes>> {
        // An uncompressed `EndStreamResponse`: valid whatever the stream
        // negotiated, as each envelope carries its own compressed flag.
        let frame = EnvelopeWriter::new(None, 0, 0)
            .write_end_stream(Some(error), &SimpleHeaders::new())?;
        Ok(Some(Bytes::from(frame)))
    }

    fn streaming_response_encoding(
        &self,
        request: &SimpleIncomingRequest,
        compression: &CompressionRegistry,
    ) -> Option<(String, String)> {
        let (content_encoding, accept_encoding) = streaming_encodings(&request.headers);
        let negotiated = negotiate_compression(
            compression,
            content_encoding.as_deref(),
            accept_encoding.as_deref(),
        )
        .ok()?;
        negotiated.response_compressor.map(|c| {
            (
                constants::HEADER_STREAMING_CONTENT_ENCODING.to_string(),
                c.name().to_string(),
            )
        })
    }

    fn decode_unary_request(
        &self,
        request: &SimpleIncomingRequest,
        body: Bytes,
        compression: &CompressionRegistry,
        read_max_bytes: usize,
    ) -> ConnectResult<Bytes> {
        // Connect unary GET carries the message in the query (Decision 05 §Unary GET).
        if request.method == SimpleMethod::GET {
            return decode_get_message(request, compression, read_max_bytes);
        }
        // Connect unary POST is a bare body, optionally compressed via Content-Encoding.
        match header(&request.headers, "content-encoding") {
            Some(name) if name != "identity" && !name.is_empty() => {
                let c = compression.get(&name).ok_or_else(|| {
                    ConnectError::unimplemented(format!("unsupported content-encoding {name:?}"))
                })?;
                Ok(Bytes::from(c.decompress(&body, read_max_bytes)?))
            }
            _ => Ok(body),
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
        response.status = Status::OK;

        // Response compression is negotiated from the request's Accept-Encoding.
        let accept = header(&request.headers, "accept-encoding");
        let compressor = negotiate_compression(compression, None, accept.as_deref())?
            .response_compressor;
        let (body, encoding) = match compressor {
            Some(c) if !outcome.frame.is_empty() => {
                (c.compress(&outcome.frame)?, Some(c.name().to_string()))
            }
            _ => (outcome.frame.to_vec(), None),
        };

        let mut headers = outcome.headers;
        headers.insert(SimpleHeader::CONTENT_TYPE, vec![unary_content_type(codec_name)]);
        if let Some(enc) = encoding {
            headers.insert(
                SimpleHeader::from("content-encoding".to_string()),
                vec![enc],
            );
        }
        // Unary trailing metadata rides as `Trailer-`-prefixed response headers.
        for (name, values) in unary_trailer_headers(&outcome.trailers) {
            headers.insert(name, values);
        }
        response.headers = headers;
        response.body = Some(SendSafeBody::Bytes(body));
        Ok(())
    }
}

/// First value of a request header (case-insensitive via `SimpleHeader`).
fn header(headers: &SimpleHeaders, name: &str) -> Option<String> {
    headers
        .get(&SimpleHeader::from(name.to_string()))
        .and_then(|v| v.first())
        .cloned()
}

/// Decode the message from a Connect unary GET query (Decision 05 §Unary GET):
/// `message` is base64url (`base64=1`) or percent-encoded, then decompressed if a
/// `compression` param names an algorithm.
fn decode_get_message(
    request: &SimpleIncomingRequest,
    compression: &CompressionRegistry,
    read_max_bytes: usize,
) -> ConnectResult<Bytes> {
    let query = |key: &str| {
        request
            .request_url
            .queries
            .as_ref()
            .and_then(|q| q.get(key))
            .cloned()
    };
    let raw = query(constants::QUERY_MESSAGE).unwrap_or_default();
    let is_base64 = query(constants::QUERY_BASE64).as_deref() == Some("1");
    let bytes = if is_base64 {
        URL_SAFE_NO_PAD
            .decode(raw.as_bytes())
            .map_err(|e| ConnectError::invalid_argument(format!("invalid base64 message: {e}")))?
    } else {
        percent_decode(raw.as_bytes())
    };
    match query(constants::QUERY_COMPRESSION) {
        Some(name) if name != "identity" && !name.is_empty() => {
            let c = compression.get(&name).ok_or_else(|| {
                ConnectError::unimplemented(format!("unsupported compression {name:?}"))
            })?;
            Ok(Bytes::from(c.decompress(&bytes, read_max_bytes)?))
        }
        _ => Ok(Bytes::from(bytes)),
    }
}

/// Decode a percent-encoded byte string (inverse of [`percent_encode`]).
fn percent_decode(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// The Connect protocol client.
#[derive(Debug, Default)]
pub struct ConnectClient;

impl ProtocolClient for ConnectClient {
    fn write_request_headers(
        &self,
        stream_type: StreamType,
        headers: &mut SimpleHeaders,
        codec_name: &str,
        compression: Option<&str>,
    ) {
        let streaming = !matches!(stream_type, StreamType::Unary);
        let content_type = if streaming {
            streaming_content_type(codec_name)
        } else {
            unary_content_type(codec_name)
        };
        headers.insert(SimpleHeader::CONTENT_TYPE, vec![content_type]);
        headers.insert(
            SimpleHeader::from(constants::HEADER_PROTOCOL_VERSION.to_string()),
            vec![constants::PROTOCOL_VERSION.to_string()],
        );
        if let Some(algo) = compression {
            let (ce, ae) = if streaming {
                (
                    constants::HEADER_STREAMING_CONTENT_ENCODING.to_string(),
                    constants::HEADER_STREAMING_ACCEPT_ENCODING.to_string(),
                )
            } else {
                ("content-encoding".to_string(), "accept-encoding".to_string())
            };
            headers.insert(SimpleHeader::from(ce), vec![algo.to_string()]);
            headers.insert(SimpleHeader::from(ae), vec![algo.to_string()]);
        }
    }

    fn encode_unary_request(&self, body: &[u8], _is_compressed: bool) -> Bytes {
        // Connect unary: bare body. Compression rides Content-Encoding (set in
        // write_request_headers), not a wire flag.
        Bytes::copy_from_slice(body)
    }

    fn decode_unary_response(&self, body: Bytes) -> ConnectResult<Bytes> {
        // Connect unary: bare body. Compression handled via Content-Encoding
        // header (already decompressed by the transport if applicable).
        Ok(body)
    }

    fn new_conn(
        &self,
        spec: &Spec,
        headers: SimpleHeaders,
        stream: TransportStream,
        config: ClientExchangeConfig,
    ) -> ConnectResult<ClientExchange> {
        let (conn, ends) = PipeClientConn::new(
            spec.clone(),
            headers,
            config.cancel.clone(),
            DEFAULT_PIPE_DEPTH,
        );

        // No compression on the client tasks here (negotiation resolves from the
        // response head in the client core, F24); identity by default.
        let writer_impl = EnvelopeWriter::new(None, 0, 0);
        let writer: BoxedTask =
            Box::pin(write_request_frames(ends.request_rx, Arc::clone(&stream.send_body), writer_impl));
        let reader: BoxedTask = Box::pin(read_connect_response(
            stream.head,
            stream.recv_body,
            ends.response_tx,
            ends.response_headers,
            config.cancel,
            config.read_max_bytes,
        ));

        Ok(ClientExchange {
            conn: Box::new(conn),
            reader_task: reader,
            writer_task: writer,
        })
    }
}

/// Read the streaming compression headers (`Connect-Content/Accept-Encoding`).
fn streaming_encodings(headers: &SimpleHeaders) -> (Option<String>, Option<String>) {
    let get = |name: &str| {
        headers
            .get(&SimpleHeader::from(name.to_string()))
            .and_then(|v| v.first())
            .cloned()
    };
    (
        get(constants::HEADER_STREAMING_CONTENT_ENCODING),
        get(constants::HEADER_STREAMING_ACCEPT_ENCODING),
    )
}

fn request_peer(request: &SimpleIncomingRequest) -> Peer {
    let addr = request
        .connection
        .peer_addr
        .as_ref()
        .map(|a| format!("{a:?}"))
        .unwrap_or_default();
    Peer {
        addr,
        protocol: "connect".to_string(),
    }
}
