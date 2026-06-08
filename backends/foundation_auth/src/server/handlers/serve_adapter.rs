use std::future::Future;
use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve, ServeFactory, respond};
use foundation_http::{SimpleIncomingRequest, RawStream};

use super::super::config::IdpConfig;
use super::core::{IdpError, IdpHandlerCore};

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
            Ok(body) => match respond::json(&mut conn, 200, &body) {
                Ok(()) => ConnectionResult::Keep,
                Err(e) => ConnectionResult::Close(Some(e)),
            },
            Err(IdpError::BadRequest(msg)) => {
                let err = serde_json::json!({
                    "error": "bad_request",
                    "error_description": msg,
                });
                match respond::json(&mut conn, 400, &err) {
                    Ok(()) => ConnectionResult::Keep,
                    Err(e) => ConnectionResult::Close(Some(e)),
                }
            }
            Err(IdpError::Unauthorized(msg)) => {
                let err = serde_json::json!({
                    "error": "invalid_token",
                    "error_description": msg,
                });
                match respond::json(&mut conn, 401, &err) {
                    Ok(()) => ConnectionResult::Keep,
                    Err(e) => ConnectionResult::Close(Some(e)),
                }
            }
            Err(IdpError::Internal(msg)) => {
                let err = serde_json::json!({
                    "error": "server_error",
                    "error_description": msg,
                });
                match respond::json(&mut conn, 500, &err) {
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
        Self {
            core: Arc::new(IdpHandlerCore::new(config)),
        }
    }
}

fn run_sync(
    core: Arc<IdpHandlerCore>,
    req: SimpleIncomingRequest,
) -> Result<serde_json::Value, IdpError> {
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
