//! WebSocket Transport — Connect envelopes over WS messages (F39).
//!
//! WHY: Bidi streaming over HTTP/1.1 requires a full-duplex carrier. WebSocket
//! upgrade provides exactly that — message-framed, bi-directional communication
//! over a single TCP connection. This is additive: nothing changes in the
//! existing HTTP/1.1 or HTTP/2 transports.
//!
//! WHAT: [`WsTransport`] implements [`Transport`] by spawning a
//! [`WsBytePump`] valtron task that handles TCP connect + WS handshake +
//! byte-pipe ↔ WS frame bridging. The caller gets the same
//! `TransportStream { send_body, head, recv_body, trailers }` as every other
//! transport.
//!
//! HOW: `open()` wires valtron pipes and spawns the pump on the pool. The pump
//! sends a 101 head once the upgrade completes. Protocol reader/writer tasks
//! (Connect framing) see the same `ByteSink`/`ByteSource` pipes — they don't
//! know or care that the bytes travel over WS frames underneath.

use std::sync::Arc;

use bytes::Bytes;
use foundation_core::url::Uri;
use foundation_core::valtron::{self, Pipe};
use foundation_netio::simple_http::shared::{
    Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, Status,
};
use foundation_netio::websocket::native::pump::WsBytePump;

use super::{
    body_stream_from_pipe, head_stream_from_pipe, Transport, TransportCapabilities,
    TransportError, TransportStream,
};

/// Capabilities: WS gives full duplex on HTTP/1.1.
const WS_CAPS: TransportCapabilities = TransportCapabilities {
    request_streaming: true,
    full_duplex: true,
    h2_trailers: false,
    http_versions: &[Proto::HTTP11],
    multiplexed: false,
};

/// A WebSocket-based client transport for bidi RPC over HTTP/1.1.
///
/// ```ignore
/// let transport: Arc<dyn Transport> = Arc::new(WsTransport);
/// let client = Client::new(transport, "ws://host:8080/path", codecs, options)?;
/// ```
pub struct WsTransport;

impl Transport for WsTransport {
    fn capabilities(&self) -> TransportCapabilities {
        WS_CAPS
    }

    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        let uri = request.request_uri;

        let host = uri.host_str().unwrap_or_else(|| "localhost".to_string());
        let port = uri.port().unwrap_or(80);
        let path = uri.path_and_query().to_string();
        let path = if path.is_empty() { "/".to_string() } else { path };

        // Pipes.
        let (send_tx, send_rx) = Pipe::<Bytes>::with_depth(64);
        let (head_tx, head_rx) = Pipe::<(Status, SimpleHeaders)>::with_depth(1);
        let (body_tx, body_rx) = Pipe::<Bytes>::with_depth(64);
        let (trailer_tx, trailer_rx) = Pipe::<SimpleHeaders>::with_depth(1);

        // Collect extra request headers to forward to the WS upgrade.
        let extra_headers = request.headers;

        let pump = WsBytePump::new(
            host, port, path, extra_headers,
            send_rx, head_tx, body_tx, trailer_tx,
        );

        valtron::send(pump).map_err(|e| {
            TransportError::Connect(Arc::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;

        Ok(TransportStream {
            send_body: send_tx,
            head: head_stream_from_pipe(head_rx),
            recv_body: body_stream_from_pipe(body_rx),
            trailers: trailer_rx,
        })
    }
}
