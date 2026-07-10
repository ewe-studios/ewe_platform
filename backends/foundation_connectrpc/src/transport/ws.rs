//! WebSocket Transport — Connect envelopes over WS messages (F39).
//!
//! `WsTransport::open()` calls `WebSocketClient::connect_parts()`, spawns the
//! WS task + collector on the pool, and returns a `TransportStream` whose
//! `send_body` is a `MappedSender` — `Bytes` pushed by the caller are
//! converted to `WebSocketMessage::Binary` inline and pushed into the task's
//! outbound pipe. Zero outbound bridge tasks.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::url::Uri;
use foundation_core::valtron::{self, Pipe, Stream};
use foundation_core::valtron::execute;
use foundation_netio::simple_http::client::shared::dns::SystemDnsResolver;
use foundation_netio::simple_http::shared::{
    Proto, RequestDescriptor, SimpleHeaders, Status,
};
use foundation_netio::websocket::native::connection::{Reconnect, WebSocketClient};
use foundation_netio::websocket::shared::message::WebSocketMessage;

use super::{
    body_stream_from_pipe, head_stream_from_pipe, Transport, TransportCapabilities,
    TransportError, TransportStream,
};

const WS_CAPS: TransportCapabilities = TransportCapabilities {
    request_streaming: true,
    full_duplex: true,
    h2_trailers: false,
    http_versions: &[Proto::HTTP11],
    multiplexed: false,
};

const READ_TIMEOUT: Duration = Duration::from_secs(30);
const SLEEP_TIMEOUT: Duration = Duration::from_millis(100);

pub struct WsTransport;

impl WsTransport {
    fn to_ws_url(http_url: &str) -> Result<String, TransportError> {
        let uri = Uri::parse(http_url)
            .map_err(|e| TransportError::Protocol(format!("invalid URL: {e}")))?;
        let ws_scheme = if uri.scheme().is_https() || uri.scheme().is_wss() {
            "wss"
        } else {
            "ws"
        };
        let host = uri.host_str().unwrap_or_else(|| "localhost".to_string());
        let port = uri.port().map(|p| format!(":{p}")).unwrap_or_default();
        let path = uri.path_and_query().to_string();
        Ok(format!("{ws_scheme}://{host}{port}{path}"))
    }
}

impl Transport for WsTransport {
    fn capabilities(&self) -> TransportCapabilities {
        WS_CAPS
    }

    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        let ws_url = Self::to_ws_url(&request.request_url.url)?;

        // 1. Get spawned WS task + MessageDelivery from the client.
        let (task, delivery) = WebSocketClient::connect_parts(
            SystemDnsResolver::default(),
            ws_url,
            Reconnect::No,
            READ_TIMEOUT,
            SLEEP_TIMEOUT,
        )
        .map_err(|e| TransportError::Protocol(format!("ws connect: {e}")))?;

        // 2. Pipes.
        let (head_tx, head_rx) = Pipe::<(Status, SimpleHeaders)>::with_depth(1);
        let (trailer_tx, trailer_rx) = Pipe::<SimpleHeaders>::with_depth(1);
        let (body_tx, body_rx) = Pipe::<Bytes>::with_depth(64);

        // 3. Outbound: caller pushes Bytes → MappedSender converts to Binary
        //    → pushed into delivery's inner PipeSender<WebSocketMessage>.
        //    The WS task's outbound_rx drains it. Zero bridge tasks.
        let outbound_pipe = delivery.pipe().clone();
        let send_body: Arc<dyn super::SendBody> = Arc::new(
            outbound_pipe.map_to(|b: &Bytes| WebSocketMessage::Binary(b.to_vec()))
        );

        // 4. Spawn the WS task on valtron, bridge its output stream → body_tx.
        let inner = execute(task, None).map_err(|e| {
            TransportError::Connect(Arc::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("execute ws task: {e}"),
            )))
        })?;

        valtron::send(valtron::from_future(async move {
            let mut stream = inner;
            loop {
                match stream.next() {
                    Some(Stream::Next(Ok(WebSocketMessage::ConnectionEstablished))) => {
                        let _ = head_tx.try_send((
                            Status::SwitchingProtocols,
                            SimpleHeaders::new(),
                        ));
                    }
                    Some(Stream::Next(Ok(WebSocketMessage::Binary(data)))) => {
                        if body_tx.send(Bytes::from(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Stream::Next(Ok(WebSocketMessage::Close(..)))) | None => {
                        body_tx.close();
                        trailer_tx.close();
                        break;
                    }
                    _ => continue,
                }
            }
        }))
        .map_err(|e| {
            TransportError::Connect(Arc::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("spawn inbound bridge: {e}"),
            )))
        })?;

        Ok(TransportStream {
            send_body,
            head: head_stream_from_pipe(head_rx),
            recv_body: body_stream_from_pipe(body_rx),
            trailers: trailer_rx,
        })
    }
}
