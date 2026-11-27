//! The native client transport for Durable Streams.
//!
//! It wraps the `foundation_netio` Hypertext Transfer Protocol (HTTP) client.
//! The protocol core stays transport-neutral.

use crate::shared::client::body_reader::{is_would_block, try_collect_bytes, ReadWakeSource};
use crate::shared::client::request_builder::PreparedRequestBuilder;
use foundation_core::durable::client::{
    ClientFuture, ClientRequest, ClientResponse, ClientSseStream, ClientTransport, HttpMethod,
    SseEvent, SseOpen, TransportError,
};
use crate::event_source::ParseResult;
use crate::shared::http::{SendSafeBody, SimpleHeader};
use crate::{DynNetClient, HttpClientBuilder};
use foundation_core::extensions::result_ext::SendableBoxedError;
use foundation_core::valtron::BoxedSendableIterator;
use std::collections::BTreeMap;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

/// A transport over the native HTTP client.
pub struct NetioTransport {
    client: DynNetClient,
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
        Self { client }
    }

    /// Build a transport over an existing client.
    #[must_use]
    pub fn from_client(client: DynNetClient) -> Self {
        Self { client }
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
    fn open_sse(&self, request: ClientRequest) -> ClientFuture<'_, SseOpen> {
        let client = self.client.clone();
        Box::pin(async move {
            let mut builder = PreparedRequestBuilder::get(&request.url)
                .map_err(|error| TransportError::new(error.to_string()))?;
            builder = builder.header(SimpleHeader::ACCEPT, "text/event-stream");
            for (name, value) in request.headers {
                builder = builder.header(known_header(&name), value);
            }
            // The async exchange opens the response and hands back its head
            // (status and headers) with the streamed body. The body is an
            // `SseStream` iterator because the response content type is
            // `text/event-stream`.
            let response = client
                .send_async(builder.build())
                .await
                .map_err(|error| TransportError::new(error.to_string()))?;
            let (status, headers, body, source) = response.into_parts_with_wake();
            let status = u16::try_from(status.into_usize())
                .map_err(|_| TransportError::new("status does not fit u16"))?;
            let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for (name, values) in headers {
                map.entry(name.to_string()).or_default().extend(values);
            }
            // A non-200 head (404, 410, ...) carries an error body, not an
            // event stream. The head status decides the outcome before the
            // stream is read, so an empty stream is enough for those.
            let events: ClientSseStream = match body {
                SendSafeBody::SseStream(Some(iterator)) => {
                    Box::pin(NativeSseStream::new(iterator, source))
                }
                _ => Box::pin(futures_lite::stream::empty::<
                    Result<SseEvent, TransportError>,
                >()),
            };
            Ok(SseOpen {
                status,
                headers: map,
                stream: events,
            })
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

/// An async Server-Sent Events (SSE) stream over a non-blocking socket.
///
/// The body iterator yields one parsed event, or a `WouldBlock` error when the
/// socket holds no complete event yet. The stream registers the connection's
/// readiness source and parks instead of spinning.
struct NativeSseStream {
    inner: BoxedSendableIterator<ParseResult, SendableBoxedError>,
    read_wake: Option<ReadWakeSource>,
    repolled: bool,
}

impl NativeSseStream {
    fn new(
        inner: BoxedSendableIterator<ParseResult, SendableBoxedError>,
        read_wake: Option<ReadWakeSource>,
    ) -> Self {
        Self {
            inner,
            read_wake,
            repolled: false,
        }
    }
}

impl futures_core::Stream for NativeSseStream {
    type Item = Result<SseEvent, TransportError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.inner.next() {
                Some(Ok(parsed)) => {
                    self.repolled = false;
                    let event = match parsed.event {
                        crate::event_source::Event::Message {
                            event_type, data, ..
                        } => SseEvent { event_type, data },
                        crate::event_source::Event::Comment(_)
                        | crate::event_source::Event::Reconnect => continue,
                    };
                    return Poll::Ready(Some(Ok(event)));
                }
                Some(Err(err)) if is_would_block(&*err) => {
                    if self.repolled {
                        self.repolled = false;
                        return Poll::Pending;
                    }
                    self.repolled = true;
                    match &self.read_wake {
                        Some(source) if source(cx.waker()) => {}
                        _ => {
                            self.repolled = false;
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }
                    }
                }
                Some(Err(err)) => {
                    self.repolled = false;
                    return Poll::Ready(Some(Err(TransportError::new(err.to_string()))));
                }
                None => return Poll::Ready(None),
            }
        }
    }
}
