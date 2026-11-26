//! The Worker's Connect adapter: a `CfServe` handler over the connectrpc
//! router (spec 83 feature 05, section 5.4).
//!
//! WHY: `foundation_http` owns the router, the middleware and the Cloudflare
//! bridge. This module adds the one adapter between that stack and the
//! connectrpc handler. The response is a `ReadableStream`, so a turn streams
//! as the model makes events.
//!
//! WHAT: [`ServedAgent`] (the handler and its limits, stored in the `Ctx`),
//! [`IncomingBody`] (the call body, moved from the JavaScript request),
//! [`incoming_from_web_request`] and [`ConnectService`].
//!
//! HOW: the entry builds the request with the forwarded headers removed and
//! the body as an async [`RequestBody`]. The adapter spawns the dispatch and
//! returns as soon as the handler writes the response head; the body keeps
//! flowing. A dropped response drops the body reader, which ends the handler.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use foundation_connectrpc::{
    request_body_from_stream, CallHooks, ConnectRpcHandler, ConnectSink, RequestBody,
};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use foundation_http::shared::serve::ServeError;
use foundation_http::wasm::cf_conn::{CfConn, CfConnectionResult};
use foundation_http::wasm::serve_cf::{CfServe, CfServeFactory};
use foundation_netio::shared::http::{
    Proto, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};
use wasm_bindgen::JsError;
use wasm_bindgen_futures::spawn_local;

use crate::host::worker::config::ServeLimits;

/// The header prefix that a client must not set.
pub const FORWARDED_HEADER_PREFIX: &str = "ewe-forwarded-";

/// The service and the limits that one served call reads.
///
/// The entry stores it in the request `Ctx`; a route factory receives only the
/// `Ctx`, so this is where one request's service is found.
pub struct ServedAgent {
    /// The Connect router of this host.
    pub handler: Arc<ConnectRpcHandler>,
    /// The call limits.
    pub limits: ServeLimits,
}

impl std::fmt::Debug for ServedAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServedAgent")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

/// The request body of one call, taken out of the request extensions.
///
/// `RequestBody` is `Send` but not `Sync`; the mutex makes the holder `Sync`
/// so it can travel in the request extensions.
pub struct IncomingBody(Mutex<Option<RequestBody<'static>>>);

impl std::fmt::Debug for IncomingBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IncomingBody").finish_non_exhaustive()
    }
}

impl IncomingBody {
    /// A holder for `body`.
    #[must_use]
    pub fn new(body: RequestBody<'static>) -> Self {
        Self(Mutex::new(Some(body)))
    }

    /// Take the body. A second call gets `None`.
    fn take(&self) -> Option<RequestBody<'static>> {
        self.0.lock().ok().and_then(|mut slot| slot.take())
    }
}

/// Build a `SimpleIncomingRequest` from a JavaScript request.
///
/// The body is not read here: it becomes an async [`RequestBody`] in the
/// request extensions, so the handler pulls one bounded chunk at a time. When
/// `filter_forwarded` is true, every `ewe-forwarded-*` header is dropped, so a
/// caller cannot claim an identity (invariant I4).
///
/// # Errors
/// The `JsError` that the runtime raised while reading the request.
pub fn incoming_from_web_request(
    req: &web_sys::Request,
    limits: &ServeLimits,
    filter_forwarded: bool,
) -> Result<SimpleIncomingRequest, JsError> {
    let url = req.url();
    let request_uri = foundation_http::wasm::bridge::parse_request_uri(&url)
        .map_err(|error| JsError::new(&error))?;
    let method = SimpleMethod::from(req.method());

    let entries = js_sys::Array::from(&req.headers().entries());
    let mut headers = SimpleHeaders::new();
    for entry in entries.iter() {
        let pair = js_sys::Array::from(&entry);
        if let (Some(key), Some(value)) = (pair.get(0).as_string(), pair.get(1).as_string()) {
            if filter_forwarded
                && key
                    .to_ascii_lowercase()
                    .starts_with(FORWARDED_HEADER_PREFIX)
            {
                continue;
            }
            headers
                .entry(SimpleHeader::from(key))
                .or_default()
                .push(value);
        }
    }

    let mut extensions = foundation_netio::shared::http::Extensions::default();
    if let Some(stream) = req.body() {
        extensions.insert(IncomingBody::new(request_body_from_stream(
            stream,
            limits.response_queue,
            Some(limits.max_request_bytes),
        )));
    }

    SimpleIncomingRequest::builder()
        .with_parsed_url(url)
        .with_uri(request_uri)
        .with_method(method)
        .with_proto(Proto::HTTP11)
        .with_headers(headers)
        .with_extensions(extensions)
        .build()
        .map_err(|error| JsError::new(&format!("failed to build the request: {error}")))
}

/// The liveness route of the Worker and the instance Durable Object.
pub struct HealthCheck;

impl CfServeFactory for HealthCheck {
    fn create(_ctx: &Ctx) -> Self {
        Self
    }
}

#[async_trait(?Send)]
impl CfServe for HealthCheck {
    async fn serve_cf(
        &self,
        _ctx: Ctx,
        _req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult {
        conn.set_status(200);
        conn.set_header("content-type", "text/plain; charset=utf-8");
        conn.set_body(b"ok".to_vec());
        CfConnectionResult::Ok
    }
}

/// The Connect service of the Worker or the instance Durable Object.
pub struct ConnectService;

impl CfServeFactory for ConnectService {
    fn create(_ctx: &Ctx) -> Self {
        Self
    }
}

#[async_trait(?Send)]
impl CfServe for ConnectService {
    async fn serve_cf(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult {
        let Some(service) = ctx.get::<ServedAgent>() else {
            conn.set_status(500);
            conn.set_header("content-type", "text/plain; charset=utf-8");
            conn.set_body(b"the Ctx holds no served agent".to_vec());
            return CfConnectionResult::Ok;
        };

        let body = req
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get::<IncomingBody>())
            .and_then(IncomingBody::take)
            .unwrap_or_else(|| Box::pin(futures::stream::empty()));

        let (sink, handle) = ConnectSink::new_pair_with_capacity(service.limits.response_queue);
        let handler = Arc::clone(&service.handler);
        // `ChatStream` streams its reply: run the dispatch as its own task and
        // return the head at once, so each frame follows. A unary call runs
        // inline; its whole response is ready before `serve_cf` returns, and a
        // call that forwards to a Durable Object must not nest two tasks.
        let streaming = req.request_uri.path().ends_with("/ChatStream");
        if streaming {
            spawn_local(async move {
                if let Err(error) = handler
                    .dispatch_to(ctx, req, body, Box::new(sink), CallHooks::default())
                    .await
                {
                    tracing::debug!(%error, "the Connect sink closed the call");
                }
            });
        } else if let Err(error) = handler
            .dispatch_to(ctx, req, body, Box::new(sink), CallHooks::default())
            .await
        {
            tracing::debug!(%error, "the Connect sink closed the call");
        }

        let response = if streaming {
            handle.wait_response().await
        } else {
            handle.into_response()
        };
        match response {
            Some(response) => conn.set_response(response),
            None => {
                // The handler ended before it opened a stream and wrote no
                // whole response either. Report an internal error.
                let error = ServeError::InternalError {
                    status: 500,
                    reason: "the Connect handler wrote no response".to_string(),
                };
                return CfConnectionResult::Close(Some(ErrorTrace::new(error)));
            }
        }
        CfConnectionResult::Ok
    }
}
