//! WebSocket Transport — Connect envelopes over WS messages (F39).
//!
//! WHY: Bidi streaming over HTTP/1.1 requires a full-duplex carrier. WebSocket
//! upgrade provides exactly that — message-framed, bi-directional communication
//! over a single TCP connection. This is additive: nothing changes in the
//! existing HTTP/1.1 or HTTP/2 transports.
//!
//! WHAT: [`WsTransport`] implements [`Transport`] by spawning a
//! `WebSocketClient<SystemDnsResolver>` on the valtron pool, then bridging
//! its `MessageDelivery` (Pipe-backed) into the standard `TransportStream`
//! byte pipes.
//!
//! HOW: `open()` converts the HTTP URL to a WS URL, calls
//! `WebSocketClient::connect()`, spawns a collector task that bridges
//! inbound WS Binary messages → `body_tx`, and returns the standard
//! `TransportStream` with `delivery.into_pipe()` as `send_body`.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::url::Uri;
use foundation_core::valtron::{self, Pipe};
use foundation_netio::simple_http::client::shared::dns::SystemDnsResolver;
use foundation_netio::simple_http::shared::{
    Proto, RequestDescriptor, SimpleHeaders, Status,
};
use foundation_netio::websocket::native::connection::WebSocketClient;
use foundation_netio::websocket::shared::message::WebSocketMessage;

use super::{
    body_stream_from_pipe, head_stream_from_pipe, Transport, TransportCapabilities,
    TransportError, TransportStream,
};

/// Capabilities: WS gives full duplex on HTTP/1.1 but no h2 trailers.
const WS_CAPS: TransportCapabilities = TransportCapabilities {
    request_streaming: true,
    full_duplex: true,
    h2_trailers: false,
    http_versions: &[Proto::HTTP11],
    multiplexed: false,
};

/// Default read / sleep timeouts for WS connections.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
const SLEEP_TIMEOUT: Duration = Duration::from_millis(100);

/// A WebSocket-based client transport for bidi RPC over HTTP/1.1.
///
/// ```ignore
/// let transport: Arc<dyn Transport> = Arc::new(WsTransport);
/// let client = Client::new(transport, "http://host:8080/path", codecs, options)?;
/// ```
pub struct WsTransport;

impl WsTransport {
    /// Convert an HTTP URL to a WS URL by swapping the scheme.
    fn to_ws_url(http_url: &str) -> Result<String, TransportError> {
        let uri = Uri::parse(http_url)
            .map_err(|e| TransportError::Protocol(format!("invalid URL: {e}")))?;
        let scheme = uri.scheme();
        let ws_scheme = if scheme.is_https() || scheme.is_wss() { "wss" } else { "ws" };
        let host = uri.host_str().unwrap_or_else(|| "localhost".to_string());
        let port = uri.port();
        let path = uri.path_and_query().to_string();

        match port {
            Some(p) => Ok(format!("{ws_scheme}://{host}:{p}{path}")),
            None => Ok(format!("{ws_scheme}://{host}{path}")),
        }
    }
}

impl Transport for WsTransport {
    fn capabilities(&self) -> TransportCapabilities {
        WS_CAPS
    }

    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        let http_url = &request.request_url.url;
        let ws_url = Self::to_ws_url(http_url)?;

        // Connect via WebSocketClient — TCP, handshake, frame I/O.
        let (client, delivery) = WebSocketClient::connect(
            SystemDnsResolver::default(),
            ws_url,
            READ_TIMEOUT,
            SLEEP_TIMEOUT,
        )
        .map_err(|e| TransportError::Protocol(format!("ws connect: {e}")))?;

        // Split the client into its inner stream + delivery handle.
        let (_client_inner, delivery) = client.into_parts();

        // Pipes for caller-facing TransportStream.
        let (head_tx, head_rx) = Pipe::<(Status, SimpleHeaders)>::with_depth(1);
        let (body_tx, body_rx) = Pipe::<Bytes>::with_depth(64);
        let (trailer_tx, trailer_rx) = Pipe::<SimpleHeaders>::with_depth(1);
        let (send_tx, send_rx) = Pipe::<Bytes>::with_depth(64);

        // Bridge: send_rx bytes → WebSocketMessage::Binary → delivery.send()
        let delivery_clone = delivery.clone();
        valtron::send(foundation_core::valtron::from_future(async move {
            loop {
                match send_rx.receive().await {
                    Some(bytes) => {
                        if delivery_clone
                            .send(WebSocketMessage::Binary(bytes.to_vec()))
                            .is_err()
                        {
                            break;
                        }
                    }
                    None => break,
                }
            }
        }))
        .map_err(|e| TransportError::Connect(Arc::new(std::io::Error::new(
            std::io::ErrorKind::Other, e.to_string(),
        ))))?;

        // Collector: inbound WS messages → body_tx bytes.
        valtron::send(foundation_core::valtron::from_future(async move {
            let mut inner = _client_inner;
            loop {
                match inner.next() {
                    Some(foundation_core::valtron::Stream::Next(Ok(
                        WebSocketMessage::ConnectionEstablished,
                    ))) => {
                        let _ = head_tx.try_send((
                            Status::SwitchingProtocols,
                            SimpleHeaders::new(),
                        ));
                    }
                    Some(foundation_core::valtron::Stream::Next(Ok(
                        WebSocketMessage::Binary(data),
                    ))) => {
                        if body_tx.send(Bytes::from(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(foundation_core::valtron::Stream::Next(Ok(
                        WebSocketMessage::Close(..),
                    )))
                    | None => {
                        body_tx.close();
                        trailer_tx.close();
                        break;
                    }
                    _ => continue,
                }
            }
        }))
        .map_err(|e| TransportError::Connect(Arc::new(std::io::Error::new(
            std::io::ErrorKind::Other, e.to_string(),
        ))))?;

        Ok(TransportStream {
            send_body: send_tx,
            head: head_stream_from_pipe(head_rx),
            recv_body: body_stream_from_pipe(body_rx),
            trailers: trailer_rx,
        })
    }
}
