//! Connection-owned, worker-pooled HTTP serving framework built on `foundation_core`.
//!
//! # Architecture
//!
//! - **`Serve` trait**: Core handler abstraction — handlers implement `create()` and `serve()`
//! - **`ConnectionResult`**: Three outcomes — `Keep` (loop), `Take` (handler owns connection), `Close`
//! - **`ContextBag`**: Type-erased, thread-safe dependency store
//! - **`Router`**: Tree-based route matching (static/param/regex/wildcard) returning `ArcServe`
//! - **`HttpApp`**: Application builder with route registration and middleware chain
//! - **`HttpServer`**: TCP accept loop with `BackgroundJobRunner` thread pool
//!
//! # No async/await
//!
//! This crate uses synchronous blocking I/O on worker threads managed by
//! `BackgroundJobRunner`. No `tokio`, no `tower`, no `async-trait`.

// Core abstractions
pub mod serve;
pub mod context;
pub mod reader;

// Server
pub mod app;
pub mod server;

// Client IP marker
pub mod client_ip;

// Router (migrated from ewe_routing, stripped of async/tower/axum)
pub mod router;

// Protocol upgrades
pub mod upgrade;

// Middleware
pub mod middleware;

// Built-in handlers
pub mod handlers;

// Public re-exports
pub use serve::{ConnectionResult, Serve, ServeFactory, ServeError};
pub use serve::respond;
pub use context::ContextBag;
pub use app::HttpApp;
pub use server::{HttpServer, ServerConfig, KeepAliveConfig};
pub use client_ip::ClientIp;
pub use router::{ArcServe, Router};
pub use middleware::{MiddlewareResult, RequestMiddleware};
pub use middleware::{CorsConfig, CorsMiddleware};
pub use middleware::{LoggerConfig, LoggerMiddleware, LogLevel};
pub use middleware::{AuthConfig, AuthMiddleware, AuthResult};
pub use middleware::{CompressionConfig, CompressionMiddleware, CompressionAlgorithm};
pub use middleware::{BodyLimitMiddleware};
pub use upgrade::{accept_websocket, SseStream, UpgradeError};

// Re-export SSE types from foundation_core
pub use foundation_core::wire::event_source::{EventWriter, SseEvent};

// Re-exported from foundation_core for convenience
pub use foundation_core::wire::simple_http::{
    SimpleIncomingRequest, SimpleMethod, SimpleHeader, SimpleHeaders, SimpleUrl,
    SimpleOutgoingResponse, SendSafeBody, Proto, Status,
};
pub use foundation_core::io::ioutils::SharedByteBufferStream;
pub use foundation_core::netcap::RawStream;
pub use foundation_core::synca::OnSignal;
