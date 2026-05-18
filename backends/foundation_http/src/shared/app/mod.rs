//! `HttpApp` — application builder with router, context, and middleware.

use std::sync::Arc;

use foundation_core::wire::simple_http::SimpleMethod;

use crate::shared::context::ContextBag;
use crate::shared::middleware::RequestMiddleware;
use crate::shared::router::Router;
use crate::shared::serve::{ServeWriter, ServeWriterFactory};
#[cfg(not(target_arch = "wasm32"))]
use crate::shared::serve::ServeFactory;

/// Top-level HTTP application builder.
pub struct HttpApp {
    ctx: Arc<ContextBag>,
    router: Router,
    middleware: Vec<Box<dyn RequestMiddleware>>,
}

impl HttpApp {
    /// Create a new empty `HttpApp`.
    #[must_use]
    pub fn new() -> Self {
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

    /// Register a handler for a specific HTTP method and path.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn route<H: ServeFactory>(&mut self, method: SimpleMethod, path: &str) -> &mut Self {
        let handler = H::create(&self.ctx);
        self.router.add_route(method, path, Arc::new(handler));
        self
    }

    /// Register a handler for all HTTP methods on a path.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn route_any<H: ServeFactory>(&mut self, path: &str) -> &mut Self {
        let handler: Arc<dyn crate::shared::serve::Serve> = Arc::new(H::create(&self.ctx));
        self.router.add_route_any(path, &handler);
        self
    }

    /// Register a writer handler for a specific HTTP method and path.
    pub fn route_writer<H: ServeWriterFactory>(&mut self, method: SimpleMethod, path: &str) -> &mut Self {
        let handler = H::create(&self.ctx);
        self.router.add_route_writer(method, path, Arc::new(handler));
        self
    }

    /// Register a writer handler for all HTTP methods on a path.
    pub fn route_any_writer<H: ServeWriterFactory>(&mut self, path: &str) -> &mut Self {
        let handler: Arc<dyn ServeWriter> = Arc::new(H::create(&self.ctx));
        self.router.add_route_any_writer(path, &handler);
        self
    }

    /// Add middleware to the application.
    pub fn middleware<M: RequestMiddleware>(&mut self, mw: M) -> &mut Self {
        self.middleware.push(Box::new(mw));
        self
    }

    /// Get a reference to the router.
    pub(crate) fn router(&self) -> &Router {
        &self.router
    }

    /// Get a reference to the middleware chain.
    pub(crate) fn middleware_chain(&self) -> &[Box<dyn RequestMiddleware>] {
        &self.middleware
    }

    /// Create an `HttpServer` from this app with default config.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn server(self, addr: &str) -> crate::native::server::HttpServer {
        crate::native::server::HttpServer::new(self, addr)
    }

    /// Create an `HttpServer` from this app with custom config.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn server_with_config(
        self,
        addr: &str,
        config: crate::native::server::ServerConfig,
    ) -> crate::native::server::HttpServer {
        crate::native::server::HttpServer::with_config(self, addr, config)
    }
}

impl Default for HttpApp {
    fn default() -> Self {
        Self::new()
    }
}
