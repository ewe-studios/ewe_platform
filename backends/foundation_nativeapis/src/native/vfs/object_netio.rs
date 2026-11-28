//! The native transport for the S3/R2 HTTP object store.
//!
//! [`NetioObjectTransport`] sends one shaped [`ObjectRequest`] over the
//! `foundation_netio` client. The store in [`object_http`](super::object_http)
//! builds the method, the full Uniform Resource Locator (URL), the headers, the
//! body and the signature. The transport owns Transmission Control Protocol
//! (TCP) and Transport Layer Security (TLS): an `https` endpoint uses TLS, and a
//! local `http` endpoint uses plain Hypertext Transfer Protocol (HTTP).
//!
//! The transport reads the response body as a stream and stops at the bound.
//! It never holds more than `max_response_bytes`. A non-2xx status is not a
//! transport error: the transport returns it, and the store maps it.
//!
//! # Limits
//!
//! [`NetioObjectTransport::max_response_bytes`] is the transport bound. The
//! request also carries a bound, and the smaller one applies. The default is
//! 8 MiB, the same default as the store's response bound.

use super::object_http::{HttpMethod, ObjectRequest, ObjectResponse, ObjectTransport};
use crate::shared::vfs::durable::{ObjectError, ObjectFuture};
use core::fmt;
use foundation_netio::shared::client::body_reader::collect_bytes_bounded;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleMethod};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};

/// The default transport response bound, in bytes.
///
/// The value is 8 MiB, the same default as the store's `max_response_bytes`.
/// A caller that reads a larger object reads it in pages.
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// A transport failure, wrapped as a typed error.
///
/// The message names the client error. The transport returns this only when the
/// request cannot be built or the client cannot send it. A non-2xx response is
/// not an error here.
#[derive(Debug)]
pub struct NetioTransportError(String);

impl fmt::Display for NetioTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for NetioTransportError {}

/// The object transport over the `foundation_netio` client.
///
/// The client is a shared handle, so a clone shares the connection pool. A
/// host that opens several stores clones one transport to reuse connections.
#[derive(Clone)]
pub struct NetioObjectTransport {
    client: DynNetClient,
    max_response_bytes: usize,
}

impl NetioObjectTransport {
    /// Build the transport with the default native client and the default
    /// response bound.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: HttpClientBuilder::new().build(),
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        }
    }

    /// Build the transport over a client that the caller built.
    ///
    /// Use this to share one client with other code, or to point the client at
    /// a test server.
    #[must_use]
    pub fn with_client(client: DynNetClient) -> Self {
        Self {
            client,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        }
    }

    /// Set the largest response body this transport returns.
    #[must_use]
    pub fn with_max_response_bytes(mut self, bytes: usize) -> Self {
        self.max_response_bytes = bytes.max(1);
        self
    }

    /// Return the transport response bound.
    #[must_use]
    pub fn max_response_bytes(&self) -> usize {
        self.max_response_bytes
    }
}

impl Default for NetioObjectTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for NetioObjectTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetioObjectTransport")
            .field("max_response_bytes", &self.max_response_bytes)
            .finish_non_exhaustive()
    }
}

fn method_of(method: HttpMethod) -> SimpleMethod {
    match method {
        HttpMethod::Get => SimpleMethod::GET,
        HttpMethod::Put => SimpleMethod::PUT,
        HttpMethod::Post => SimpleMethod::POST,
        HttpMethod::Head => SimpleMethod::HEAD,
        HttpMethod::Delete => SimpleMethod::DELETE,
    }
}

impl ObjectTransport for NetioObjectTransport {
    fn send(&self, request: ObjectRequest) -> ObjectFuture<'_, ObjectResponse> {
        Box::pin(async move {
            let limit = request
                .max_response_bytes
                .min(self.max_response_bytes.max(1));
            let url = request.full_url();
            let mut builder = PreparedRequestBuilder::new(method_of(request.method), &url)
                .map_err(|error| {
                    ObjectError::Backend(Box::new(NetioTransportError(error.to_string())))
                })?;
            for (name, value) in &request.headers {
                // The request builder sets `Host` from the URL. A second value
                // would make the signed request invalid.
                if name.eq_ignore_ascii_case("host") {
                    continue;
                }
                builder = builder.header(SimpleHeader::custom(name.clone()), value.clone());
            }
            if !request.body.is_empty() {
                builder = builder.body(SendSafeBody::Bytes(request.body));
            }
            let prepared = builder.build();
            let response = self.client.send_async(prepared).await.map_err(|error| {
                ObjectError::Backend(Box::new(NetioTransportError(error.to_string())))
            })?;
            let (status, headers, body) = response.into_async_parts();
            let status = u16::try_from(status.into_usize()).unwrap_or(0);
            // A HEAD response and a 204 or 304 response carry no body, even when
            // the headers name a length. A read would wait for bytes that never
            // arrive and would hold the connection.
            let bodyless = request.method == HttpMethod::Head
                || status == 204
                || status == 304
                || (100..200).contains(&status);
            let body = if bodyless {
                Vec::new()
            } else {
                collect_bytes_bounded(body, limit).await
            };
            let headers = headers
                .into_iter()
                .flat_map(|(key, values)| {
                    values
                        .into_iter()
                        .map(move |value| (key.to_string(), value))
                })
                .collect();
            Ok(ObjectResponse {
                status,
                headers,
                body,
            })
        })
    }
}
