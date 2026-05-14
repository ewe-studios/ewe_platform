//! Serve trait — core handler abstraction.
//!
//! All handlers implement `Serve`, which provides:
//! - `create(bag) -> Self` — instantiation during route registration
//! - `serve(bag, req, conn) -> ConnectionResult` — request handling

use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::netcap::RawStream;
use foundation_core::wire::simple_http::{
    Http11, RenderHttp, SimpleIncomingRequest, SimpleOutgoingResponse, Status,
    SimpleHeader, SendSafeBody,
};
use foundation_errstacks::ErrorTrace;

use crate::context::ContextBag;

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
// Serve trait

/// Core handler trait — executed at request time.
pub trait Serve: Send + Sync + 'static {
    /// Handle an incoming request.
    fn serve(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult;
}

/// Factory trait for creating handler instances.
///
/// Separated from `Serve` to make `dyn Serve` dyn-compatible.
pub trait ServeFactory: Serve + Sized {
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
    ///
    /// # Errors
    ///
    /// Returns `ServeError::InternalError` if the body cannot be serialized
    /// or the response cannot be rendered.
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
    ///
    /// # Errors
    ///
    /// Returns `ServeError::InternalError` if the response cannot be rendered.
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
    ///
    /// Returns `ServeError::InternalError` if the response cannot be rendered.
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
    ///
    /// Returns `ServeError::InternalError` if the response cannot be rendered.
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
    ///
    /// # Errors
    ///
    /// Returns `ServeError::InternalError` if the response cannot be rendered.
    pub fn not_found(conn: &mut impl std::io::Write) -> Result<(), ErrorTrace<ServeError>> {
        text(conn, 404, "Not Found")
    }

    /// Write a 500 Internal Server Error response.
    ///
    /// # Errors
    ///
    /// Returns `ServeError::InternalError` if the response cannot be rendered.
    pub fn server_error(
        conn: &mut impl std::io::Write,
        reason: Option<&str>,
    ) -> Result<(), ErrorTrace<ServeError>> {
        let body = reason.unwrap_or("Internal Server Error");
        text(conn, 500, body)
    }

    /// Write a 100 Continue interim response.
    ///
    /// Used for HTTP/1.1 Expect: 100-continue handling. The client sends
    /// request headers with `Expect: 100-continue` and waits for this
    /// response before sending the request body.
    ///
    /// # Errors
    ///
    /// Returns `ServeError::InternalError` if the response cannot be rendered.
    #[tracing::instrument(skip(conn))]
    pub fn continue_100(conn: &mut impl std::io::Write) -> Result<(), ErrorTrace<ServeError>> {
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

        tracing::trace!("100 Continue response rendered successfully");
        Ok(())
    }
}
