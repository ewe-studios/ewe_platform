//! `CfServe` — Cloudflare Workers handler trait.
//!
//! Implement this trait to handle requests with structured response fields
//! instead of writing HTTP wire bytes.

use std::sync::Arc;

use foundation_core::wire::simple_http::SimpleIncomingRequest;

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
