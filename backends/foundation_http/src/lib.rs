//! Connection-owned, worker-pooled HTTP serving framework built on `foundation_core`.
//!
//! # Architecture
//!
//! - **`shared/`** — wasm-compatible modules (traits, router, middleware, handlers, app builder)
//! - **`native/`** — TCP server, HTTP reader, protocol upgrades (non-wasm only)
//! - **`wasm/`** — request dispatch with memory-backed streams (wasm32 only)
//!
//! # No async/await
//!
//! This crate uses synchronous blocking I/O on worker threads managed by
//! `BackgroundJobRunner`. No `tokio`, no `tower`, no `async-trait`.

// Shared modules — always compiled
pub mod shared;

// Native modules — TCP server, reader, upgrades
#[cfg(not(target_arch = "wasm32"))]
pub mod native;

// Wasm modules — dispatch, WasmStream
#[cfg(target_arch = "wasm32")]
pub mod wasm;

// Public re-exports (shared — always available)
pub use shared::serve::{ConnectionResult, ServeError};
pub use shared::serve::respond;
pub use shared::context::ContextBag;
pub use shared::app::HttpApp;
pub use shared::router::{Router, Server};
pub use shared::middleware::{MiddlewareResult, RequestMiddleware};
pub use shared::middleware::{CorsConfig, CorsMiddleware};
pub use shared::middleware::{LoggerConfig, LoggerMiddleware, LogLevel};
pub use shared::middleware::{AuthConfig, AuthMiddleware, AuthResult};
pub use shared::middleware::{CompressionConfig, CompressionMiddleware, CompressionAlgorithm};
pub use shared::middleware::{BodyLimitMiddleware};
pub use shared::client_ip::ClientIp;

// Native-only re-exports
#[cfg(not(target_arch = "wasm32"))]
pub use shared::serve::{Serve, ServeFactory};
#[cfg(not(target_arch = "wasm32"))]
pub use shared::router::ArcServe;
#[cfg(not(target_arch = "wasm32"))]
pub use native::server::{HttpServer, ServerConfig, KeepAliveConfig};
#[cfg(not(target_arch = "wasm32"))]
pub use native::server::timeout::ExpectContinueConfig;
#[cfg(not(target_arch = "wasm32"))]
pub use native::upgrade::{accept_websocket, SseStream, UpgradeError};

// Wasm-only re-exports
#[cfg(target_arch = "wasm32")]
pub use shared::serve::{ServeWriter, ServeWriterFactory};
#[cfg(target_arch = "wasm32")]
pub use wasm::stream::WasmStream;
#[cfg(target_arch = "wasm32")]
pub use wasm::server::{handle_request, handle_request_with_bag};

// Re-export SSE types from foundation_core
pub use foundation_core::wire::event_source::{EventWriter, SseEvent};

// Re-exported from foundation_core for convenience
pub use foundation_core::wire::simple_http::{
    SimpleIncomingRequest, SimpleMethod, SimpleHeader, SimpleHeaders, SimpleUrl,
    SimpleOutgoingResponse, SendSafeBody, Proto, Status,
};
pub use foundation_core::io::ioutils::SharedByteBufferStream;

#[cfg(not(target_arch = "wasm32"))]
pub use foundation_core::netcap::RawStream;
#[cfg(not(target_arch = "wasm32"))]
pub use foundation_core::synca::OnSignal;
