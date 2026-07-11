use std::future::Future;
use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve, ServeFactory, ServeError};
use foundation_http::{SimpleIncomingRequest, RawStream};
use foundation_netio::shared::http::{
    Http11, RenderHttp, SimpleHeader, SimpleOutgoingResponse, SendSafeBody,
    Status,
};
use foundation_errstacks::ErrorTrace;

use super::super::config::IdpConfig;
use super::super::storage::HandlerStorage;
use foundation_db::MemoryStorage;
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
        423 => Status::Numbered(423, "Locked".into()),
        429 => Status::TooManyRequests,
        500 => Status::InternalServerError,
        n => Status::Numbered(n as usize, String::new()),
    }
}

pub struct ServeAdapter {
    core: Arc<IdpHandlerCore>,
}

impl Serve for ServeAdapter {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let result = run_sync(self.core.clone(), req);

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

impl ServeFactory for ServeAdapter {
    fn create(bag: &ContextBag) -> Self {
        let config = bag
            .get::<IdpConfig>()
            .expect("IdpConfig must be in ContextBag");
        let storage = bag
            .get::<HandlerStorage<MemoryStorage>>()
            .unwrap_or_else(|| {
                // Fallback for tests that don't have storage set up yet
                Arc::new(HandlerStorage::new(
                    Arc::new(NoOpStore),
                    MemoryStorage::new(),
                ))
            });
        Self {
            core: Arc::new(IdpHandlerCore::new(config, storage)),
        }
    }
}

// No-op QueryStore for test fallback
struct NoOpStore;
impl foundation_db::core::storage_provider::QueryStore for NoOpStore {
    fn query(
        &self,
        _sql: &str,
        _params: &[foundation_db::core::storage_provider::DataValue],
    ) -> Result<foundation_db::core::storage_provider::StorageItemStream<'_, foundation_db::core::storage_provider::SqlRow>, foundation_db::core::errors::StorageError> {
        Ok(Box::new(std::iter::empty()))
    }
    fn execute(
        &self,
        _sql: &str,
        _params: &[foundation_db::core::storage_provider::DataValue],
    ) -> Result<u64, foundation_db::core::errors::StorageError> {
        Ok(0)
    }
    fn execute_batch(
        &self,
        _sql: &str,
    ) -> Result<(), foundation_db::core::errors::StorageError> {
        Ok(())
    }
}

fn run_sync(
    core: Arc<IdpHandlerCore>,
    req: SimpleIncomingRequest,
) -> Result<HandlerResponse, IdpError> {
    let bag = ContextBag::new();
    let fut = async move { core.dispatch(&bag, &req).await };
    let mut fut = core::pin::pin!(fut);
    let waker = noop_waker();
    let mut cx = core::task::Context::from_waker(&waker);
    match fut.as_mut().poll(&mut cx) {
        core::task::Poll::Ready(result) => result,
        core::task::Poll::Pending => Err(IdpError::Internal("Async operation pending".into())),
    }
}

fn noop_waker() -> core::task::Waker {
    use core::task::{RawWaker, RawWakerVTable};
    fn no_op(_: *const ()) {}
    fn clone(p: *const ()) -> RawWaker {
        RawWaker::new(p, &VTABLE)
    }
    const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
    unsafe { core::task::Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
}
