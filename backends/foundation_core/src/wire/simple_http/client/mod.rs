// HTTP 1.1 Client Module
//
// This module provides an HTTP 1.1 client implementation using iterator-based
// patterns and pluggable DNS resolution.

mod actions;
mod api;
mod client;
mod connection;
mod control;
mod dns;
mod errors;
mod intro;
mod pool;
mod request;
mod task;
mod tls_task;

pub use actions::*;
pub use api::*;
pub use client::*;
pub use connection::*;
pub use control::*;
pub use dns::*;
pub use errors::*;
pub use intro::*;
pub use pool::*;
pub use request::*;
pub use task::*;

// Re-export ExecutionAction from valtron so tests and helpers can reference it
pub use crate::valtron::ExecutionAction;

#[cfg(not(target_arch = "wasm32"))]
pub use tls_task::*;
