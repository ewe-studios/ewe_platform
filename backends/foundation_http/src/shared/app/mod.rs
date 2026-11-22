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

use foundation_context::Ctx;
use foundation_netio::shared::http::SimpleMethod;

use crate::shared::middleware::RequestMiddleware;
use crate::shared::router::Router;
use crate::shared::serve::ServeWriterFactory;

/// Top-level HTTP application builder, generic over handler type `S`.
pub struct HttpApp<S> {
    pub ctx: Ctx,
    pub router: Router<S>,
    pub middleware: Vec<Box<dyn RequestMiddleware>>,
    /// The largest request body the wasm bridges read, in bytes.
    ///
    /// WHY: the wasm bridge streams a request body from a JS `ReadableStream`.
    /// The reader stops at this limit and fails the read, so a large or
    /// endless upload cannot exhaust memory.
    ///
    /// The default is 8 MiB. Cloudflare Workers caps a request body at 100 MB;
    /// nginx defaults `client_max_body_size` to 1 MiB. 8 MiB leaves room for an
    /// RPC frame batch and stays far below the platform cap.
    ///
    /// Used only by the wasm bridges; the native server reads through its own
    /// body limit.
    pub max_request_bytes: usize,
    /// The largest request body the in-memory `ServeWriter` path accepts, in
    /// bytes.
    ///
    /// WHY: the writer path dispatches a request that is already in memory. A
    /// handler never reads a body larger than this, so a case that is about a
    /// body limit can set it and get a 413.
    ///
    /// WHAT: `serve_writer::handle_request` answers 413 when a sized body is
    /// larger than this value. A streaming body keeps the wasm bridge's own
    /// bound ([`HttpApp::max_request_bytes`]) because its length is not known
    /// before it is read.
    ///
    /// The default is 1 MiB, the nginx `client_max_body_size` default.
    pub max_writer_body_bytes: usize,
    /// How many request body chunks a wasm bridge may hold ahead of the
    /// handler.
    ///
    /// WHY: the wasm bridge reads a JS `ReadableStream` in a spawned task. The
    /// bound stops a slow handler from growing the queue without limit.
    ///
    /// The default is the wasm client's `body_queue_chunks` default, so a
    /// Worker reads a request and a response with the same bound.
    ///
    /// Used only by the wasm bridges; the native server reads through its own
    /// socket buffer.
    pub body_queue_chunks: usize,
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
            max_request_bytes: 8 * 1024 * 1024,
            max_writer_body_bytes: 1024 * 1024,
            body_queue_chunks: 16,
        }
    }

    /// Replace the wasm request body limit (see [`HttpApp::max_request_bytes`]).
    #[must_use]
    pub fn with_max_request_bytes(mut self, bytes: usize) -> Self {
        self.max_request_bytes = bytes;
        self
    }

    /// Replace the in-memory writer-path body limit (see
    /// [`HttpApp::max_writer_body_bytes`]).
    #[must_use]
    pub fn with_max_writer_body_bytes(mut self, bytes: usize) -> Self {
        self.max_writer_body_bytes = bytes;
        self
    }

    /// Replace the wasm request read-ahead bound (see
    /// [`HttpApp::body_queue_chunks`]).
    #[must_use]
    pub fn with_body_queue_chunks(mut self, chunks: usize) -> Self {
        self.body_queue_chunks = chunks;
        self
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
