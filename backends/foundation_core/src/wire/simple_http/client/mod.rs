// HTTP 1.1 Client Module
//
// This module provides an HTTP 1.1 client implementation using iterator-based
// patterns and pluggable DNS resolution.

mod actions;
mod api;
mod client;
mod connection;
mod dns;
mod errors;
mod executor;
mod intro;
mod pool;
mod request;
mod task;
mod tls_task;

pub use api::ClientRequest;
pub use client::{ClientConfig, SimpleHttpClient};
pub use connection::{HttpClientConnection, ParsedUrl, Scheme};
pub use dns::{CachingDnsResolver, DnsResolver, MockDnsResolver, SystemDnsResolver};
pub use errors::{DnsError, HttpClientError};
pub use intro::ResponseIntro;
pub use request::{ClientRequestBuilder, PreparedRequest};

// Internal re-exports for use within the client module
pub(crate) use actions::HttpClientAction;
pub(crate) use pool::ConnectionPool;
pub(crate) use task::{HttpRequestState, HttpRequestTask, HttpTaskReady, RequestControl};

#[cfg(not(target_arch = "wasm32"))]
pub use tls_task::{TlsHandshakeState, TlsHandshakeTask};

#[cfg(test)]
mod tests;
