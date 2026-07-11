//! `HttpApp<S>` — application builder with router, context, and middleware.
//!
//! The handler type `S` flows through `Router<S>` down to `RouteSegment<S>`
//! and `RouteMethod<S>`. Register handlers using the appropriate method:
//! - `route` / `route_any` — native `Serve` handlers
//! - `route_writer` / `route_any_writer` — `ServeWriter` handlers
//! - `route_cf` / `route_any_cf` — Cloudflare Workers `CfServe` handlers
//! - `route_web` / `route_any_web` — browser `WebServe` handlers

use std::sync::Arc;

use foundation_netio::shared::http::SimpleMethod;

use crate::shared::context::ContextBag;
use crate::shared::middleware::RequestMiddleware;
use crate::shared::router::Router;
#[cfg(not(target_family = "wasm"))]
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

#[cfg(not(target_family = "wasm"))]
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
// Native H2Serve handlers (cfg-gated, non-wasm only)

#[cfg(not(target_family = "wasm"))]
impl HttpApp<Arc<dyn crate::shared::serve::h2::H2Serve>> {
    /// Create a new empty `HttpApp` for HTTP/2 handlers.
    #[must_use]
    pub fn new_h2_serve() -> Self {
        Self {
            ctx: Arc::new(ContextBag::new()),
            router: Router::new(),
            middleware: Vec::new(),
        }
    }

    /// Register an `H2Serve` handler for a specific method and path.
    ///
    /// Unlike the `Serve` registrations, this takes the handler itself rather
    /// than a factory: an h2 handler is shared across every stream on every
    /// connection, so there is nothing to construct per request.
    pub fn route_h2(
        &mut self,
        method: SimpleMethod,
        path: &str,
        handler: Arc<dyn crate::shared::serve::h2::H2Serve>,
    ) -> &mut Self {
        self.router.add_route(method, path, handler);
        self
    }

    /// Register an `H2Serve` handler for all methods on a path.
    pub fn route_any_h2(
        &mut self,
        path: &str,
        handler: Arc<dyn crate::shared::serve::h2::H2Serve>,
    ) -> &mut Self {
        self.router.add_route_any(path, &handler);
        self
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

// ── ServerApp: dual-protocol server ─────────────────────────────────────────

/// Which protocol(s) a server can speak, and the app that answers each.
///
/// `Serve` takes a whole `SimpleIncomingRequest`; `H2Serve` takes a header plus
/// a body pipe. The two handler types — and their middleware — cannot be
/// unified, so a dual-protocol server carries one app per protocol.
///
/// There is deliberately no `Any(Option, Option)` variant: it would make a
/// server that speaks nothing representable, and `All(a, b)` would duplicate
/// `Both`. Adding HTTP/3 later means adding a variant, not loosening this one.
#[cfg(not(target_family = "wasm"))]
pub type H1App = Arc<HttpApp<Arc<dyn crate::shared::serve::Serve>>>;

#[cfg(not(target_family = "wasm"))]
pub type H2App = Arc<HttpApp<Arc<dyn crate::shared::serve::h2::H2Serve>>>;

#[cfg(not(target_family = "wasm"))]
#[derive(Clone)]
pub enum ServerApp {
    Http1(H1App),
    Http2(H2App),
    Both { http1: H1App, http2: H2App },
}

#[cfg(not(target_family = "wasm"))]
impl ServerApp {
    #[must_use]
    pub fn http1(app: HttpApp<Arc<dyn crate::shared::serve::Serve>>) -> Self {
        ServerApp::Http1(Arc::new(app))
    }

    #[must_use]
    pub fn http2(app: HttpApp<Arc<dyn crate::shared::serve::h2::H2Serve>>) -> Self {
        ServerApp::Http2(Arc::new(app))
    }

    #[must_use]
    pub fn both(
        http1: HttpApp<Arc<dyn crate::shared::serve::Serve>>,
        http2: HttpApp<Arc<dyn crate::shared::serve::h2::H2Serve>>,
    ) -> Self {
        ServerApp::Both {
            http1: Arc::new(http1),
            http2: Arc::new(http2),
        }
    }

    /// The HTTP/1.1 app, or `None` if this server does not speak HTTP/1.1.
    ///
    /// A `None` here means the connection must be refused with `505`, never
    /// served by an empty router — "no app" is not "route not found".
    #[must_use]
    pub fn get_h1(&self) -> Option<&H1App> {
        match self {
            ServerApp::Http1(a) | ServerApp::Both { http1: a, .. } => Some(a),
            ServerApp::Http2(_) => None,
        }
    }

    /// The HTTP/2 app, or `None` if this server does not speak HTTP/2.
    ///
    /// A `None` here means the h2 connection must be refused, never downgraded.
    #[must_use]
    pub fn get_h2(&self) -> Option<&H2App> {
        match self {
            ServerApp::Http2(a) | ServerApp::Both { http2: a, .. } => Some(a),
            ServerApp::Http1(_) => None,
        }
    }
}
