//! The gRPC-Web protocol (Decision 05 §Protocol 3) — browser reach on HTTP/1.1.
//!
//! WHY: gRPC-Web is gRPC framing without the HTTP/2 requirement: status +
//! trailers ride an in-body **trailer frame** (flags `0x80`) instead of HTTP/2
//! trailing HEADERS, and a **text mode** base64-encodes the whole body for
//! browsers that cannot read binary response bodies.
//!
//! WHAT: the grpc / grpc-web constants, gRPC status trailer build/parse (with the
//! `grpc-status-details-bin` preference, P9), a streaming base64 adapter for text
//! mode, the reader/writer tasks, and [`GrpcWebHandler`]/[`GrpcWebClient`].
//!
//! HOW: messages are `0x00` envelopes; the terminator is the `0x80` trailer frame
//! whose body is an HTTP-header block (`grpc-status`, percent-encoded
//! `grpc-message`, custom trailers). Text mode wraps the body bytes in the
//! [`Base64StreamEncoder`]/[`Base64StreamDecoder`], which buffer partial groups
//! across chunk boundaries.

use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use bytes::{Bytes, BytesMut};
use foundation_core::valtron::{PipeReceiver, PipeSender};
use foundation_errstacks::ErrorTrace;
use foundation_netio::simple_http::shared::{
    SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};

use crate::compression::{negotiate_compression, CompressionRegistry, Compressor};
use crate::context::{CancelSignal, Peer, Spec, StreamType};
use crate::error::{Code, ConnectError, ConnectResult};
use crate::envelope::{Envelope, EnvelopeWriter, ENVELOPE_HEADER_LEN};
use crate::transport::{
    ByteSink, ByteSource, Frame, PipeClientConn, PipeHandlerConn, TransportStream,
    DEFAULT_PIPE_DEPTH,
};

use super::{
    BoxedTask, ClientExchange, HandlerExchange, ProtocolClient, ProtocolHandler, UnaryOutcome,
};

/// gRPC (and gRPC-Web) constants (Decision 05).
pub mod constants {
    /// `grpc-timeout` header.
    pub const HEADER_TIMEOUT: &str = "grpc-timeout";
    /// `grpc-encoding` header.
    pub const HEADER_ENCODING: &str = "grpc-encoding";
    /// `grpc-accept-encoding` header.
    pub const HEADER_ACCEPT_ENCODING: &str = "grpc-accept-encoding";
    /// `grpc-status` trailer.
    pub const TRAILER_STATUS: &str = "grpc-status";
    /// `grpc-message` trailer (percent-encoded).
    pub const TRAILER_MESSAGE: &str = "grpc-message";
    /// `grpc-status-details-bin` trailer (base64 `google.rpc.Status`).
    pub const TRAILER_STATUS_DETAILS: &str = "grpc-status-details-bin";
    /// gRPC-Web binary content-type prefix.
    pub const CONTENT_TYPE_PREFIX: &str = "application/grpc-web";
    /// gRPC-Web text content-type prefix.
    pub const CONTENT_TYPE_TEXT_PREFIX: &str = "application/grpc-web-text";
    /// Trailer-frame flag.
    pub const TRAILER_FLAG: u8 = 0x80;
}

// ── grpc-message percent coding ───────────────────────────────────────────────

/// Percent-encode a `grpc-message` (bytes outside printable ASCII, and `%`).
#[must_use]
pub fn percent_encode_message(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    for &b in msg.as_bytes() {
        if (0x20..=0x7E).contains(&b) && b != b'%' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Decode a percent-encoded `grpc-message`.
#[must_use]
pub fn percent_decode_message(msg: &str) -> String {
    let bytes = msg.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&msg[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ── google.rpc.Status (minimal) for grpc-status-details-bin ───────────────────

/// Encode a minimal `google.rpc.Status` (`code` field 1, `message` field 2).
#[must_use]
pub fn encode_status_proto(code: i32, message: &str) -> Vec<u8> {
    let mut out = Vec::new();
    if code != 0 {
        out.push(0x08); // field 1, varint
        write_varint(code as u64, &mut out);
    }
    if !message.is_empty() {
        out.push(0x12); // field 2, length-delimited
        write_varint(message.len() as u64, &mut out);
        out.extend_from_slice(message.as_bytes());
    }
    out
}

/// Decode the `code`/`message` from a minimal `google.rpc.Status`.
#[must_use]
pub fn decode_status_proto(bytes: &[u8]) -> (i32, String) {
    let mut code = 0i32;
    let mut message = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let tag = bytes[i];
        i += 1;
        match tag {
            0x08 => {
                if let Some((v, n)) = read_varint(&bytes[i..]) {
                    code = v as i32;
                    i += n;
                } else {
                    break;
                }
            }
            0x12 => {
                let Some((len, n)) = read_varint(&bytes[i..]) else {
                    break;
                };
                i += n;
                let len = len as usize;
                if i + len > bytes.len() {
                    break;
                }
                message = String::from_utf8_lossy(&bytes[i..i + len]).into_owned();
                i += len;
            }
            _ => break,
        }
    }
    (code, message)
}

fn write_varint(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
            break;
        }
    }
}

fn read_varint(bytes: &[u8]) -> Option<(u64, usize)> {
    let mut result = 0u64;
    let mut shift = 0;
    for (i, &b) in bytes.iter().enumerate() {
        result |= u64::from(b & 0x7F) << shift;
        if b & 0x80 == 0 {
            return Some((result, i + 1));
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
    None
}

// ── status trailers ───────────────────────────────────────────────────────────

/// Build the gRPC status trailers for a result (error or success) plus any custom
/// trailing metadata.
#[must_use]
pub fn build_status_trailers(
    error: Option<&ErrorTrace<ConnectError>>,
    custom: &SimpleHeaders,
) -> SimpleHeaders {
    let mut trailers = custom.clone();
    let (status, message) = match error {
        Some(e) => {
            let ctx = e.current_context();
            (ctx.code().grpc_code(), ctx.message().to_string())
        }
        None => (0, String::new()),
    };
    trailers.insert(
        SimpleHeader::from(constants::TRAILER_STATUS.to_string()),
        vec![status.to_string()],
    );
    if !message.is_empty() {
        trailers.insert(
            SimpleHeader::from(constants::TRAILER_MESSAGE.to_string()),
            vec![percent_encode_message(&message)],
        );
        // Also emit the binary details (google.rpc.Status) for P9 clients.
        let details = encode_status_proto(status as i32, &message);
        trailers.insert(
            SimpleHeader::from(constants::TRAILER_STATUS_DETAILS.to_string()),
            vec![STANDARD.encode(details)],
        );
    }
    trailers
}

/// Parse gRPC status trailers into an optional terminal error, preferring
/// `grpc-status-details-bin` over `grpc-status`/`grpc-message` when both are
/// present (Decision 05 P9).
#[must_use]
pub fn parse_status_trailers(trailers: &SimpleHeaders) -> Option<ErrorTrace<ConnectError>> {
    let get = |name: &str| {
        trailers
            .get(&SimpleHeader::from(name.to_string()))
            .and_then(|v| v.first())
            .cloned()
    };

    // P9: prefer the binary Status details when present and decodable.
    if let Some(details_b64) = get(constants::TRAILER_STATUS_DETAILS) {
        if let Ok(raw) = STANDARD.decode(details_b64.trim()) {
            let (code, message) = decode_status_proto(&raw);
            if code == 0 {
                return None;
            }
            let c = Code::from_u32(code as u32).unwrap_or(Code::Unknown);
            return Some(ConnectError::wire(c, message).into());
        }
    }

    let status: u32 = get(constants::TRAILER_STATUS)?.trim().parse().ok()?;
    if status == 0 {
        return None;
    }
    let message = get(constants::TRAILER_MESSAGE)
        .map(|m| percent_decode_message(&m))
        .unwrap_or_default();
    let code = Code::from_u32(status).unwrap_or(Code::Unknown);
    Some(ConnectError::wire(code, message).into())
}

/// Render trailers as the `0x80` trailer-frame body (HTTP header block).
#[must_use]
pub fn render_trailer_frame_body(trailers: &SimpleHeaders) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, values) in trailers {
        for value in values {
            body.extend_from_slice(name.to_string().to_ascii_lowercase().as_bytes());
            body.extend_from_slice(b": ");
            body.extend_from_slice(value.as_bytes());
            body.extend_from_slice(b"\r\n");
        }
    }
    body
}

/// Parse the `0x80` trailer-frame body (HTTP header block) into headers.
#[must_use]
pub fn parse_trailer_frame_body(body: &[u8]) -> SimpleHeaders {
    let mut headers = SimpleHeaders::new();
    let text = String::from_utf8_lossy(body);
    for line in text.split("\r\n") {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers
                .entry(SimpleHeader::from(k.trim().to_string()))
                .or_default()
                .push(v.trim().to_string());
        }
    }
    headers
}

// ── streaming base64 (text mode) ──────────────────────────────────────────────

/// Streaming base64 encoder: emits complete 4-char groups as bytes arrive and
/// flushes a padded final group on [`finish`](Self::finish). Standard alphabet.
#[derive(Default)]
pub struct Base64StreamEncoder {
    pending: Vec<u8>,
}

impl Base64StreamEncoder {
    /// Feed raw bytes; returns the base64 for all complete 3-byte groups so far.
    pub fn feed(&mut self, data: &[u8]) -> Vec<u8> {
        self.pending.extend_from_slice(data);
        let complete = self.pending.len() - self.pending.len() % 3;
        if complete == 0 {
            return Vec::new();
        }
        let head: Vec<u8> = self.pending.drain(..complete).collect();
        STANDARD.encode(head).into_bytes()
    }

    /// Flush the trailing partial group (with padding).
    pub fn finish(self) -> Vec<u8> {
        if self.pending.is_empty() {
            Vec::new()
        } else {
            STANDARD.encode(self.pending).into_bytes()
        }
    }
}

/// Streaming base64 decoder: decodes complete 4-char groups as characters arrive,
/// buffering partial groups across chunk boundaries. Standard alphabet.
#[derive(Default)]
pub struct Base64StreamDecoder {
    pending: Vec<u8>,
}

impl Base64StreamDecoder {
    /// Feed base64 characters; returns decoded bytes for all complete 4-char
    /// groups so far.
    ///
    /// # Errors
    /// [`ConnectError`] ([`Code::InvalidArgument`]) on invalid base64.
    pub fn feed(&mut self, data: &[u8]) -> ConnectResult<Vec<u8>> {
        self.pending.extend_from_slice(data);
        let complete = self.pending.len() - self.pending.len() % 4;
        if complete == 0 {
            return Ok(Vec::new());
        }
        let head: Vec<u8> = self.pending.drain(..complete).collect();
        STANDARD
            .decode(head)
            .map_err(|e| ConnectError::invalid_argument(format!("invalid grpc-web-text base64: {e}")).into())
    }
}

// ── content-type ──────────────────────────────────────────────────────────────

/// The gRPC-Web content-type: `application/grpc-web+{codec}` (binary) or
/// `application/grpc-web-text+{codec}` (text).
#[must_use]
pub fn content_type(codec: &str, text: bool) -> String {
    if text {
        format!("{}+{codec}", constants::CONTENT_TYPE_TEXT_PREFIX)
    } else {
        format!("{}+{codec}", constants::CONTENT_TYPE_PREFIX)
    }
}

/// Parse a gRPC-Web content-type into `(codec, is_text)`, or `None` if not
/// gRPC-Web.
#[must_use]
pub fn parse_content_type(content_type: &str) -> Option<(String, bool)> {
    let canonical = super::canonicalize_content_type(content_type);
    if let Some(rest) = canonical.strip_prefix(constants::CONTENT_TYPE_TEXT_PREFIX) {
        let codec = rest.strip_prefix('+').unwrap_or("proto");
        return Some((codec.to_string(), true));
    }
    if let Some(rest) = canonical.strip_prefix(constants::CONTENT_TYPE_PREFIX) {
        let codec = rest.strip_prefix('+').unwrap_or("proto");
        return Some((codec.to_string(), false));
    }
    None
}

// ── reader/writer tasks ───────────────────────────────────────────────────────

async fn read_frames(
    body: ByteSource,
    tx: PipeSender<Frame>,
    decompressor: Option<Arc<dyn Compressor>>,
    read_max: usize,
    text: bool,
    client: bool,
) -> ConnectResult<()> {
    let mut buf = BytesMut::new();
    let mut b64 = Base64StreamDecoder::default();
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
            if flags & constants::TRAILER_FLAG != 0 {
                // Trailer frame (client only): normalize to Frame::EndStream.
                let trailers = parse_trailer_frame_body(&payload);
                let error = if client {
                    parse_status_trailers(&trailers)
                } else {
                    None
                };
                let _ = tx.send(Frame::EndStream { error, trailers }).await;
                tx.close();
                return Ok(());
            }
            let data = decompress_if(flags, payload, &decompressor, read_max)?;
            if tx.send(Frame::Message(data)).await.is_err() {
                return Ok(());
            }
        }
        match body.receive().await {
            Some(chunk) => {
                if text {
                    buf.extend_from_slice(&b64.feed(&chunk)?);
                } else {
                    buf.extend_from_slice(&chunk);
                }
            }
            None => {
                tx.close();
                return Ok(());
            }
        }
    }
}

async fn write_frames(
    resp_rx: PipeReceiver<Frame>,
    sink: ByteSink,
    writer: EnvelopeWriter,
    text: bool,
    server: bool,
) -> ConnectResult<()> {
    let mut encoder = Base64StreamEncoder::default();
    let emit = |bytes: Vec<u8>, enc: &mut Base64StreamEncoder| {
        if text {
            enc.feed(&bytes)
        } else {
            bytes
        }
    };
    loop {
        match resp_rx.receive().await {
            Some(Frame::Message(frame)) => {
                let enveloped = writer.write(frame)?;
                let out = emit(enveloped, &mut encoder);
                if !out.is_empty() && sink.send(Bytes::from(out)).await.is_err() {
                    return Ok(());
                }
            }
            Some(Frame::EndStream { error, trailers }) => {
                if server {
                    // Render the 0x80 trailer frame with gRPC status.
                    let status_trailers = build_status_trailers(error.as_ref(), &trailers);
                    let body = render_trailer_frame_body(&status_trailers);
                    let mut frame = Vec::with_capacity(ENVELOPE_HEADER_LEN + body.len());
                    frame.push(constants::TRAILER_FLAG);
                    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
                    frame.extend_from_slice(&body);
                    let out = emit(frame, &mut encoder);
                    if !out.is_empty() {
                        let _ = sink.send(Bytes::from(out)).await;
                    }
                }
                let tail = encoder.finish();
                if !tail.is_empty() {
                    let _ = sink.send(Bytes::from(tail)).await;
                }
                sink.close();
                return Ok(());
            }
            None => {
                let tail = encoder.finish();
                if !tail.is_empty() {
                    let _ = sink.send(Bytes::from(tail)).await;
                }
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

// ── handler / client ──────────────────────────────────────────────────────────

/// The gRPC-Web protocol handler (server).
#[derive(Debug, Default)]
pub struct GrpcWebHandler;

const ALLOWED_METHODS: [SimpleMethod; 1] = [SimpleMethod::POST];

impl ProtocolHandler for GrpcWebHandler {
    fn kind(&self) -> crate::transport::ProtocolKind {
        crate::transport::ProtocolKind::GrpcWeb
    }
    fn allowed_methods(&self) -> &[SimpleMethod] {
        &ALLOWED_METHODS
    }
    fn content_types(&self) -> Vec<String> {
        vec![
            content_type("proto", false),
            content_type("json", false),
            content_type("proto", true),
            content_type("json", true),
        ]
    }
    fn parse_timeout(&self, headers: &SimpleHeaders) -> Option<Duration> {
        let v = headers
            .get(&SimpleHeader::from(constants::HEADER_TIMEOUT.to_string()))?
            .first()?;
        crate::envelope::decode_grpc_timeout(v.trim()).ok()
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
            .map(|(codec, _)| codec)
    }

    fn new_conn(
        &self,
        request: &SimpleIncomingRequest,
        spec: Spec,
        body: ByteSource,
        responder: ByteSink,
        compression: &CompressionRegistry,
    ) -> ConnectResult<HandlerExchange> {
        let text = is_text(request);

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
            protocol: "grpc-web".to_string(),
        };

        let (conn, ends) = PipeHandlerConn::new(
            spec,
            peer,
            request.headers.clone(),
            CancelSignal::new(),
            DEFAULT_PIPE_DEPTH,
        );

        let reader: BoxedTask = Box::pin(read_frames(
            body,
            ends.request_tx,
            negotiated.request_decompressor,
            0,
            text,
            false,
        ));
        let writer: BoxedTask = Box::pin(write_frames(
            ends.response_rx,
            responder,
            EnvelopeWriter::new(negotiated.response_compressor, 0, 0),
            text,
            true,
        ));

        Ok(HandlerExchange {
            conn: Box::new(conn),
            reader_task: reader,
            writer_task: writer,
        })
    }

    fn streaming_response_content_type(
        &self,
        request: &SimpleIncomingRequest,
        codec_name: &str,
    ) -> String {
        content_type(codec_name, is_text(request))
    }

    fn decode_unary_request(
        &self,
        request: &SimpleIncomingRequest,
        body: Bytes,
        compression: &CompressionRegistry,
    ) -> ConnectResult<Bytes> {
        // Text mode base64-encodes the whole body; decode it back to binary first.
        let raw = if is_text(request) {
            Bytes::from(STANDARD.decode(trim_ascii(&body)).map_err(|e| {
                ConnectError::invalid_argument(format!("invalid grpc-web-text base64: {e}"))
            })?)
        } else {
            body
        };
        if raw.is_empty() {
            return Ok(Bytes::new());
        }
        if raw.len() < ENVELOPE_HEADER_LEN {
            return Err(ConnectError::invalid_argument("truncated gRPC-Web frame").into());
        }
        let flags = raw[0];
        let len = u32::from_be_bytes([raw[1], raw[2], raw[3], raw[4]]) as usize;
        let end = (ENVELOPE_HEADER_LEN + len).min(raw.len());
        let payload = raw.slice(ENVELOPE_HEADER_LEN..end);
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
        let text = is_text(request);
        response.status = Status::OK; // gRPC-Web status rides the trailer frame.

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

        // Message envelope followed by the `0x80` trailer frame carrying grpc-status: 0.
        let mut out = writer.write(outcome.frame)?;
        let status_trailers = build_status_trailers(None, &outcome.trailers);
        let trailer_body = render_trailer_frame_body(&status_trailers);
        out.push(constants::TRAILER_FLAG);
        out.extend_from_slice(&(trailer_body.len() as u32).to_be_bytes());
        out.extend_from_slice(&trailer_body);

        let body = if text {
            let mut enc = Base64StreamEncoder::default();
            let mut encoded = enc.feed(&out);
            encoded.extend(enc.finish());
            encoded
        } else {
            out
        };

        let mut headers = outcome.headers;
        headers.insert(SimpleHeader::CONTENT_TYPE, vec![content_type(codec_name, text)]);
        if let Some(enc) = response_encoding {
            headers.insert(
                SimpleHeader::from(constants::HEADER_ENCODING.to_string()),
                vec![enc],
            );
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

/// Whether the request uses gRPC-Web **text** mode (base64 body).
fn is_text(request: &SimpleIncomingRequest) -> bool {
    request
        .headers
        .get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|v| v.first())
        .and_then(|ct| parse_content_type(ct))
        .map(|(_, t)| t)
        .unwrap_or(false)
}

/// Trim ASCII whitespace from both ends of a byte slice (base64 bodies may carry
/// trailing newlines).
fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map_or(start, |p| p + 1);
    &bytes[start..end]
}

/// The gRPC-Web protocol client.
#[derive(Debug, Default)]
pub struct GrpcWebClient {
    /// Whether to use text mode.
    pub text: bool,
}

impl ProtocolClient for GrpcWebClient {
    fn write_request_headers(
        &self,
        _stream_type: StreamType,
        headers: &mut SimpleHeaders,
        codec_name: &str,
        compression: Option<&str>,
    ) {
        headers.insert(
            SimpleHeader::CONTENT_TYPE,
            vec![content_type(codec_name, self.text)],
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

    fn new_conn(
        &self,
        spec: &Spec,
        headers: SimpleHeaders,
        stream: TransportStream,
    ) -> ConnectResult<ClientExchange> {
        let (conn, ends) =
            PipeClientConn::new(spec.clone(), headers, CancelSignal::new(), DEFAULT_PIPE_DEPTH);

        let writer: BoxedTask = Box::pin(write_frames(
            ends.request_rx,
            stream.send_body,
            EnvelopeWriter::new(None, 0, 0),
            self.text,
            false,
        ));
        let reader: BoxedTask = Box::pin(read_frames(
            stream.recv_body,
            ends.response_tx,
            None,
            0,
            self.text,
            true,
        ));

        Ok(ClientExchange {
            conn: Box::new(conn),
            reader_task: reader,
            writer_task: writer,
        })
    }
}
