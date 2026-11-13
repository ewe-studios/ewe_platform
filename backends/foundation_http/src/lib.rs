//! Connection-owned, worker-pooled HTTP serving framework built on `foundation_core`.
//!
//! # Architecture
//!
//! - **`shared/`** — wasm-compatible modules (traits, router, middleware, handlers, app builder)
//! - **`native/`** — TCP server, HTTP reader, protocol upgrades (non-wasm only)
//! - **`wasm/`** — request dispatch with memory-backed streams (wasm32 only)
//!
//! # Async responses
//!
//! Handlers that write to a connection return [`shared::serve::ConnectionResult::Async`]
//! and write through `native::writer::ResponseWriter`, which parks on socket
//! readiness instead of blocking a worker. The synchronous helpers in
//! [`shared::serve::respond_to_buffer`] are only for in-memory writers (tests,
//! wasm); they cannot truncate a `Vec<u8>`, but they can truncate a
//! non-blocking socket.

// Shared modules — always compiled
pub mod shared;

// Native modules — TCP server, reader, upgrades
#[cfg(not(target_family = "wasm"))]
pub mod native;

// Wasm modules — dispatch, WasmStream
#[cfg(any(target_family = "wasm", feature = "wasm-test"))]
pub mod wasm;

// Re-exported from foundation_core for convenience
pub use foundation_core::io::ioutils::SharedByteBufferStream;
pub use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, SimpleUrl, Status,
};

#[cfg(not(target_family = "wasm"))]
pub use foundation_iogate::ShutdownSignal;
#[cfg(not(target_family = "wasm"))]
pub use foundation_netio::netcap::RawStream;
