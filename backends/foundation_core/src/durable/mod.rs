//! The Durable Streams protocol core.
//!
//! Shared code lives in [`shared`]. Platform-specific code lives in `wasm/` or `cf/` when it
//! lands. The core has no valtron, HTTP or socket dependency.

pub mod shared;

pub use shared::*;
