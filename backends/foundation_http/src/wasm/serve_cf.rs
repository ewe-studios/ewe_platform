//! `CfServe` — Cloudflare Workers handler trait.
//!
//! Implement this trait to handle requests with structured response fields
//! instead of writing HTTP wire bytes.

use std::sync::Arc;

use foundation_netio::simple_http::{SimpleIncomingRequest, SimpleMethod};

use crate::shared::app::HttpApp;
use crate::shared::context::ContextBag;
use crate::wasm::cf_conn::{CfConn, CfConnectionResult};

/// Cloudflare Workers handler — receives a typed `CfConn` for building
/// structured responses without HTTP wire format.
pub trait CfServe: Send + Sync + 'static {
    /// Handle an incoming request, writing the response to `conn`.
    fn serve_cf(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult;
}

/// Factory trait for creating `CfServe` handler instances.
pub trait CfServeFactory: CfServe + Sized {
    /// Create a new instance of this handler.
    fn create(bag: &ContextBag) -> Self;
}

// ---------------------------------------------------------------------------
// HttpApp<Arc<dyn CfServe>> convenience methods

impl HttpApp<Arc<dyn CfServe>> {
    /// Create a new empty `HttpApp` for CfServe handlers.
    #[must_use]
    pub fn new_cf() -> Self {
        use crate::shared::router::Router;
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
}
