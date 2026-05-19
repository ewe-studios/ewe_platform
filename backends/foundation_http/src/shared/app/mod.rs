//! `HttpApp<S>` — application builder with router, context, and middleware.
//!
//! The handler type `S` flows through `Router<S>` down to `RouteSegment<S>`
//! and `RouteMethod<S>`. Register handlers using the appropriate method:
//! - `route` / `route_any` — native `Serve` handlers
//! - `route_writer` / `route_any_writer` — `ServeWriter` handlers
//! - `route_cf` / `route_any_cf` — Cloudflare Workers `CfServe` handlers
//! - `route_web` / `route_any_web` — browser `WebServe` handlers

use std::sync::Arc;

use foundation_core::wire::simple_http::SimpleMethod;
#[cfg(any(target_arch = "wasm32", feature = "wasm-test"))]
use foundation_core::wire::simple_http::SimpleIncomingRequest;

use crate::shared::context::ContextBag;
use crate::shared::middleware::RequestMiddleware;
use crate::shared::router::Router;
#[cfg(not(target_arch = "wasm32"))]
use crate::shared::serve::ServeFactory;
use crate::shared::serve::ServeWriterFactory;
#[cfg(any(target_arch = "wasm32", feature = "wasm-test"))]
use crate::wasm::serve_cf::{CfServe, CfServeFactory};
#[cfg(any(target_arch = "wasm32", feature = "wasm-test"))]
use crate::wasm::serve_web::{WebServe, WebServeFactory};

/// Top-level HTTP application builder, generic over handler type `S`.
pub struct HttpApp<S> {
    ctx: Arc<ContextBag>,
    router: Router<S>,
    middleware: Vec<Box<dyn RequestMiddleware>>,
}

impl<S> HttpApp<S> {
    /// Create a new empty `HttpApp`.
    #[must_use]
    pub fn new() -> Self
    where
        S: Default,
    {
        Self {
            ctx: Arc::new(ContextBag::new()),
            router: Router::new(),
            middleware: Vec::new(),
        }
    }

    /// Get a reference to the shared context bag.
    #[must_use]
    pub fn context(&self) -> &Arc<ContextBag> {
        &self.ctx
    }

    /// Add middleware to the application.
    pub fn middleware<M: RequestMiddleware>(&mut self, mw: M) -> &mut Self {
        self.middleware.push(Box::new(mw));
        self
    }

    /// Get a reference to the router.
    pub fn router(&self) -> &Router<S> {
        &self.router
    }

    /// Get a reference to the middleware chain.
    pub fn middleware_chain(&self) -> &[Box<dyn RequestMiddleware>] {
        &self.middleware
    }
}

// ---------------------------------------------------------------------------
// Native Serve handlers (cfg-gated, non-wasm only)

#[cfg(not(target_arch = "wasm32"))]
impl HttpApp<Arc<dyn crate::shared::serve::Serve>> {
    /// Create a new empty `HttpApp` for native Serve handlers.
    #[must_use]
    pub fn new_serve() -> Self {
        Self {
            ctx: Arc::new(ContextBag::new()),
            router: Router::new(),
            middleware: Vec::new(),
        }
    }

    /// Register a handler for a specific HTTP method and path.
    pub fn route<H: ServeFactory>(&mut self, method: SimpleMethod, path: &str) -> &mut Self {
        let handler = H::create(&self.ctx);
        self.router.add_route(method, path, Arc::new(handler));
        self
    }

    /// Register a handler for all HTTP methods on a path.
    pub fn route_any<H: ServeFactory>(&mut self, path: &str) -> &mut Self {
        let handler: Arc<dyn crate::shared::serve::Serve> = Arc::new(H::create(&self.ctx));
        self.router.add_route_any(path, &handler);
        self
    }

    /// Create an `HttpServer` from this app with default config.
    #[must_use]
    pub fn server(self, addr: &str) -> crate::native::server::HttpServer {
        crate::native::server::HttpServer::new(self, addr)
    }

    /// Create an `HttpServer` from this app with custom config.
    #[must_use]
    pub fn server_with_config(
        self,
        addr: &str,
        config: crate::native::server::ServerConfig,
    ) -> crate::native::server::HttpServer {
        crate::native::server::HttpServer::with_config(self, addr, config)
    }
}

// ---------------------------------------------------------------------------
// ServeWriter handlers (available on all targets)

impl HttpApp<Arc<dyn crate::shared::serve::ServeWriter>> {
    /// Create a new empty `HttpApp` for ServeWriter handlers.
    #[must_use]
    pub fn new_writer() -> Self {
        Self {
            ctx: Arc::new(ContextBag::new()),
            router: Router::new(),
            middleware: Vec::new(),
        }
    }

    /// Register a writer handler for a specific HTTP method and path.
    pub fn route_writer<H: ServeWriterFactory>(&mut self, method: SimpleMethod, path: &str) -> &mut Self {
        let handler = H::create(&self.ctx);
        self.router.add_route_writer(method, path, Arc::new(handler));
        self
    }

    /// Register a writer handler for all HTTP methods on a path.
    pub fn route_any_writer<H: ServeWriterFactory>(&mut self, path: &str) -> &mut Self {
        let handler: Arc<dyn crate::shared::serve::ServeWriter> = Arc::new(H::create(&self.ctx));
        self.router.add_route_any_writer(path, &handler);
        self
    }
}

// ---------------------------------------------------------------------------
// CfServe handlers (wasm only)

#[cfg(any(target_arch = "wasm32", feature = "wasm-test"))]
impl HttpApp<Arc<dyn CfServe>> {
    /// Create a new empty `HttpApp` for CfServe handlers.
    #[must_use]
    pub fn new_cf() -> Self {
        Self {
            ctx: Arc::new(ContextBag::new()),
            router: Router::new(),
            middleware: Vec::new(),
        }
    }

    /// Register a CF handler for a specific HTTP method and path.
    pub fn route_cf<H: CfServeFactory>(&mut self, method: SimpleMethod, path: &str) -> &mut Self {
        let handler = H::create(&self.ctx);
        self.router.add_route_cf(method, path, Arc::new(handler));
        self
    }

    /// Register a CF handler for all HTTP methods on a path.
    pub fn route_any_cf<H: CfServeFactory>(&mut self, path: &str) -> &mut Self {
        let handler: Arc<dyn CfServe> = Arc::new(H::create(&self.ctx));
        self.router.add_route_any_cf(path, &handler);
        self
    }

    /// Dispatch a request through middleware and router, returning a `web_sys::Response`.
    #[cfg(target_arch = "wasm32")]
    pub fn dispatch_cf(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
    ) -> Result<web_sys::Response, foundation_errstacks::ErrorTrace<crate::shared::serve::ServeError>> {
        use foundation_core::wire::simple_http::SendSafeBody;
        use crate::shared::middleware::MiddlewareResult;
        use crate::shared::serve::ServeError;
        use crate::wasm::cf_conn::CfConn;

        let method = req.method.clone();
        let path = req.request_url.url.clone();

        // Run middleware
        let mut req = req;
        for mw in &self.middleware {
            match mw.handle(&bag, &mut req) {
                MiddlewareResult::Continue => {}
                MiddlewareResult::Response(response) => {
                    let status_code: usize = response.status.clone().into();
                    let body_bytes = match &response.body {
                        Some(SendSafeBody::Text(s)) => s.as_bytes().to_vec(),
                        Some(SendSafeBody::Bytes(b)) => b.clone(),
                        _ => Vec::new(),
                    };
                    let mut conn = CfConn::new();
                    conn.set_status(status_code as u16);
                    for (k, vals) in &response.headers {
                        conn.set_header(&k.to_string(), &vals.join(", "));
                    }
                    conn.set_body(body_bytes);
                    return conn.into_response().map_err(|e| {
                        ServeError::InternalError { status: 500, reason: format!("{e:?}") }.into()
                    });
                }
                MiddlewareResult::InterimResponse(_) => {}
            }
        }

        // Dispatch to router
        let handler = self.router.dispatch(&method, &path)
            .ok_or_else(|| ServeError::BadRequest {
                status: 404,
                reason: format!("no route for {method:?} {path}"),
            })?;

        // Execute handler
        let mut conn = CfConn::new();
        let result = handler.serve_cf(bag, req, &mut conn);

        match result {
            crate::wasm::cf_conn::CfConnectionResult::Ok => {
                conn.into_response().map_err(|e| {
                    ServeError::InternalError { status: 500, reason: format!("{e:?}") }.into()
                })
            }
            crate::wasm::cf_conn::CfConnectionResult::Close(err) => Err(err.unwrap_or_else(|| {
                ServeError::InternalError {
                    status: 500,
                    reason: "handler closed connection".into(),
                }.into()
            })),
        }
    }
}

// ---------------------------------------------------------------------------
// WebServe handlers (wasm only)

#[cfg(any(target_arch = "wasm32", feature = "wasm-test"))]
impl HttpApp<Arc<dyn WebServe>> {
    /// Create a new empty `HttpApp` for WebServe handlers.
    #[must_use]
    pub fn new_web() -> Self {
        Self {
            ctx: Arc::new(ContextBag::new()),
            router: Router::new(),
            middleware: Vec::new(),
        }
    }

    /// Register a web handler for a specific HTTP method and path.
    pub fn route_web<H: WebServeFactory>(&mut self, method: SimpleMethod, path: &str) -> &mut Self {
        let handler = H::create(&self.ctx);
        self.router.add_route_web(method, path, Arc::new(handler));
        self
    }

    /// Register a web handler for all HTTP methods on a path.
    pub fn route_any_web<H: WebServeFactory>(&mut self, path: &str) -> &mut Self {
        let handler: Arc<dyn WebServe> = Arc::new(H::create(&self.ctx));
        self.router.add_route_any_web(path, &handler);
        self
    }

    /// Dispatch a request through middleware and router, returning a `web_sys::Response`.
    #[cfg(target_arch = "wasm32")]
    pub fn dispatch_web(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
    ) -> Result<web_sys::Response, foundation_errstacks::ErrorTrace<crate::shared::serve::ServeError>> {
        use foundation_core::wire::simple_http::SendSafeBody;
        use crate::shared::middleware::MiddlewareResult;
        use crate::shared::serve::ServeError;
        use crate::wasm::web_conn::WebConn;

        let method = req.method.clone();
        let path = req.request_url.url.clone();

        // Run middleware
        let mut req = req;
        for mw in &self.middleware {
            match mw.handle(&bag, &mut req) {
                MiddlewareResult::Continue => {}
                MiddlewareResult::Response(response) => {
                    let status_code: usize = response.status.clone().into();
                    let body_bytes = match &response.body {
                        Some(SendSafeBody::Text(s)) => s.as_bytes().to_vec(),
                        Some(SendSafeBody::Bytes(b)) => b.clone(),
                        _ => Vec::new(),
                    };
                    let mut conn = WebConn::new();
                    conn.set_status(status_code as u16);
                    for (k, vals) in &response.headers {
                        conn.set_header(&k.to_string(), &vals.join(", "));
                    }
                    conn.set_body(body_bytes);
                    return conn.into_response().map_err(|e| {
                        ServeError::InternalError { status: 500, reason: format!("{e:?}") }.into()
                    });
                }
                MiddlewareResult::InterimResponse(_) => {}
            }
        }

        // Dispatch to router (web)
        let handler = self.router.dispatch(&method, &path)
            .ok_or_else(|| ServeError::BadRequest {
                status: 404,
                reason: format!("no route for {method:?} {path}"),
            })?;

        // Execute handler
        let mut conn = WebConn::new();
        let result = handler.serve_web(bag, req, &mut conn);

        match result {
            crate::wasm::web_conn::WebConnectionResult::Ok => {
                conn.into_response().map_err(|e| {
                    ServeError::InternalError { status: 500, reason: format!("{e:?}") }.into()
                })
            }
            crate::wasm::web_conn::WebConnectionResult::Close(err) => Err(err.unwrap_or_else(|| {
                ServeError::InternalError {
                    status: 500,
                    reason: "handler closed connection".into(),
                }.into()
            })),
        }
    }
}

impl<S> Default for HttpApp<S>
where
    S: Default,
{
    fn default() -> Self {
        Self::new()
    }
}
