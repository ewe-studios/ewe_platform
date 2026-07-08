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

use std::sync::Arc;
use std::time::Duration;

use bytes::{Bytes, BytesMut};
use foundation_core::valtron::{PipeReceiver, PipeSender};
use foundation_netio::simple_http::shared::{
    SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};
use futures::StreamExt;

use crate::compression::{negotiate_compression, CompressionRegistry, Compressor};
use crate::context::{CancelSignal, Peer, Spec, StreamType};
use crate::error::{ConnectError, ConnectResult};
use crate::envelope::{Envelope, EnvelopeWriter, ENVELOPE_HEADER_LEN};
use crate::transport::{
    body_stream_from_pipe, BodyStream, ByteSink, ByteSource, Frame, PipeClientConn,
    PipeHandlerConn, TransportStream, DEFAULT_PIPE_DEPTH,
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
#[must_use]
pub fn parse_content_type(content_type: &str) -> Option<String> {
    let canonical = super::canonicalize_content_type(content_type);
    let rest = canonical.strip_prefix(constants::CONTENT_TYPE_PREFIX)?;
    let codec = rest.strip_prefix('+').unwrap_or("proto");
    Some(codec.to_string())
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
            Some(Err(err)) => {
                tx.close();
                return Err(err.into());
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
) -> ConnectResult<()> {
    loop {
        match resp_rx.receive().await {
            Some(Frame::Message(frame)) => {
                let enveloped = writer.write(frame)?;
                if !enveloped.is_empty() && sink.send(Bytes::from(enveloped)).await.is_err() {
                    return Ok(());
                }
            }
            Some(Frame::EndStream { error: _, trailers: _ }) => {
                // gRPC errors ride HTTP/2 trailing HEADERS, not the body.
                // The EndStream frame is consumed without writing body bytes;
                // the writer task ends here and the transport layer emits
                // trailing HEADERS from SimpleOutgoingResponse.trailers.
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
    fn kind(&self) -> crate::transport::ProtocolKind {
        crate::transport::ProtocolKind::Grpc
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
        let writer: BoxedTask = Box::pin(write_frames(
            ends.response_rx,
            responder,
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
            stream.send_body,
            EnvelopeWriter::new(None, 0, 0),
        ));
        let reader: BoxedTask = Box::pin(read_frames(
            stream.recv_body,
            ends.response_tx,
            None,
            0,
        ));

        Ok(ClientExchange {
            conn: Box::new(conn),
            reader_task: reader,
            writer_task: writer,
        })
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn header(headers: &SimpleHeaders, name: &str) -> Option<String> {
    headers
        .get(&SimpleHeader::from(name.to_string()))
        .and_then(|v| v.first())
        .cloned()
}

// ── tests ───────────────────────────────────────────────────────────────────

