//! `HttpApp<S>` — application builder with router, context, and middleware.
//!
//! The handler type `S` flows through `Router<S>` down to `RouteSegment<S>`
//! and `RouteMethod<S>`. Register handlers using the appropriate method:
//! - `route_writer` / `route_any_writer` — `ServeWriter` handlers
//! - `route_cf` / `route_any_cf` — Cloudflare Workers `CfServe` handlers
//! - `route_web` / `route_any_web` — browser `WebServe` handlers
//!
//! The native handler builders (`route`, `route_h2`, `route_h3`), the
//! `ServerApp` protocol selector and the `H1App`/`H2App`/`H3App` aliases live
//! in [`crate::native::app`], because their handler traits use native-only I/O.

use std::sync::Arc;

use foundation_core::valtron::Ctx;
use foundation_netio::shared::http::SimpleMethod;

use crate::shared::middleware::RequestMiddleware;
use crate::shared::router::Router;
use crate::shared::serve::ServeWriterFactory;

/// Top-level HTTP application builder, generic over handler type `S`.
pub struct HttpApp<S> {
    pub ctx: Ctx,
    pub router: Router<S>,
    pub middleware: Vec<Box<dyn RequestMiddleware>>,
}

impl<S> HttpApp<S> {
    /// Build an empty `HttpApp` from an explicit valtron [`Ctx`].
    ///
    /// This is the deterministic-test entry point. The app's clock and RNG
    /// stream come from `ctx`, so a simulated clock and a seeded RNG flow into
    /// every handler that reads `app.context()`. A deterministic host passes
    /// its own bundle here instead of replacing `app.ctx` after construction.
    ///
    /// The plain constructors ([`HttpApp::new`], `new_serve`, `new_h2_serve`,
    /// `new_h3_serve`, `new_writer`, `new_web`, `new_cf`) keep their
    /// signature. They call this method with [`Ctx::host`] and stay the host
    /// boundary.
    #[must_use]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the constructor consumes the caller's bundle by design; a reference \
                  would leak the temporary `Ctx` lifetime into the public signature"
    )]
    pub fn with_ctx(ctx: Ctx) -> Self {
        Self::with_context(ctx.with_empty_map())
    }

    /// Build an empty `HttpApp` from an already-built [`Ctx`].
    ///
    /// Use this when the caller already holds the context, for example a wasm
    /// singleton whose builder receives the context. The app shares the `Arc`, so
    /// its clock and RNG are the ones the caller injected.
    #[must_use]
    pub fn with_context(ctx: Ctx) -> Self {
        Self {
            ctx,
            router: Router::new(),
            middleware: Vec::new(),
        }
    }

    /// Create a new empty `HttpApp`.
    ///
    /// This is a host entry point: it uses the real host clock. Use
    /// [`HttpApp::with_ctx`] in a deterministic test.
    #[must_use]
    pub fn new() -> Self
    where
        S: Default,
    {
        Self::with_ctx(Ctx::host())
    }

    /// Get a reference to the shared context.
    #[must_use]
    pub fn context(&self) -> &Ctx {
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
// ServeWriter handlers (available on all targets)

impl HttpApp<Arc<dyn crate::shared::serve::ServeWriter>> {
    /// Create a new empty `HttpApp` for `ServeWriter` handlers.
    ///
    /// This is a host entry point: it uses the real host clock. Use
    /// [`HttpApp::with_ctx`] in a deterministic test.
    #[must_use]
    pub fn new_writer() -> Self {
        Self::with_ctx(Ctx::host())
    }

    /// Register a writer handler for a specific HTTP method and path.
    pub fn route_writer<H: ServeWriterFactory>(
        &mut self,
        method: SimpleMethod,
        path: &str,
    ) -> &mut Self {
        let handler = H::create(&self.ctx);
        self.router
            .add_route_writer(method, path, Arc::new(handler));
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
