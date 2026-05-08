//! Batteries-included handlers and middleware.

mod static_file;
mod health;
mod panic_recovery;
mod rate_limit;
mod error_middleware;
mod request_id;

pub use static_file::StaticFileHandler;
pub use health::HealthHandler;
pub use panic_recovery::with_panic_recovery;
pub use rate_limit::RateLimiter;
pub use error_middleware::ErrorMiddleware;
pub use request_id::RequestIdMiddleware;
