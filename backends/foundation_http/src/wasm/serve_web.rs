//! `WebServe` — web/browser handler trait.
//!
//! Implement this trait to handle requests with structured response fields
//! instead of writing HTTP wire bytes.

use std::sync::Arc;

use foundation_core::wire::simple_http::SimpleIncomingRequest;

use crate::shared::context::ContextBag;
use crate::wasm::web_conn::{WebConn, WebConnectionResult};

/// Web/browser handler — receives a typed `WebConn` for building
/// structured responses without HTTP wire format.
pub trait WebServe: Send + Sync + 'static {
    /// Handle an incoming request, writing the response to `conn`.
    fn serve_web(
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
