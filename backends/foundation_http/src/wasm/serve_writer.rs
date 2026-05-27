//! Wasm request dispatch — routes `SimpleIncomingRequest` through
//! middleware and handler, returning a structured `WasmResponse`.

use std::sync::Arc;

use foundation_netio::simple_http::shared::{
    SendSafeBody, SimpleIncomingRequest,
};
use foundation_errstacks::ErrorTrace;

use crate::shared::app::HttpApp;
use crate::shared::context::ContextBag;
use crate::shared::middleware::MiddlewareResult;
use crate::shared::serve::{ConnectionResult, ServeError, ServeWriter};
use crate::wasm::response::WasmResponse;
use crate::wasm::stream::WasmStream;

/// Dispatch a request through the app's middleware and router,
/// returning a structured `WasmResponse` (status, body, headers).
pub fn handle_request(
    app: &HttpApp<Arc<dyn ServeWriter>>,
    req: SimpleIncomingRequest,
) -> Result<WasmResponse, ErrorTrace<ServeError>> {
    handle_request_with_bag(app.context().clone(), app, req)
}

/// Dispatch a request with a custom context bag.
pub fn handle_request_with_bag(
    bag: Arc<ContextBag>,
    app: &HttpApp<Arc<dyn ServeWriter>>,
    req: SimpleIncomingRequest,
) -> Result<WasmResponse, ErrorTrace<ServeError>> {
    let method = req.method.clone();
    let path = req.request_url.url.clone();

    // Run middleware chain
    let (req, short_circuit) = run_middleware_writer(app, bag.clone(), req);
    if let Some(response) = short_circuit {
        return Ok(response);
    }

    // Dispatch to router
    let handler = app.router().dispatch(&method, &path)
        .ok_or_else(|| ServeError::BadRequest {
            status: 404,
            reason: format!("no route for {method:?} {path}"),
        })?;

    // Execute handler via serve_writer
    let mut stream = WasmStream::new();
    let result = handler.serve_writer(&bag, req, &mut stream);

    match result {
        ConnectionResult::Keep | ConnectionResult::Take => Ok(stream.into_wasm_response()),
        ConnectionResult::Close(err) => Err(err.unwrap_or_else(|| {
            ServeError::InternalError {
                status: 500,
                reason: "handler closed connection".into(),
            }.into()
        })),
    }
}

/// Run the middleware chain, returning the (possibly modified) request and an optional
/// short-circuit response. If `Some(WasmResponse)`, the caller should return it immediately.
pub fn run_middleware_writer(
    app: &HttpApp<Arc<dyn ServeWriter>>,
    bag: Arc<ContextBag>,
    mut req: SimpleIncomingRequest,
) -> (SimpleIncomingRequest, Option<WasmResponse>) {
    for mw in app.middleware_chain() {
        match mw.handle(&bag, &mut req) {
            MiddlewareResult::Continue => {}
            MiddlewareResult::Response(response) => {
                let status_code: usize = response.status.clone().into();
                let body_bytes = match &response.body {
                    Some(SendSafeBody::Text(s)) => s.as_bytes().to_vec(),
                    Some(SendSafeBody::Bytes(b)) => b.clone(),
                    _ => Vec::new(),
                };
                return (req, Some(WasmResponse {
                    status: status_code as u16,
                    body: Some(body_bytes),
                    headers: response.headers.iter().map(|(k, v)| {
                        (k.to_string(), v.join(", "))
                    }).collect(),
                }));
            }
            MiddlewareResult::InterimResponse(_) => {}
        }
    }

    (req, None)
}
