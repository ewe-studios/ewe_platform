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
#[cfg(any(target_arch = "wasm32", feature = "wasm-test"))]
pub mod wasm;

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
