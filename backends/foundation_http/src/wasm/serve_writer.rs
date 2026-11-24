//! Wasm request dispatch — routes `SimpleIncomingRequest` through
//! middleware and handler, returning a structured `WasmResponse`.

use std::sync::Arc;

use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::{SendSafeBody, SimpleIncomingRequest};

use crate::shared::app::HttpApp;
use crate::shared::context::Ctx;
use crate::shared::middleware::MiddlewareResult;
use crate::shared::router::RouteOp;
use crate::shared::serve::{ConnectionResult, ServeError, ServeWriter};
use crate::wasm::response::WasmResponse;
use crate::wasm::stream::WasmStream;

/// The declared length of a sized request body, or `None` for a stream.
fn declared_body_len(body: &Option<SendSafeBody>) -> Option<usize> {
    match body {
        Some(SendSafeBody::Bytes(bytes)) => Some(bytes.len()),
        Some(SendSafeBody::Text(text)) => Some(text.len()),
        _ => None,
    }
}

/// A plain-text `WasmResponse` for a status.
fn status_response(status: u16, text: &str) -> WasmResponse {
    WasmResponse::new(status)
        .with_header("content-type", "text/plain")
        .with_body(text.as_bytes().to_vec())
}

/// Dispatch a request through the app's middleware and router,
/// returning a structured `WasmResponse` (status, body, headers).
pub fn handle_request(
    app: &HttpApp<Arc<dyn ServeWriter>>,
    req: SimpleIncomingRequest,
) -> Result<WasmResponse, ErrorTrace<ServeError>> {
    handle_request_with_bag(app.context().clone(), app, req)
}

/// Dispatch a request with a custom context ctx.
pub fn handle_request_with_bag(
    ctx: Ctx,
    app: &HttpApp<Arc<dyn ServeWriter>>,
    req: SimpleIncomingRequest,
) -> Result<WasmResponse, ErrorTrace<ServeError>> {
    let method = req.method.clone();
    // Route on the path, not the full URL, exactly as the web and Cloudflare
    // bridges do. The old form passed `request_url.url` (the whole URL) to the
    // router, so every writer route missed.
    let path = req.request_uri.path().to_string();

    // The in-memory writer path refuses a sized body above its configured
    // limit. A stream keeps the wasm bridge's own bound (spec 88 feature 02,
    // case 18).
    if let Some(len) = declared_body_len(&req.body) {
        if len > app.max_writer_body_bytes {
            return Ok(status_response(413, "Payload Too Large"));
        }
    }

    // Run middleware chain
    let (req, short_circuit) = run_middleware_writer(app, ctx.clone(), req);
    if let Some(response) = short_circuit {
        return Ok(response);
    }

    // Dispatch to router. A matched path with no handler for the method is a
    // 405; a path that matches nothing is a 404.
    let handler = match app.router().dispatch_result(&method, &path) {
        Ok(handler) => handler,
        Err(RouteOp::MethodNotAllowed) => return Ok(status_response(405, "Method Not Allowed")),
        Err(_) => return Ok(status_response(404, "Not Found")),
    };

    // Execute handler via serve_writer
    let mut stream = WasmStream::new();
    let result = handler.serve_writer(&ctx, req, &mut stream);

    match result {
        ConnectionResult::Keep | ConnectionResult::Take => Ok(stream.into_wasm_response()),
        ConnectionResult::Close(err) => Err(err.unwrap_or_else(|| {
            ServeError::InternalError {
                status: 500,
                reason: "handler closed connection".into(),
            }.into()
        })),
        // The wasm path is synchronous: it cannot drive an async continuation.
        ConnectionResult::Async(_) => Err(ServeError::InternalError {
            status: 500,
            reason: "a handler returned ConnectionResult::Async, which the synchronous wasm path cannot drive".into(),
        }.into()),
    }
}

/// Run the middleware chain, returning the (possibly modified) request and an optional
/// short-circuit response. If `Some(WasmResponse)`, the caller should return it immediately.
pub fn run_middleware_writer(
    app: &HttpApp<Arc<dyn ServeWriter>>,
    ctx: Ctx,
    mut req: SimpleIncomingRequest,
) -> (SimpleIncomingRequest, Option<WasmResponse>) {
    for mw in app.middleware_chain() {
        match mw.handle(&ctx, &mut req) {
            MiddlewareResult::Continue => {}
            MiddlewareResult::Response(response) => {
                let status_code: usize = response.status.clone().into();
                let body_bytes = match &response.body {
                    Some(SendSafeBody::Text(s)) => s.as_bytes().to_vec(),
                    Some(SendSafeBody::Bytes(b)) => b.clone(),
                    _ => Vec::new(),
                };
                return (
                    req,
                    Some(WasmResponse {
                        status: status_code as u16,
                        body: Some(body_bytes),
                        headers: response
                            .headers
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.join(", ")))
                            .collect(),
                    }),
                );
            }
            MiddlewareResult::InterimResponse(_) => {}
        }
    }

    (req, None)
}
