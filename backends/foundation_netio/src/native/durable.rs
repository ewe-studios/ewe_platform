//! The native client transport for Durable Streams.
//!
//! It wraps the `foundation_netio` Hypertext Transfer Protocol (HTTP) client.
//! The protocol core stays transport-neutral.

use crate::shared::client::body_reader::try_collect_bytes;
use crate::shared::client::request_builder::PreparedRequestBuilder;
use crate::shared::durable::{
    ClientFuture, ClientRequest, ClientResponse, ClientSseStream, ClientTransport, HttpMethod,
    SseEvent, TransportError,
};
use crate::shared::http::SimpleHeader;
use crate::{DynNetClient, HttpClientBuilder};
use core::future::Future;
use std::collections::BTreeMap;
use std::time::Duration;

/// A transport over the native HTTP client.
pub struct NetioTransport {
    client: DynNetClient,
    sse_open_timeout: Duration,
}

impl NetioTransport {
    /// Build a transport with default timeouts.
    #[must_use]
    pub fn new() -> Self {
        Self::with_timeouts(Duration::from_secs(10), Duration::from_secs(60))
    }

    /// Build a transport with explicit connect and read timeouts.
    #[must_use]
    pub fn with_timeouts(connect: Duration, read: Duration) -> Self {
        let client = HttpClientBuilder::new()
            .connect_timeout(connect)
            .read_timeout(read)
            .build();
        Self {
            client,
            sse_open_timeout: Duration::from_secs(30),
        }
    }

    /// Build a transport over an existing client.
    #[must_use]
    pub fn from_client(client: DynNetClient) -> Self {
        Self {
            client,
            sse_open_timeout: Duration::from_secs(30),
        }
    }

    /// Set the longest wait for a Server-Sent Events (SSE) response head.
    #[must_use]
    pub fn with_sse_open_timeout(mut self, timeout: Duration) -> Self {
        self.sse_open_timeout = timeout;
        self
    }
}

/// Map a standard header name to its enum variant.
///
/// The request builder adds `Content-Type` again for a byte body. A custom
/// variant would hold a second value under a different map key.
fn known_header(name: &str) -> SimpleHeader {
    if name.eq_ignore_ascii_case("content-type") {
        SimpleHeader::CONTENT_TYPE
    } else if name.eq_ignore_ascii_case("content-length") {
        SimpleHeader::CONTENT_LENGTH
    } else if name.eq_ignore_ascii_case("accept") {
        SimpleHeader::ACCEPT
    } else if name.eq_ignore_ascii_case("authorization") {
        SimpleHeader::AUTHORIZATION
    } else if name.eq_ignore_ascii_case("host") {
        SimpleHeader::HOST
    } else {
        SimpleHeader::custom(name)
    }
}

impl Default for NetioTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientTransport for NetioTransport {
    fn open_sse(&self, request: ClientRequest) -> ClientFuture<'_, ClientSseStream> {
        let client = self.client.clone();
        let sse_open_timeout = self.sse_open_timeout;
        Box::pin(async move {
            let mut builder = PreparedRequestBuilder::get(&request.url)
                .map_err(|error| TransportError::new(error.to_string()))?;
            builder = builder.header(SimpleHeader::ACCEPT, "text/event-stream");
            for (name, value) in request.headers {
                builder = builder.header(known_header(&name), value);
            }
            // The valtron timer does not always progress under a blocking
            // executor, so the open future wakes itself and checks the clock.
            let deadline = std::time::Instant::now() + sse_open_timeout;
            let mut open = client.send_sse_async(builder.build());
            let outcome = futures_lite::future::poll_fn(|cx| {
                match open.as_mut().poll(cx) {
                    std::task::Poll::Pending => {
                        if std::time::Instant::now() >= deadline {
                            std::task::Poll::Ready(None)
                        } else {
                            cx.waker().wake_by_ref();
                            std::task::Poll::Pending
                        }
                    }
                    std::task::Poll::Ready(value) => std::task::Poll::Ready(Some(value)),
                }
            })
            .await;
            let inner = match outcome {
                Some(Ok(inner)) => inner,
                Some(Err(error)) => return Err(TransportError::new(error.to_string())),
                None => {
                    return Err(TransportError::new(
                        "Server-Sent Events head timed out",
                    ))
                }
            };
            let events = futures_lite::stream::unfold(inner, |mut inner| async move {
                use foundation_core::valtron::Stream;
                loop {
                    match futures_lite::StreamExt::next(&mut inner).await {
                        None => return None,
                        Some(Stream::Next(parsed)) => {
                            let event = match parsed.event {
                                crate::event_source::Event::Message {
                                    event_type, data, ..
                                } => SseEvent {
                                    event_type,
                                    data,
                                },
                                crate::event_source::Event::Comment(_)
                                | crate::event_source::Event::Reconnect => continue,
                            };
                            return Some((Ok(event), inner));
                        }
                        Some(Stream::Pending(
                            crate::shared::client::SseProgress::Failed(message),
                        )) => {
                            return Some((Err(TransportError::new(message)), inner));
                        }
                        _ => continue,
                    }
                }
            });
            Ok(Box::pin(events) as ClientSseStream)
        })
    }

    fn send(&self, request: ClientRequest) -> ClientFuture<'_, ClientResponse> {
        let client = self.client.clone();
        Box::pin(async move {
            let builder = match request.method {
                HttpMethod::Get => PreparedRequestBuilder::get(&request.url),
                HttpMethod::Head => PreparedRequestBuilder::head(&request.url),
                HttpMethod::Put => PreparedRequestBuilder::put(&request.url),
                HttpMethod::Post => PreparedRequestBuilder::post(&request.url),
                HttpMethod::Delete => PreparedRequestBuilder::delete(&request.url),
            }
            .map_err(|error| TransportError::new(error.to_string()))?;

            let mut builder = builder;
            if request.method == HttpMethod::Head {
                // The client exchange cannot always delimit a HEAD body on a
                // reused connection. Close the connection after the answer.
                builder = builder.header(SimpleHeader::CONNECTION, "close");
            }
            for (name, value) in request.headers {
                builder = builder.header(known_header(&name), value);
            }
            if !request.body.is_empty() {
                builder = builder.body_bytes(request.body);
            }

            let response = client
                .send_async(builder.build())
                .await
                .map_err(|error| TransportError::new(error.to_string()))?;
            let (status, headers, body) = response.into_parts();
            let status = u16::try_from(status.into_usize())
                .map_err(|_| TransportError::new("status does not fit u16"))?;
            let bytes = try_collect_bytes(body)
                .map_err(|error| TransportError::new(error.to_string()))?;

            let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for (name, values) in headers {
                map.entry(name.to_string()).or_default().extend(values);
            }
            Ok(ClientResponse {
                status,
                headers: map,
                body: bytes,
            })
        })
    }
}
