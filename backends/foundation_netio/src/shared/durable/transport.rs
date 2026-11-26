//! The injected client transport.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

/// A boxed transport future.
pub type ClientFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, TransportError>> + Send + 'a>>;

/// The Hypertext Transfer Protocol (HTTP) method of one client request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpMethod {
    /// Read a resource.
    Get,
    /// Read the resource headers only.
    Head,
    /// Create a resource.
    Put,
    /// Append to a resource.
    Post,
    /// Remove a resource.
    Delete,
}

impl HttpMethod {
    /// The method name on the wire.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Put => "PUT",
            Self::Post => "POST",
            Self::Delete => "DELETE",
        }
    }
}

/// One client request. The body is small and complete.
#[derive(Clone, Debug)]
pub struct ClientRequest {
    /// The method.
    pub method: HttpMethod,
    /// The absolute URL.
    pub url: String,
    /// The request headers.
    pub headers: Vec<(String, String)>,
    /// The request body.
    pub body: Vec<u8>,
}

/// One client response. The caller parses or bounds the body.
#[derive(Clone, Debug)]
pub struct ClientResponse {
    /// The status code.
    pub status: u16,
    /// The response headers, keyed without case.
    pub headers: BTreeMap<String, Vec<String>>,
    /// The response body.
    pub body: Vec<u8>,
}

impl ClientResponse {
    /// The first value of a header, matched without case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .and_then(|(_, values)| values.first())
            .map(String::as_str)
    }
}

/// A transport failure. The status stays in [`ClientResponse`].
#[derive(Debug)]
pub struct TransportError {
    message: String,
}

impl TransportError {
    /// Build a transport failure.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Return the message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl core::fmt::Display for TransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.message)
    }
}

impl core::error::Error for TransportError {}

/// The injected client transport.
pub trait ClientTransport: Send + Sync {
    /// Send one request and return the response.
    fn send(&self, request: ClientRequest) -> ClientFuture<'_, ClientResponse>;
}
