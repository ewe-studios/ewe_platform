//! `RequestMiddleware` — synchronous middleware trait and result type.

use foundation_netio::shared::http::{SimpleIncomingRequest, SimpleOutgoingResponse};

use crate::shared::context::Ctx;

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
    fn handle(&self, ctx: &Ctx, req: &mut SimpleIncomingRequest) -> MiddlewareResult;
}

// Built-in middleware modules
mod auth;
mod body_limit;
mod compression;
mod cors;
mod logger;

// Re-export middleware types
pub use auth::{AuthConfig, AuthMiddleware, AuthResult};
pub use body_limit::BodyLimitMiddleware;
pub use compression::{CompressionAlgorithm, CompressionConfig, CompressionMiddleware};
pub use cors::{CorsConfig, CorsMiddleware};
pub use logger::{LogLevel, LoggerConfig, LoggerMiddleware};
