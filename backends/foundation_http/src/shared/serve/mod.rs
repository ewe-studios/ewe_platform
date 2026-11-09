//! Shared serve types — outcomes, errors and buffer-writer handlers.
//!
//! `ServeWriter` (both targets): takes `&mut dyn Write` for memory-backed or any
//! writable stream.
//!
//! The native `Serve` and `ServeFactory` traits take
//! `SharedByteBufferStream<RawStream>` for TCP connections. They are
//! native-only and live at [`crate::native::serve`]. The HTTP/2 and HTTP/3 serve
//! traits are native-only too, in the same module.

use std::future::Future;
use std::pin::Pin;

use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::{
    Http11, RenderHttp, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleOutgoingResponse,
    Status,
};

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

/// How a handler's async continuation ends. It has no `Async` variant: a
/// future resolves once, to one of the three connection actions.
pub enum ServeOutcome {
    /// The handler wrote a response; the connection carries the next request.
    Keep,
    /// The handler owns the connection permanently (WebSocket, SSE, long poll).
    Take,
    /// Close the connection, optionally with an error for the log.
    Close(Option<ErrorTrace<ServeError>>),
}

/// A handler's async continuation. It owns everything it needs (`'static`) and
/// resolves once to what the connection does next.
pub type ServeFuture = Pin<Box<dyn Future<Output = ServeOutcome> + Send + 'static>>;

/// Three outcomes telling the worker what to do after a handler runs.
pub enum ConnectionResult {
    /// Handler wrote a response; connection kept alive for next request.
    Keep,
    /// Handler took ownership permanently (WebSocket, SSE, long-poll).
    /// The worker loop exits immediately.
    Take,
    /// Close the connection, optionally carrying an error for logging.
    Close(Option<ErrorTrace<ServeError>>),
    /// The handler continues asynchronously. The connection task parks on the
    /// future and applies its result (Keep / Take / Close) when it completes.
    Async(ServeFuture),
}

impl From<ServeOutcome> for ConnectionResult {
    fn from(outcome: ServeOutcome) -> Self {
        match outcome {
            ServeOutcome::Keep => ConnectionResult::Keep,
            ServeOutcome::Take => ConnectionResult::Take,
            ServeOutcome::Close(err) => ConnectionResult::Close(err),
        }
    }
}

impl ConnectionResult {
    /// Split the result into the future to await, or the ready outcome.
    ///
    /// `Ok(future)` for `Async`, `Err(outcome)` for the three ready variants. A
    /// wrapper uses it to observe both cases without a four-arm match.
    ///
    /// # Errors
    /// Returns the ready outcome when the handler did not continue
    /// asynchronously.
    pub fn split(self) -> Result<ServeFuture, ServeOutcome> {
        match self {
            ConnectionResult::Async(future) => Ok(future),
            ConnectionResult::Keep => Err(ServeOutcome::Keep),
            ConnectionResult::Take => Err(ServeOutcome::Take),
            ConnectionResult::Close(err) => Err(ServeOutcome::Close(err)),
        }
    }
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
pub(crate) fn status_from_code(code: u16) -> Status {
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
        .map_err(|e| ServeError::InternalError {
            status: 500,
            reason: e.to_string(),
        })?;

    Http11::response(response)
        .http_render_to_writer(conn)
        .map_err(|e| ServeError::InternalError {
            status: 500,
            reason: e.to_string(),
        })?;

    Ok(())
}

/// Quick-response helpers for in-memory writers.
///
/// These write synchronously to any `impl Write` that cannot block: a `Vec<u8>`
/// in a test, or a wasm response buffer. They are **not** safe for a body larger
/// than the send buffer of a **non-blocking** connection socket: the write fails
/// with `WouldBlock` after a partial write. For any native `Serve` handler, use
/// `native::writer::send_text` / `send_json` / `send_bytes` instead; they write
/// through `ResponseWriter`. The name says which writers these are for.
pub mod respond_to_buffer {
    use serde::Serialize;

    use super::{
        render_response, status_from_code, ErrorTrace, Http11, RenderHttp, SendSafeBody,
        ServeError, SimpleHeader, SimpleOutgoingResponse,
    };

    /// Write a JSON response with the given status code.
    ///
    /// # Errors
    /// Returns an error when the body cannot be serialized, or the response
    /// cannot be built or written.
    pub fn json<T: Serialize>(
        conn: &mut impl std::io::Write,
        status: u16,
        body: &T,
    ) -> Result<(), ErrorTrace<ServeError>> {
        let bytes = serde_json::to_vec(body).map_err(|e| ServeError::InternalError {
            status: 500,
            reason: e.to_string(),
        })?;
        render_response(conn, status, "application/json", SendSafeBody::Bytes(bytes))
    }

    /// Write a plain text response.
    ///
    /// # Errors
    /// Returns an error when the response cannot be built or written.
    pub fn text(
        conn: &mut impl std::io::Write,
        status: u16,
        body: &str,
    ) -> Result<(), ErrorTrace<ServeError>> {
        render_response(conn, status, "text/plain", SendSafeBody::Text(body.into()))
    }

    /// Write an HTML response.
    ///
    /// # Errors
    /// Returns an error when the response cannot be built or written.
    pub fn html(
        conn: &mut impl std::io::Write,
        status: u16,
        body: &str,
    ) -> Result<(), ErrorTrace<ServeError>> {
        render_response(conn, status, "text/html", SendSafeBody::Text(body.into()))
    }

    /// Write a redirect response (301 or 302).
    ///
    /// # Errors
    /// Returns an error when the response cannot be built or written.
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
            .map_err(|e| ServeError::InternalError {
                status: 500,
                reason: e.to_string(),
            })?;

        Http11::response(response)
            .http_render_to_writer(conn)
            .map_err(|e| ServeError::InternalError {
                status: 500,
                reason: e.to_string(),
            })?;

        Ok(())
    }

    /// Write a 404 Not Found response.
    ///
    /// # Errors
    /// Returns an error when the response cannot be built or written.
    pub fn not_found(conn: &mut impl std::io::Write) -> Result<(), ErrorTrace<ServeError>> {
        text(conn, 404, "Not Found")
    }

    /// Write a 500 Internal Server Error response.
    ///
    /// # Errors
    /// Returns an error when the response cannot be built or written.
    pub fn server_error(
        conn: &mut impl std::io::Write,
        reason: Option<&str>,
    ) -> Result<(), ErrorTrace<ServeError>> {
        let body = reason.unwrap_or("Internal Server Error");
        text(conn, 500, body)
    }

    /// Write a 100 Continue interim response.
    ///
    /// # Errors
    /// Returns an error when the response cannot be built, written or flushed.
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
            .map_err(|e| ServeError::InternalError {
                status: 500,
                reason: e.to_string(),
            })?;

        tracing::trace!("Rendering 100 Continue response to connection");
        Http11::response(response)
            .http_render_to_writer(conn)
            .map_err(|e| ServeError::InternalError {
                status: 500,
                reason: e.to_string(),
            })?;

        // Flush so the interim response reaches the client immediately — the client
        // blocks reading it before it will send the request body, so any buffering
        // here would stall the whole exchange.
        conn.flush().map_err(|e| ServeError::InternalError {
            status: 500,
            reason: e.to_string(),
        })?;

        tracing::trace!("100 Continue response sent successfully");
        Ok(())
    }
}
