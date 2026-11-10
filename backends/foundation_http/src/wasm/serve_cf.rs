//! `CfServe` — Cloudflare Workers handler trait.
//!
//! Implement this trait to handle requests with structured response fields
//! instead of writing HTTP wire bytes.

use std::sync::Arc;

use foundation_netio::shared::http::{SimpleIncomingRequest, SimpleMethod};

use crate::shared::app::HttpApp;
use crate::shared::context::Ctx;
use crate::wasm::cf_conn::{CfConn, CfConnectionResult};

/// Cloudflare Workers handler — receives a typed `CfConn` for building
/// structured responses without HTTP wire format.
#[async_trait::async_trait(?Send)]
pub trait CfServe: Sync + 'static {
    /// Handle an incoming request, writing the response to `conn`.
    async fn serve_cf(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult;
}

/// Factory trait for creating `CfServe` handler instances.
pub trait CfServeFactory: CfServe + Sized {
    /// Create a new instance of this handler.
    fn create(ctx: &Ctx) -> Self;
}

// ---------------------------------------------------------------------------
// HttpApp<Arc<dyn CfServe>> convenience methods

impl HttpApp<Arc<dyn CfServe>> {
    /// Create a new empty `HttpApp` for CfServe handlers.
    ///
    /// This is a host entry point: it uses the real host clock. Use
    /// [`HttpApp::with_ctx`] or [`HttpApp::with_clock`] in a deterministic
    /// test.
    #[must_use]
    pub fn new_cf() -> Self {
        Self::with_ctx(foundation_core::valtron::Ctx::host())
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
