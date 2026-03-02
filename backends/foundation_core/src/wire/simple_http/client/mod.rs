// HTTP 1.1 Client Module
//
// This module provides an HTTP 1.1 client implementation using iterator-based
// patterns and pluggable DNS resolution.

mod api;
#[allow(clippy::module_inception)]
mod client;
mod compression;
mod connection;
mod control;
mod dns;
mod intro;
mod pool;
mod redirects;
mod request;
mod tasks;
mod tls_task;

pub use api::*;
pub use client::*;
pub use compression::*;
pub use connection::*;
pub use control::*;
pub use dns::*;
pub use intro::*;
pub use pool::*;
pub use redirects::*;
pub use request::*;
pub use tasks::*;

// Re-export ExecutionAction from valtron so tests and helpers can reference it
pub use crate::valtron::ExecutionAction;

pub use tls_task::*;
