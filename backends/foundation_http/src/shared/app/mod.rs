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

use crate::shared::context::ContextBag;
use crate::shared::middleware::RequestMiddleware;
use crate::shared::router::Router;
#[cfg(not(target_arch = "wasm32"))]
use crate::shared::serve::ServeFactory;
use crate::shared::serve::ServeWriterFactory;

/// Top-level HTTP application builder, generic over handler type `S`.
pub struct HttpApp<S> {
    pub ctx: Arc<ContextBag>,
    pub router: Router<S>,
    pub middleware: Vec<Box<dyn RequestMiddleware>>,
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

impl<S> Default for HttpApp<S>
where
    S: Default,
{
    fn default() -> Self {
        Self::new()
    }
}
