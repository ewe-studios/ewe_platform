//! Error middleware — centralized `ConnectionResult::Close` handling.
//!
//! Provides consistent error response rendering for debug/release modes.

use foundation_core::wire::simple_http::{
    Http11, RenderHttp, SimpleHeader, SimpleOutgoingResponse, SendSafeBody, Status,
};
use foundation_errstacks::ErrorTrace;

use crate::shared::serve::ServeError;

/// Handle a `ConnectionResult::Close` by logging and optionally rendering an error response.
#[allow(dead_code)]
pub fn handle_close(
    conn: &mut impl std::io::Write,
    err: Option<ErrorTrace<ServeError>>,
    debug_mode: bool,
) {
    let Some(err) = err else {
        return; // Silent close
    };

    let serve_error = err.downcast_ref::<ServeError>();
    let (status, reason) = match serve_error {
        Some(ServeError::BadRequest { status, reason } | ServeError::InternalError {
status, reason }) => (*status, reason.clone()),
        None => (500, format!("{err}")),
    };

    tracing::error!("Connection close: {reason}");

    let body = if debug_mode {
        format!("{status} {reason}")
    } else {
        match status {
            400..=499 => "Bad Request".into(),
            _ => "Internal Server Error".into(),
        }
    };

    let response = SimpleOutgoingResponse::builder()
        .with_status(match status {
            400 => Status::BadRequest,
            401 => Status::Unauthorized,
            403 => Status::Forbidden,
            404 => Status::NotFound,
            429 => Status::TooManyRequests,
            500 => Status::InternalServerError,
            503 => Status::ServiceUnavailable,
            n => Status::Numbered(n as usize, String::new()),
        })
        .add_header(SimpleHeader::CONTENT_TYPE, "text/plain")
        .with_body(SendSafeBody::Text(body))
        .build()
        .expect("valid error response");

    if let Err(e) = Http11::response(response).http_render_to_writer(conn) {
        tracing::error!("Failed to render error response: {e}");
    }
}

/// `ErrorMiddleware` — logs and handles connection errors uniformly.
pub struct ErrorMiddleware {
    pub debug_mode: bool,
}

impl ErrorMiddleware {
    #[must_use]
    pub fn new(debug_mode: bool) -> Self {
        Self { debug_mode }
    }
}
