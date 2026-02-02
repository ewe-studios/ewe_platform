// HTTP 1.1 Client Module
//
// This module provides an HTTP 1.1 client implementation using iterator-based
// patterns and pluggable DNS resolution.

mod actions;
mod client;
mod connection;
mod dns;
mod errors;
mod executor;
mod intro;
mod request;
mod task;

pub use client::{ClientConfig, SimpleHttpClient};
pub use connection::{HttpClientConnection, ParsedUrl, Scheme};
pub use dns::{CachingDnsResolver, DnsResolver, MockDnsResolver, SystemDnsResolver};
pub use errors::{DnsError, HttpClientError};
pub use intro::ResponseIntro;
pub use request::{ClientRequestBuilder, PreparedRequest};

// Internal re-exports for use within the client module
pub(crate) use actions::HttpClientAction;
pub(crate) use task::HttpRequestTask;

#[cfg(test)]
mod tests;
