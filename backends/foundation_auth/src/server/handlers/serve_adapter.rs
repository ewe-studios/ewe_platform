use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::io::readers::Data;
use foundation_core::valtron::{collect_one, execute, from_future};
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve, ServeFactory, ServeError};
use foundation_http::{SimpleIncomingRequest, RawStream};
use foundation_netio::shared::http::{
    ChunkedData, Http11, RenderHttp, SimpleHeader, SimpleOutgoingResponse, SendSafeBody,
    Status,
};
use foundation_errstacks::ErrorTrace;

use super::super::config::IdpConfig;
use super::super::storage::HandlerStorage;
use foundation_db::{KeyValueStore, MemoryStorage};
use super::core::{HandlerResponse, IdpError, IdpHandlerCore};

/// Write JSON response with optional extra headers.
fn json_with_headers(
    conn: &mut impl std::io::Write,
    status: u16,
    body: &impl serde::Serialize,
    extra_headers: &[(String, String)],
) -> Result<(), ErrorTrace<ServeError>> {
    let bytes = serde_json::to_vec(body)
        .map_err(|e| ErrorTrace::new(ServeError::InternalError { status: 500, reason: e.to_string() }))?;

    let mut builder = SimpleOutgoingResponse::builder()
        .with_status(status_code(status))
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json");

    for (name, value) in extra_headers {
        builder = builder.add_header(name.to_string(), value.to_string());
    }

    let response = builder
        .with_body(SendSafeBody::Bytes(bytes))
        .build()
        .map_err(|e| ErrorTrace::new(ServeError::InternalError { status: 500, reason: e.to_string() }))?;

    Http11::response(response)
        .http_render_to_writer(conn)
        .map(|_| ())
        .map_err(|e| ErrorTrace::new(ServeError::InternalError { status: 500, reason: e.to_string() }))
}

fn status_code(code: u16) -> Status {
    match code {
        200 => Status::OK,
        202 => Status::Accepted,
        204 => Status::NoContent,
        206 => Status::PartialContent,
        302 => Status::Found,
        400 => Status::BadRequest,
        401 => Status::Unauthorized,
        403 => Status::Forbidden,
        404 => Status::NotFound,
        409 => Status::Conflict,
        413 => Status::Numbered(413, "Payload Too Large".into()),
        423 => Status::Numbered(423, "Locked".into()),
        429 => Status::TooManyRequests,
        500 => Status::InternalServerError,
        n => Status::Numbered(n as usize, String::new()),
    }
}

/// Serves the IdP routes over the handlers in [`IdpHandlerCore`].
///
/// `KV` is the cache type of the [`HandlerStorage`] the app's `ContextBag`
/// holds (as `Arc<HandlerStorage<KV>>`, which is how [`IdpServer::http_app`]
/// stores it).
///
/// [`IdpServer::http_app`]: crate::server::IdpServer::http_app
pub struct ServeAdapter<KV: KeyValueStore + Clone + 'static = MemoryStorage> {
    core: Arc<IdpHandlerCore<KV>>,
}

impl<KV: KeyValueStore + Clone + 'static> Serve for ServeAdapter<KV> {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        mut req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        // The handlers parse whole form/JSON bodies, so read the (lazily
        // streamed) body here, bounded by the configured limit.
        let limit = self.core.config().max_request_body_bytes;
        let result = match read_body_bounded(req.body.take(), limit) {
            Ok(bytes) => {
                req.body = Some(SendSafeBody::Bytes(bytes));
                run_sync(self.core.clone(), req)
            }
            Err(e) => Err(e),
        };

        match result {
            Ok(HandlerResponse { status, body, headers }) => {
                let err_body: serde_json::Value = body;
                match json_with_headers(&mut conn, status, &err_body, &headers) {
                    Ok(()) => ConnectionResult::Keep,
                    Err(e) => ConnectionResult::Close(Some(e)),
                }
            }
            Err(IdpError::BadRequest(msg)) => {
                let err = serde_json::json!({
                    "error": "bad_request",
                    "error_description": msg,
                });
                match json_with_headers(&mut conn, 400, &err, &[]) {
                    Ok(()) => ConnectionResult::Keep,
                    Err(e) => ConnectionResult::Close(Some(e)),
                }
            }
            Err(IdpError::Unauthorized(msg)) => {
                let err = serde_json::json!({
                    "error": "invalid_token",
                    "error_description": msg,
                });
                match json_with_headers(&mut conn, 401, &err, &[]) {
                    Ok(()) => ConnectionResult::Keep,
                    Err(e) => ConnectionResult::Close(Some(e)),
                }
            }
            Err(IdpError::Forbidden(msg)) => {
                let err = serde_json::json!({
                    "error": "forbidden",
                    "error_description": msg,
                });
                match json_with_headers(&mut conn, 403, &err, &[]) {
                    Ok(()) => ConnectionResult::Keep,
                    Err(e) => ConnectionResult::Close(Some(e)),
                }
            }
            Err(IdpError::NotFound(msg)) => {
                let err = serde_json::json!({
                    "error": "not_found",
                    "error_description": msg,
                });
                match json_with_headers(&mut conn, 404, &err, &[]) {
                    Ok(()) => ConnectionResult::Keep,
                    Err(e) => ConnectionResult::Close(Some(e)),
                }
            }
            Err(IdpError::Conflict(msg)) => {
                let err = serde_json::json!({
                    "error": "conflict",
                    "error_description": msg,
                });
                match json_with_headers(&mut conn, 409, &err, &[]) {
                    Ok(()) => ConnectionResult::Keep,
                    Err(e) => ConnectionResult::Close(Some(e)),
                }
            }
            Err(IdpError::Locked(retry_not_before)) => {
                let err = serde_json::json!({
                    "error": "account_locked",
                    "error_description": format!("Account locked until {retry_not_before}"),
                });
                let headers = vec![("x-retry-not-before".to_string(), retry_not_before.to_string())];
                match json_with_headers(&mut conn, 423, &err, &headers) {
                    Ok(()) => ConnectionResult::Keep,
                    Err(e) => ConnectionResult::Close(Some(e)),
                }
            }
            Err(IdpError::TooManyRequests(retry_not_before)) => {
                let err = serde_json::json!({
                    "error": "too_many_requests",
                    "error_description": format!("Retry after {retry_not_before}"),
                });
                let headers = vec![("x-retry-not-before".to_string(), retry_not_before.to_string())];
                match json_with_headers(&mut conn, 429, &err, &headers) {
                    Ok(()) => ConnectionResult::Keep,
                    Err(e) => ConnectionResult::Close(Some(e)),
                }
            }
            Err(IdpError::PayloadTooLarge(limit)) => {
                let err = serde_json::json!({
                    "error": "payload_too_large",
                    "error_description": format!("Request body larger than {limit} bytes"),
                });
                match json_with_headers(&mut conn, 413, &err, &[]) {
                    Ok(()) => ConnectionResult::Close(None),
                    Err(e) => ConnectionResult::Close(Some(e)),
                }
            }
            Err(IdpError::Internal(msg)) => {
                let err = serde_json::json!({
                    "error": "server_error",
                    "error_description": msg,
                });
                match json_with_headers(&mut conn, 500, &err, &[]) {
                    Ok(()) => ConnectionResult::Keep,
                    Err(e) => ConnectionResult::Close(Some(e)),
                }
            }
        }
    }
}

impl<KV: KeyValueStore + Clone + 'static> ServeFactory for ServeAdapter<KV> {
    /// # Panics
    ///
    /// If the `ContextBag` holds no `IdpConfig` or no `Arc<HandlerStorage<KV>>`
    /// (a misconfigured app; serving without storage would answer every
    /// request as if the database were empty).
    fn create(bag: &ContextBag) -> Self {
        let config = bag
            .get::<IdpConfig>()
            .expect("IdpConfig must be in ContextBag");
        // The bag keys values by type and wraps each in its own `Arc`, so the
        // stored `Arc<HandlerStorage<KV>>` comes back as `Arc<Arc<…>>`.
        let storage = bag
            .get::<Arc<HandlerStorage<KV>>>()
            .map(|stored| Arc::clone(&*stored))
            .expect("Arc<HandlerStorage<KV>> must be in ContextBag");
        Self {
            core: Arc::new(IdpHandlerCore::new(config, storage)),
        }
    }
}

/// `Serve::serve` is sync (`foundation_http`'s trait), so this is a sync
/// boundary: it runs the async dispatch on valtron and waits for its result.
fn run_sync<KV: KeyValueStore + Clone + 'static>(
    core: Arc<IdpHandlerCore<KV>>,
    req: SimpleIncomingRequest,
) -> Result<HandlerResponse, IdpError> {
    let task = from_future(async move {
        let bag = ContextBag::new();
        core.dispatch(&bag, &req).await
    });
    let stream = execute(task, None)
        .map_err(|e| IdpError::Internal(format!("valtron: {e}")))?;
    // The handler's own `IdpError` is the response; only a missing result is
    // an executor failure.
    collect_one(stream).ok_or_else(|| IdpError::Internal("future produced no result".into()))?
}

/// Read a request body into memory, refusing more than `limit` bytes.
///
/// IdP bodies are small forms and JSON documents the handlers parse whole.
/// The streamed variants are read chunk by chunk and abandoned as soon as the
/// limit is passed.
fn read_body_bounded(body: Option<SendSafeBody>, limit: usize) -> Result<Vec<u8>, IdpError> {
    fn push(buf: &mut Vec<u8>, chunk: &[u8], limit: usize) -> Result<(), IdpError> {
        if buf.len() + chunk.len() > limit {
            return Err(IdpError::PayloadTooLarge(limit));
        }
        buf.extend_from_slice(chunk);
        Ok(())
    }
    let read_err = |e: &dyn std::fmt::Display| IdpError::BadRequest(format!("failed to read request body: {e}"));

    let mut buf = Vec::new();
    match body {
        None | Some(SendSafeBody::None) => {}
        Some(SendSafeBody::Text(t)) => push(&mut buf, t.as_bytes(), limit)?,
        Some(SendSafeBody::Bytes(b)) => push(&mut buf, &b, limit)?,
        Some(SendSafeBody::Stream(mut iter)) => {
            for item in iter.take().into_iter().flatten() {
                match item.map_err(|e| read_err(&e))? {
                    Data::Bytes(bytes) => push(&mut buf, &bytes, limit)?,
                    Data::Retry => {}
                }
            }
        }
        Some(SendSafeBody::ChunkedStream(mut iter)) => {
            for item in iter.take().into_iter().flatten() {
                match item.map_err(|e| read_err(&e))? {
                    ChunkedData::Data(bytes, _) => push(&mut buf, &bytes, limit)?,
                    ChunkedData::Trailers(_) | ChunkedData::DataEnded => {}
                }
            }
        }
        Some(SendSafeBody::LineFeedStream(_) | SendSafeBody::SseStream(_)) => {
            return Err(IdpError::BadRequest("unsupported request body encoding".into()));
        }
    }
    Ok(buf)
}
