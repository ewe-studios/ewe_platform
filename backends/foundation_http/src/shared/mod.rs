//! Shared (wasm-compatible) modules.

pub mod app;
pub mod client_ip;
/// The shared context. Handlers read their resources from a [`Ctx`](foundation_context::Ctx).
pub mod context {
    pub use foundation_context::Ctx;
}
pub mod handlers;
pub mod middleware;
pub mod router;
pub mod serve;
pub mod serve_web;
pub mod web_conn;
