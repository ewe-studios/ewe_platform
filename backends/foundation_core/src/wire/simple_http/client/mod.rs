// HTTP 1.1 Client Module
//
// This module provides an HTTP 1.1 client implementation using iterator-based
// patterns and pluggable DNS resolution.

mod connection;
mod dns;
mod errors;
mod intro;
mod request;

pub use connection::{HttpClientConnection, ParsedUrl, Scheme};
pub use dns::{CachingDnsResolver, DnsResolver, MockDnsResolver, SystemDnsResolver};
pub use errors::{DnsError, HttpClientError};
pub use intro::ResponseIntro;
pub use request::{ClientRequestBuilder, PreparedRequest};

#[cfg(test)]
mod tests;
