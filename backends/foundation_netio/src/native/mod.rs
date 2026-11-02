//! Native-only modules (TCP, TLS, `io_uring` — not compiled on wasm).

pub mod connection;
pub mod raw_stream;
pub mod readiness;
pub mod ssl;
