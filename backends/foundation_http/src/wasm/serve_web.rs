//! `WebServe` — web/browser handler trait.
//!
//! Implement this trait to handle requests with structured response fields
//! instead of writing HTTP wire bytes.

use std::sync::Arc;

use foundation_netio::simple_http::shared::{SimpleIncomingRequest, SimpleMethod};

use crate::shared::app::HttpApp;
use crate::shared::context::ContextBag;
use crate::wasm::web_conn::{WebConn, WebConnectionResult};

/// Web/browser handler — receives a typed `WebConn` for building
/// structured responses without HTTP wire format.
#[async_trait::async_trait(?Send)]
pub trait WebServe: Sync + 'static {
    /// Handle an incoming request, writing the response to `conn`.
    async fn serve_web(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut WebConn,
    ) -> WebConnectionResult;
}

/// Factory trait for creating `WebServe` handler instances.
pub trait WebServeFactory: WebServe + Sized {
    /// Create a new instance of this handler.
    fn create(bag: &ContextBag) -> Self;
}

// ---------------------------------------------------------------------------
// HttpApp<Arc<dyn WebServe>> convenience methods

impl HttpApp<Arc<dyn WebServe>> {
    /// Create a new empty `HttpApp` for WebServe handlers.
    #[must_use]
    pub fn new_web() -> Self {
        use crate::shared::router::Router;
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
}
