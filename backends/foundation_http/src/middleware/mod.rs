//! `RequestMiddleware` — synchronous middleware trait and result type.

use std::sync::Arc;

use foundation_core::wire::simple_http::{SimpleIncomingRequest, SimpleOutgoingResponse};

use crate::context::ContextBag;

/// Result of middleware execution.
pub enum MiddlewareResult {
    /// Continue to next middleware / handler.
    Continue,
    /// Short-circuit with a response.
    Response(SimpleOutgoingResponse),
}

/// Synchronous request middleware.
///
/// Middleware can inspect and modify requests, or short-circuit with a response.
pub trait RequestMiddleware: Send + Sync + 'static {
    /// Handle an incoming request.
    ///
    /// Called in registration order. Returning `MiddlewareResult::Response`
    /// short-circuits the chain and renders the response immediately.
    fn handle(
        &self,
        ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult;
}
