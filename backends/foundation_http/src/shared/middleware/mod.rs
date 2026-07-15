//! `RequestMiddleware` — synchronous middleware trait and result type.

use std::sync::Arc;

use foundation_netio::shared::http::{SimpleIncomingRequest, SimpleOutgoingResponse};

use crate::shared::context::ContextBag;

/// Result of middleware execution.
pub enum MiddlewareResult {
    /// Continue to next middleware / handler.
    Continue,
    /// Short-circuit with a response — skip remaining middleware and the handler.
    Response(SimpleOutgoingResponse),
    /// Render immediately but DO NOT short-circuit — middleware chain and handler
    /// continue running after this interim response is written to the connection.
    /// Useful for HTTP/1.1 100-Continue, progress trailers, etc.
    InterimResponse(SimpleOutgoingResponse),
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

// Built-in middleware modules
mod cors;
mod logger;
mod auth;
mod compression;
mod body_limit;

// Re-export middleware types
pub use cors::{CorsConfig, CorsMiddleware};
pub use logger::{LoggerConfig, LoggerMiddleware, LogLevel};
pub use auth::{AuthConfig, AuthMiddleware, AuthResult};
pub use compression::{CompressionConfig, CompressionMiddleware, CompressionAlgorithm};
pub use body_limit::BodyLimitMiddleware;
