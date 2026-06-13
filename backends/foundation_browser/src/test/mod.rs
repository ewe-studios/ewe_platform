//! # Test harness runtime (spec-43 phase-1 §6)
//!
//! The runtime the `#[wasm_ui_server]` macro expands onto: a [`TestServer`]
//! (serves the page under test) + a [`Harness`] (deterministic setup/teardown +
//! panic trap) + [`TestConfig`].

pub mod harness;
pub mod server;
pub mod stream;

pub use harness::{Harness, TestConfig};
pub use server::{Encoding, PageSource, StaticMount, TestServer};
pub use stream::{BroadcastTx, Broadcaster, STREAM_PATH};
