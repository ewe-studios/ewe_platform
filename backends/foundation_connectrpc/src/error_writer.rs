//! `ErrorWriter` — protocol-aware error responses (Decision 03 H17 / Decision 05).
//!
//! WHY: Middleware that runs **before** handler dispatch (e.g. auth) must be able
//! to write an error in the correct protocol format without owning the handler
//! machinery. The rendering differs per protocol: Connect unary is an HTTP status
//! + JSON body; Connect streaming is a `200` with an `EndStreamResponse` envelope
//! (`0x02`); gRPC-Web is a `200` with a `0x80` trailer frame carrying the gRPC
//! status.
//!
//! WHAT: [`ErrorWriter`] with [`is_supported`](ErrorWriter::is_supported) and
//! [`write`](ErrorWriter::write), plus the GET-only `304 Not Modified` rendering.
//!
//! HOW: reuses the envelope/status helpers (F15/F20) and the error model's
//! canonical JSON (F12). It owns no buffer pool and no codec table (Decision 03
//! H17) — it is one shared instance across worker threads.
//!
//! > The `google.rpc.Status` protobuf and `google.protobuf.Any` typed details use
//! > the minimal in-crate encoder here; the full generated `google.rpc.Status` /
//! > buffa-types `Any` integration lands with codegen (spec-41 F26).

use foundation_errstacks::ErrorTrace;
use foundation_netio::simple_http::shared::{
    SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};

use crate::error::{Code, ConnectError, ConnectResult};
use crate::envelope::EnvelopeWriter;
use crate::protocol::grpc_web::{
    self, build_status_trailers, render_trailer_frame_body, Base64StreamEncoder,
};
use crate::protocol::{connect, parse_connect_content_type};

/// Writes errors in the correct protocol format from pre-dispatch middleware
/// (Decision 03). Stateless; one shared instance is used across worker threads.
#[derive(Debug, Default, Clone, Copy)]
pub struct ErrorWriter;

impl ErrorWriter {
    /// Create an error writer.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Whether this writer can render an error for `request` (i.e. the request is
    /// a recognized Connect or gRPC-Web request).
    #[must_use]
    pub fn is_supported(&self, request: &SimpleIncomingRequest) -> bool {
        request_kind(request).is_some()
    }

    /// Write `error` into `response` in the appropriate protocol format.
    ///
    /// # Errors
    /// A [`ConnectError`]-carrying trace if the error body cannot be serialized.
    pub fn write(
        &self,
        response: &mut SimpleOutgoingResponse,
        request: &SimpleIncomingRequest,
        error: &ErrorTrace<ConnectError>,
    ) -> ConnectResult<()> {
        // GET-only 304 Not Modified (Decision 03 P15): status + headers, no body.
        if error.current_context().is_not_modified() && request.method == SimpleMethod::GET {
            response.status = Status::NotModified;
            response.body = None;
            return Ok(());
        }

        match request_kind(request) {
            Some(RequestKind::ConnectUnary) => self.write_connect_unary(response, error),
            Some(RequestKind::ConnectStreaming { codec }) => {
                self.write_connect_streaming(response, &codec, error)
            }
            Some(RequestKind::GrpcWeb { codec, text }) => {
                self.write_grpc_web(response, &codec, text, error)
            }
            // Not a recognized RPC request — render a Connect-JSON error as a safe default.
            None => self.write_connect_unary(response, error),
        }
    }

    fn write_connect_unary(
        &self,
        response: &mut SimpleOutgoingResponse,
        error: &ErrorTrace<ConnectError>,
    ) -> ConnectResult<()> {
        response.status = status_from_code(error.current_context().code());
        set_content_type(response, connect::ERROR_CONTENT_TYPE);
        response.body = Some(SendSafeBody::Bytes(connect::unary_error_body(error)?));
        Ok(())
    }

    fn write_connect_streaming(
        &self,
        response: &mut SimpleOutgoingResponse,
        codec: &str,
        error: &ErrorTrace<ConnectError>,
    ) -> ConnectResult<()> {
        response.status = Status::OK; // Connect streaming errors are in the EndStream frame.
        set_content_type(response, &connect::streaming_content_type(codec));
        let writer = EnvelopeWriter::new(None, 0, 0);
        let frame = writer.write_end_stream(Some(error), &SimpleHeaders::new())?;
        response.body = Some(SendSafeBody::Bytes(frame));
        Ok(())
    }

    fn write_grpc_web(
        &self,
        response: &mut SimpleOutgoingResponse,
        codec: &str,
        text: bool,
        error: &ErrorTrace<ConnectError>,
    ) -> ConnectResult<()> {
        response.status = Status::OK; // gRPC-Web status is in the trailer frame.
        set_content_type(response, &grpc_web::content_type(codec, text));

        let trailers = build_status_trailers(Some(error), &SimpleHeaders::new());
        let body = render_trailer_frame_body(&trailers);
        let mut frame = Vec::with_capacity(5 + body.len());
        frame.push(grpc_web::constants::TRAILER_FLAG);
        frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
        frame.extend_from_slice(&body);

        let out = if text {
            let mut enc = Base64StreamEncoder::default();
            let mut encoded = enc.feed(&frame);
            encoded.extend(enc.finish());
            encoded
        } else {
            frame
        };
        response.body = Some(SendSafeBody::Bytes(out));
        Ok(())
    }
}

/// The recognized error-writing shape of a request.
enum RequestKind {
    ConnectUnary,
    ConnectStreaming { codec: String },
    GrpcWeb { codec: String, text: bool },
}

fn request_kind(request: &SimpleIncomingRequest) -> Option<RequestKind> {
    let ct = request
        .headers
        .get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|v| v.first())?;
    if let Some((codec, text)) = grpc_web::parse_content_type(ct) {
        return Some(RequestKind::GrpcWeb { codec, text });
    }
    match parse_connect_content_type(ct)? {
        (codec, true) => Some(RequestKind::ConnectStreaming { codec }),
        (_, false) => Some(RequestKind::ConnectUnary),
    }
}

fn set_content_type(response: &mut SimpleOutgoingResponse, value: &str) {
    response
        .headers
        .insert(SimpleHeader::CONTENT_TYPE, vec![value.to_string()]);
}

/// Map an RPC [`Code`] to the HTTP [`Status`] for a Connect unary error.
///
/// Note: [`Code::Canceled`] maps to HTTP 499 in the Connect spec, which has no
/// `Status` variant in `foundation_netio`; it falls back to `500` here (rare for
/// a server-set unary status).
#[must_use]
pub fn status_from_code(code: Code) -> Status {
    match code {
        Code::Canceled => Status::InternalServerError, // 499 has no Status variant
        Code::Unknown | Code::DataLoss | Code::Internal => Status::InternalServerError,
        Code::InvalidArgument | Code::FailedPrecondition | Code::OutOfRange => Status::BadRequest,
        Code::DeadlineExceeded => Status::GatewayTimeout,
        Code::NotFound => Status::NotFound,
        Code::AlreadyExists | Code::Aborted => Status::Conflict,
        Code::PermissionDenied => Status::Forbidden,
        Code::ResourceExhausted => Status::TooManyRequests,
        Code::Unimplemented => Status::NotImplemented,
        Code::Unavailable => Status::ServiceUnavailable,
        Code::Unauthenticated => Status::Unauthorized,
    }
}

