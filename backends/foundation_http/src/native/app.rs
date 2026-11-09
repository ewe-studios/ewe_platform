//! Native application types built on [`HttpApp`].
//!
//! `HttpApp<S>` itself is target-agnostic and lives in [`crate::shared::app`].
//! This module holds the handler types that only exist on native targets:
//! [`Serve`](crate::native::serve::Serve),
//! [`H2Serve`](crate::native::serve::H2Serve) and
//! [`H3Serve`](crate::native::serve::H3Serve) app builders, plus [`ServerApp`]
//! that selects the protocol an [`HttpServer`](crate::native::server::HttpServer)
//! serves.

use std::sync::Arc;

use foundation_core::valtron::Ctx;
use foundation_netio::shared::http::SimpleMethod;

use crate::native::serve::H2Serve;
#[cfg(feature = "quic")]
use crate::native::serve::H3Serve;
use crate::native::serve::Serve;
use crate::native::serve::ServeFactory;
use crate::native::server::HttpServer;
use crate::native::server::ServerConfig;
use crate::shared::app::HttpApp;

// ---------------------------------------------------------------------------
// Native Serve handlers

impl HttpApp<Arc<dyn Serve>> {
    /// Create a new empty `HttpApp` for native Serve handlers.
    ///
    /// This is a host entry point: it uses the real host clock. Use
    /// [`HttpApp::with_ctx`] or [`HttpApp::with_clock`] in a deterministic
    /// test.
    #[must_use]
    pub fn new_serve() -> Self {
        Self::with_ctx(Ctx::host())
    }

    /// Register a handler for a specific HTTP method and path.
    pub fn route<H: ServeFactory>(&mut self, method: SimpleMethod, path: &str) -> &mut Self {
        let handler = H::create(&self.ctx);
        self.router.add_route(method, path, Arc::new(handler));
        self
    }

    /// Register a handler for all HTTP methods on a path.
    pub fn route_any<H: ServeFactory>(&mut self, path: &str) -> &mut Self {
        let handler: Arc<dyn Serve> = Arc::new(H::create(&self.ctx));
        self.router.add_route_any(path, &handler);
        self
    }

    /// Create an `HttpServer` from this app with default config.
    #[must_use]
    pub fn server(self, addr: &str) -> HttpServer {
        HttpServer::new(self, addr)
    }

    /// Create an `HttpServer` from this app with custom config.
    #[must_use]
    pub fn server_with_config(self, addr: &str, config: ServerConfig) -> HttpServer {
        HttpServer::with_config(self, addr, config)
    }
}

// ---------------------------------------------------------------------------
// Native H2Serve handlers

impl HttpApp<Arc<dyn H2Serve>> {
    /// Create a new empty `HttpApp` for HTTP/2 handlers.
    ///
    /// This is a host entry point: it uses the real host clock. Use
    /// [`HttpApp::with_ctx`] or [`HttpApp::with_clock`] in a deterministic
    /// test.
    #[must_use]
    pub fn new_h2_serve() -> Self {
        Self::with_ctx(Ctx::host())
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
        handler: Arc<dyn H2Serve>,
    ) -> &mut Self {
        self.router.add_route(method, path, handler);
        self
    }

    /// Register an `H2Serve` handler for all methods on a path.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "callers pass a temporary Arc; the router clones what it stores"
    )]
    pub fn route_any_h2(&mut self, path: &str, handler: Arc<dyn H2Serve>) -> &mut Self {
        self.router.add_route_any(path, &handler);
        self
    }
}

// ---------------------------------------------------------------------------
// Native H3Serve handlers

#[cfg(feature = "quic")]
impl HttpApp<Arc<dyn H3Serve>> {
    /// Create a new empty `HttpApp` for HTTP/3 handlers.
    ///
    /// This is a host entry point: it uses the real host clock. Use
    /// [`HttpApp::with_ctx`] or [`HttpApp::with_clock`] in a deterministic
    /// test.
    #[must_use]
    pub fn new_h3_serve() -> Self {
        Self::with_ctx(Ctx::host())
    }

    /// Register an `H3Serve` handler for a specific method and path. Like
    /// `route_h2`, this takes the shared handler itself (an h3 handler serves
    /// every request stream on every connection).
    pub fn route_h3(
        &mut self,
        method: SimpleMethod,
        path: &str,
        handler: Arc<dyn H3Serve>,
    ) -> &mut Self {
        self.router.add_route(method, path, handler);
        self
    }

    /// Register an `H3Serve` handler for all methods on a path.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "callers pass a temporary Arc; the router clones what it stores"
    )]
    pub fn route_any_h3(&mut self, path: &str, handler: Arc<dyn H3Serve>) -> &mut Self {
        self.router.add_route_any(path, &handler);
        self
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
pub type H1App = Arc<HttpApp<Arc<dyn Serve>>>;

pub type H2App = Arc<HttpApp<Arc<dyn H2Serve>>>;

#[cfg(feature = "quic")]
pub type H3App = Arc<HttpApp<Arc<dyn H3Serve>>>;

#[derive(Clone)]
pub enum ServerApp {
    Http1(H1App),
    Http2(H2App),
    #[cfg(feature = "quic")]
    Http3(H3App),
    Both {
        http1: H1App,
        http2: H2App,
    },
    /// HTTP/1.1 + HTTP/2 + HTTP/3 multiplexed on one port (h1 + h2c-negotiated +
    /// Alt-Svc-advertised QUIC). Only available with the `quic` feature (F35).
    #[cfg(feature = "quic")]
    Any {
        http1: H1App,
        http2: H2App,
        http3: H3App,
    },
}

impl ServerApp {
    #[must_use]
    pub fn http1(app: HttpApp<Arc<dyn Serve>>) -> Self {
        ServerApp::Http1(Arc::new(app))
    }

    #[must_use]
    pub fn http2(app: HttpApp<Arc<dyn H2Serve>>) -> Self {
        ServerApp::Http2(Arc::new(app))
    }

    #[cfg(feature = "quic")]
    #[must_use]
    pub fn http3(app: HttpApp<Arc<dyn H3Serve>>) -> Self {
        ServerApp::Http3(Arc::new(app))
    }

    #[must_use]
    pub fn both(http1: HttpApp<Arc<dyn Serve>>, http2: HttpApp<Arc<dyn H2Serve>>) -> Self {
        ServerApp::Both {
            http1: Arc::new(http1),
            http2: Arc::new(http2),
        }
    }

    #[cfg(feature = "quic")]
    #[must_use]
    pub fn any(
        http1: HttpApp<Arc<dyn Serve>>,
        http2: HttpApp<Arc<dyn H2Serve>>,
        http3: HttpApp<Arc<dyn H3Serve>>,
    ) -> Self {
        ServerApp::Any {
            http1: Arc::new(http1),
            http2: Arc::new(http2),
            http3: Arc::new(http3),
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
            #[cfg(feature = "quic")]
            ServerApp::Any { http1: a, .. } => Some(a),
            ServerApp::Http2(_) => None,
            #[cfg(feature = "quic")]
            ServerApp::Http3(_) => None,
        }
    }

    /// The HTTP/2 app, or `None` if this server does not speak HTTP/2.
    ///
    /// A `None` here means the h2 connection must be refused, never downgraded.
    #[must_use]
    pub fn get_h2(&self) -> Option<&H2App> {
        match self {
            ServerApp::Http2(a) | ServerApp::Both { http2: a, .. } => Some(a),
            #[cfg(feature = "quic")]
            ServerApp::Any { http2: a, .. } => Some(a),
            ServerApp::Http1(_) => None,
            #[cfg(feature = "quic")]
            ServerApp::Http3(_) => None,
        }
    }

    /// The HTTP/3 app, or `None` if this server does not speak HTTP/3 (F35).
    #[cfg(feature = "quic")]
    #[must_use]
    pub fn get_h3(&self) -> Option<&H3App> {
        match self {
            ServerApp::Http3(a) | ServerApp::Any { http3: a, .. } => Some(a),
            ServerApp::Http1(_) | ServerApp::Http2(_) | ServerApp::Both { .. } => None,
        }
    }
}
