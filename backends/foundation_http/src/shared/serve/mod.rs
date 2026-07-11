//! Serve trait — core handler abstraction.
//!
//! `Serve` (native-only): takes `SharedByteBufferStream<RawStream>` for TCP connections.
//! `ServeWriter` (both targets): takes `&mut dyn Write` for memory-backed or any writable stream.

pub mod h2;

#[cfg(not(target_family = "wasm"))]
use std::sync::Arc;

use foundation_netio::shared::http::{
    Http11, RenderHttp, SimpleIncomingRequest, SimpleOutgoingResponse, Status,
    SimpleHeader, SendSafeBody,
};
use foundation_errstacks::ErrorTrace;

use crate::shared::context::ContextBag;

// ---------------------------------------------------------------------------
// ServeError

/// Error types that can cause a connection close.
#[derive(Debug)]
pub enum ServeError {
    /// The handler detected a bad request (400-level).
    BadRequest { status: u16, reason: String },
    /// Internal handler error (500-level).
    InternalError { status: u16, reason: String },
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServeError::BadRequest { status, reason } => {
                write!(f, "bad request ({status}): {reason}")
            }
            ServeError::InternalError { status, reason } => {
                write!(f, "internal error ({status}): {reason}")
            }
        }
    }
}

impl std::error::Error for ServeError {}

impl From<ServeError> for ErrorTrace<ServeError> {
    fn from(err: ServeError) -> Self {
        ErrorTrace::new(err)
    }
}

// ---------------------------------------------------------------------------
// ConnectionResult

/// Three outcomes telling the worker what to do after a handler runs.
pub enum ConnectionResult {
    /// Handler wrote a response; connection kept alive for next request.
    Keep,
    /// Handler took ownership permanently (WebSocket, SSE, long-poll).
    /// The worker loop exits immediately.
    Take,
    /// Close the connection, optionally carrying an error for logging.
    Close(Option<ErrorTrace<ServeError>>),
}

// ---------------------------------------------------------------------------
// Serve (native-only — uses RawStream)

/// Core handler trait — executed at request time (native TCP connections).
#[cfg(not(target_family = "wasm"))]
pub trait Serve: Send + Sync + 'static {
    /// Handle an incoming request.
    fn serve(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: foundation_core::io::ioutils::SharedByteBufferStream<foundation_netio::netcap::RawStream>,
    ) -> ConnectionResult;
}

/// Factory trait for creating handler instances.
#[cfg(not(target_family = "wasm"))]
pub trait ServeFactory: Serve + Sized {
    /// Create a new instance of this handler.
    fn create(bag: &ContextBag) -> Self;
}

// ---------------------------------------------------------------------------
// ServeWriter (both targets)

/// Handler trait for writers — works with any `std::io::Write` destination.
///
/// Use this for wasm-compatible handlers or when you want to collect
/// response bytes into a buffer instead of writing to a TCP stream.
pub trait ServeWriter: Send + Sync + 'static {
    /// Handle an incoming request, writing the response to `conn`.
    fn serve_writer(
        &self,
        bag: &ContextBag,
        req: SimpleIncomingRequest,
        conn: &mut dyn std::io::Write,
    ) -> ConnectionResult;
}

/// Factory trait for creating `ServeWriter` handler instances.
pub trait ServeWriterFactory: ServeWriter + Sized {
    /// Create a new instance of this handler.
    fn create(bag: &ContextBag) -> Self;
}

// ---------------------------------------------------------------------------
// respond helpers

/// Convert a u16 status code to a `Status` enum.
fn status_from_code(code: u16) -> Status {
    match code {
        100 => Status::Continue,
        101 => Status::SwitchingProtocols,
        200 => Status::OK,
        201 => Status::Created,
        202 => Status::Accepted,
        204 => Status::NoContent,
        301 => Status::MovedPermanently,
        302 => Status::Found,
        303 => Status::SeeOther,
        304 => Status::NotModified,
        307 => Status::TemporaryRedirect,
        308 => Status::PermanentRedirect,
        400 => Status::BadRequest,
        401 => Status::Unauthorized,
        403 => Status::Forbidden,
        404 => Status::NotFound,
        405 => Status::MethodNotAllowed,
        409 => Status::Conflict,
        410 => Status::Gone,
        422 => Status::UnprocessableEntity,
        429 => Status::TooManyRequests,
        500 => Status::InternalServerError,
        501 => Status::NotImplemented,
        502 => Status::BadGateway,
        503 => Status::ServiceUnavailable,
        504 => Status::GatewayTimeout,
        505 => Status::HttpVersionNotSupported,
        n => Status::Numbered(n as usize, String::new()),
    }
}

/// Build and render a response directly to a writer.
fn render_response(
    conn: &mut impl std::io::Write,
    status: u16,
    content_type: &str,
    body: SendSafeBody,
) -> Result<(), ErrorTrace<ServeError>> {
    let response = SimpleOutgoingResponse::builder()
        .with_status(status_from_code(status))
        .add_header(SimpleHeader::CONTENT_TYPE, content_type)
        .add_header(SimpleHeader::CONNECTION, "close")
        .with_body(body)
        .build()
        .map_err(|e| ServeError::InternalError { status: 500, reason: e.to_string() })?;

    Http11::response(response)
        .http_render_to_writer(conn)
        .map_err(|e| ServeError::InternalError { status: 500, reason: e.to_string() })?;

    Ok(())
}

/// Quick-response helpers for common HTTP patterns.
pub mod respond {
    use serde::Serialize;

    use super::{
        ErrorTrace, Http11, RenderHttp, SendSafeBody, ServeError, SimpleHeader,
        SimpleOutgoingResponse, render_response, status_from_code,
    };

    /// Write a JSON response with the given status code.
    pub fn json<T: Serialize>(
        conn: &mut impl std::io::Write,
        status: u16,
        body: &T,
    ) -> Result<(), ErrorTrace<ServeError>> {
        let bytes = serde_json::to_vec(body)
            .map_err(|e| ServeError::InternalError { status: 500, reason: e.to_string() })?;
        render_response(conn, status, "application/json", SendSafeBody::Bytes(bytes))
    }

    /// Write a plain text response.
    pub fn text(
        conn: &mut impl std::io::Write,
        status: u16,
        body: &str,
    ) -> Result<(), ErrorTrace<ServeError>> {
        render_response(conn, status, "text/plain", SendSafeBody::Text(body.into()))
    }

    /// Write an HTML response.
    pub fn html(
        conn: &mut impl std::io::Write,
        status: u16,
        body: &str,
    ) -> Result<(), ErrorTrace<ServeError>> {
        render_response(conn, status, "text/html", SendSafeBody::Text(body.into()))
    }

    /// Write a redirect response (301 or 302).
    pub fn redirect(
        conn: &mut impl std::io::Write,
        status: u16,
        location: &str,
    ) -> Result<(), ErrorTrace<ServeError>> {
        let response = SimpleOutgoingResponse::builder()
            .with_status(status_from_code(status))
            .add_header(SimpleHeader::LOCATION, location)
            .with_body(SendSafeBody::None)
            .build()
            .map_err(|e| ServeError::InternalError { status: 500, reason: e.to_string() })?;

        Http11::response(response)
            .http_render_to_writer(conn)
            .map_err(|e| ServeError::InternalError { status: 500, reason: e.to_string() })?;

        Ok(())
    }

    /// Write a 404 Not Found response.
    pub fn not_found(conn: &mut impl std::io::Write) -> Result<(), ErrorTrace<ServeError>> {
        text(conn, 404, "Not Found")
    }

    /// Write a 500 Internal Server Error response.
    pub fn server_error(
        conn: &mut impl std::io::Write,
        reason: Option<&str>,
    ) -> Result<(), ErrorTrace<ServeError>> {
        let body = reason.unwrap_or("Internal Server Error");
        text(conn, 500, body)
    }

    /// Write a 100 Continue interim response.
    #[tracing::instrument(skip(conn))]
    pub fn continue_100(conn: &mut impl std::io::Write) -> Result<(), ErrorTrace<ServeError>> {
        // A `100 Continue` interim response is header-less. The `Http11` renderer
        // allows that for informational (1xx) statuses (other statuses still
        // require at least one header) — see `Http11ResState::Headers`.
        tracing::trace!("Building 100 Continue response");
        let response = SimpleOutgoingResponse::builder()
            .with_status(status_from_code(100))
            .with_body(SendSafeBody::None)
            .build()
            .map_err(|e| ServeError::InternalError { status: 500, reason: e.to_string() })?;

        tracing::trace!("Rendering 100 Continue response to connection");
        Http11::response(response)
            .http_render_to_writer(conn)
            .map_err(|e| ServeError::InternalError { status: 500, reason: e.to_string() })?;

        // Flush so the interim response reaches the client immediately — the client
        // blocks reading it before it will send the request body, so any buffering
        // here would stall the whole exchange.
        conn.flush()
            .map_err(|e| ServeError::InternalError { status: 500, reason: e.to_string() })?;

        tracing::trace!("100 Continue response sent successfully");
        Ok(())
    }
}
