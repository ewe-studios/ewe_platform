//! WebSocket Transport — Connect envelopes over WS messages (F39).
//!
//! `WsTransport::open()` splits the WS task's output with `split_collector_map`
//! (same pattern as `h1.rs`): `ConnectionEstablished` → head observer,
//! `Binary(data)` → body observer. No spawned bridge tasks — both sides
//! are zero-task.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::url::Uri;
use foundation_core::valtron::{self, CollectionState, Pipe, StreamIteratorExt, TaskIteratorExt};
use foundation_netio::shared::client::dns::SystemDnsResolver;
use foundation_netio::shared::http::{
    Proto, RequestDescriptor, SimpleHeaders, Status,
};
use foundation_netio::websocket::native::connection::{Reconnect, WebSocketClient};
use foundation_netio::websocket::shared::error::WebSocketError;
use foundation_netio::websocket::shared::message::WebSocketMessage;

use super::{
    SendBody, Transport, TransportCapabilities, TransportError, TransportStream,
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

        // 1. Get spawned WS task + MessageDelivery from WebSocketClient.
        let (task, delivery) = WebSocketClient::connect_parts(
            SystemDnsResolver::default(),
            ws_url,
            Reconnect::No,
            READ_TIMEOUT,
            SLEEP_TIMEOUT,
        )
        .map_err(|e| TransportError::Protocol(format!("ws connect: {e}")))?;

        // 2. Outbound: MappedSender converts Bytes → Binary → delivery's pipe.
        //    Zero bridge tasks.
        let outbound_pipe = delivery.pipe().clone();
        let send_body: Arc<dyn SendBody> = Arc::new(
            outbound_pipe.map_to(|b: &Bytes| WebSocketMessage::Binary(b.to_vec()))
        );

        // 3. Inbound: split the task's Ready values into head + body observers
        //    (same pattern as h1.rs — split_collector_map fans output into pipes).
        //
        //    First split: head (ConnectionEstablished → 101 + Ok, close after one).
        let (head_obs, head_tail) = task.split_collect_until_map(
            |item: &Result<WebSocketMessage, WebSocketError>| match item {
                Ok(WebSocketMessage::ConnectionEstablished) => (
                    CollectionState::Close(true),
                    Some(Ok((Status::SwitchingProtocols, SimpleHeaders::new()))),
                ),
                Err(e) => (
                    CollectionState::Close(true),
                    Some(Err(TransportError::Connect(Arc::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("ws handshake failed: {e}"),
                    ))))),
                ),
                _ => (CollectionState::Skip, None),
            },
            1,
        );

        //    Second split: body (Binary → Bytes).
        let (body_obs, body_tail) = head_tail.split_collector_map(
            |item: &Result<WebSocketMessage, WebSocketError>| match item {
                Ok(WebSocketMessage::Binary(data)) => (
                    true,
                    Some(Ok(Bytes::from(data.clone()))),
                ),
                Ok(WebSocketMessage::Close(..)) => (true, None),
                Err(e) => (
                    true,
                    Some(Err(TransportError::Connect(Arc::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("ws stream error: {e}"),
                    ))))),
                ),
                _ => (false, None),
            },
            64,
        );

        //    Drive: the final continuation must still be consumed. map_ready(|_| ())
        //    terminates it — no buffered delivery queue leaking.
        let drive = body_tail.map_ready(|_| ());
        valtron::send(drive).map_err(|e| {
            TransportError::Connect(Arc::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;

        // 4. TransportStream — head/body as erased streams, send_body as Arc<dyn SendBody>.
        let head = Box::pin(head_obs.into_next_stream());
        let recv_body = Box::pin(body_obs.into_next_stream());
        let (trailer_tx, trailer_rx) = Pipe::<SimpleHeaders>::with_depth(1);
        trailer_tx.close(); // WS has no h2 trailers

        Ok(TransportStream {
            send_body,
            head,
            recv_body,
            trailers: trailer_rx,
        })
    }
}
