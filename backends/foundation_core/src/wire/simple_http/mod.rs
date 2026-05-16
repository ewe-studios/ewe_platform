mod errors;
mod impls;

pub mod client_classifier;
pub mod latency_tracker;
pub mod load_tracker;
pub mod sse;
pub mod shared;
pub mod timeout;

pub use crate::url;

pub use errors::*;
pub use impls::*;
pub use sse::SseParser;

// Client module — depends on netcap::RawStream, Connection, SSLConnector, HttpConnectionPool
#[cfg(not(target_arch = "wasm32"))]
pub mod client;
