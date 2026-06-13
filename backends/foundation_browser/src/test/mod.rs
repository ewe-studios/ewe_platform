//! # Test harness runtime (spec-43 phase-1 §6)
//!
//! The runtime the `#[wasm_ui_server]` macro expands onto: a [`TestServer`]
//! (serves the page under test) + a [`Harness`] (deterministic setup/teardown +
//! panic trap) + [`TestConfig`].
//!
//! The server-driven UI machinery (broadcaster + App sink) lives in
//! `foundation_wasm_ui::server` — a real server capability, not a test detail —
//! and is re-exported here for convenience. Only the foundation_http/SSE glue
//! ([`stream`]) is local.

pub mod harness;
pub mod server;
pub mod stream;

pub use harness::{Harness, TestConfig};
pub use server::{Encoding, PageSource, StaticMount, TestServer, ARROW_LIB_PATH, RUNTIME_PATH};
pub use stream::STREAM_PATH;

// Server-driven UI fan-out + the App protocol sink now live in foundation_wasm_ui.
pub use foundation_wasm_ui::server::{BroadcastSink, BroadcastTx, Broadcaster};
