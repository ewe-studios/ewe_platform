//! Wasm request dispatch — routes `SimpleIncomingRequest` through
//! middleware and handler, collecting response bytes.

use std::sync::Arc;

use foundation_core::wire::simple_http::{RenderHttp, SimpleIncomingRequest};
use foundation_errstacks::ErrorTrace;

use crate::shared::app::HttpApp;
use crate::shared::context::ContextBag;
use crate::shared::middleware::MiddlewareResult;
use crate::shared::serve::{ConnectionResult, ServeError};
use crate::wasm::stream::WasmStream;

/// Dispatch a request through the app's middleware and router,
/// returning the response bytes.
///
/// # Errors
///
/// Returns a `ServeError` if no route matches or a middleware/handler error occurs.
pub fn handle_request(
    app: &HttpApp,
    req: SimpleIncomingRequest,
) -> Result<Vec<u8>, ErrorTrace<ServeError>> {
    handle_request_with_bag(app.context().clone(), app, req)
}

/// Dispatch a request with a custom context bag.
///
/// # Errors
///
/// Returns a `ServeError` if no route matches or a middleware/handler error occurs.
pub fn handle_request_with_bag(
    bag: Arc<ContextBag>,
    app: &HttpApp,
    req: SimpleIncomingRequest,
) -> Result<Vec<u8>, ErrorTrace<ServeError>> {
    let method = req.method.clone();
    let path = req.request_url.url.clone();

    // Run middleware chain
    let req = run_middleware(app, bag.clone(), req)?;

    // Dispatch to router
    let server = app.router().dispatch(&method, &path)
        .ok_or_else(|| ServeError::BadRequest {
            status: 404,
            reason: format!("no route for {method:?} {path}"),
        })?;

    // Execute handler
    let Some(writer) = server.as_writer() else {
        return Err(ServeError::InternalError {
            status: 500,
            reason: "handler is Serve (native-only), not ServeWriter".into(),
        }.into());
    };

    let mut stream = WasmStream::new();
    let result = writer.serve_writer(&bag, req, &mut stream);

    match result {
        ConnectionResult::Keep | ConnectionResult::Take => Ok(stream.into_bytes()),
        ConnectionResult::Close(err) => Err(err.unwrap_or_else(|| {
            ServeError::InternalError {
                status: 500,
                reason: "handler closed connection".into(),
            }.into()
        })),
    }
}

/// Run the middleware chain, returning the (possibly modified) request.
fn run_middleware(
    app: &HttpApp,
    bag: Arc<ContextBag>,
    mut req: SimpleIncomingRequest,
) -> Result<SimpleIncomingRequest, ErrorTrace<ServeError>> {
    for mw in app.middleware_chain() {
        match mw.handle(&bag, &mut req) {
            MiddlewareResult::Continue => {}
            MiddlewareResult::Response(response) => {
                // Middleware short-circuited — render response directly
                let mut stream = WasmStream::new();
                foundation_core::wire::simple_http::Http11::response(response)
                    .http_render_to_writer(&mut stream)
                    .map_err(|e| ServeError::InternalError { status: 500, reason: e.to_string() })?;
                return Ok(req);
            }
            MiddlewareResult::InterimResponse(_) => {}
        }
    }

    Ok(req)
}
