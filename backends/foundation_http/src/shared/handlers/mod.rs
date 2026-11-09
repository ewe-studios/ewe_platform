//! Batteries-included handlers and middleware.

mod error_middleware;
pub(crate) mod health;
mod panic_recovery;
mod rate_limit;
mod request_id;

pub use error_middleware::ErrorMiddleware;
pub use health::HealthHandler;
pub use panic_recovery::with_panic_recovery;
pub use rate_limit::RateLimiter;
pub use request_id::RequestIdMiddleware;
