//! The WebSocket driver.
//!
//! The driver speaks the Durable Streams JSON schema over the platform
//! WebSocket client. The schema carries the same protocol operations as the
//! other transports. The encoding is base64 for bytes and one text frame per
//! request and reply.

use std::net::SocketAddr;
use std::time::Duration;

use base64::Engine as _;
use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::websocket::{
    MessageDelivery, WebSocketClient, WebSocketEvent, WebSocketMessage,
};
use serde_json::{json, Value};

use crate::driver::{Driver, ErrorKind, Observation, Producer, StepResult};

/// Encode bytes as standard base64 with padding.
fn encode(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Decode standard base64 with padding.
fn decode(text: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .expect("a base64 chunk")
}

/// Map the schema error name to a transport-neutral kind.
fn error_kind(name: &str) -> ErrorKind {
    match name {
        "not_found" => ErrorKind::NotFound,
        "gone" => ErrorKind::Gone,
        "closed" => ErrorKind::Closed,
        "config_conflict" | "content_type_mismatch" | "stream_seq_conflict" => ErrorKind::Conflict,
        "bad_request" => ErrorKind::BadRequest,
        "unsupported" => ErrorKind::Unsupported,
        other => ErrorKind::Other(other.to_string()),
    }
}

/// The Durable Streams WebSocket client.
pub struct WsDriver {
    client: WebSocketClient,
    delivery: MessageDelivery,
    next_id: u64,
}

impl WsDriver {
    /// Connect to the WebSocket route of the in-process server.
    ///
    /// # Panics
    /// Panics when the client cannot open the connection.
    #[must_use]
    pub fn connect(address: SocketAddr) -> Self {
        let url = format!("ws://127.0.0.1:{}/ws", address.port());
        let (client, delivery) = WebSocketClient::connect(
            SystemDnsResolver,
            url,
            Duration::from_secs(10),
            Duration::from_millis(10),
        )
        .expect("websocket connect");
        Self {
            client,
            delivery,
            next_id: 1,
        }
    }

    /// Send one request and return the reply with the same identifier.
    fn request(&mut self, mut message: Value) -> Value {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        message["id"] = json!(id);
        self.delivery
            .send(WebSocketMessage::Text(message.to_string()))
            .expect("websocket send");
        loop {
            for event in self.client.messages() {
                match event.expect("websocket event") {
                    WebSocketEvent::Skip => {}
                    WebSocketEvent::Message(WebSocketMessage::Text(text)) => {
                        let reply: Value = serde_json::from_str(&text).expect("a JSON reply");
                        if reply["id"].as_u64() == Some(id) {
                            return reply;
                        }
                    }
                    WebSocketEvent::Message(other) => {
                        panic!("unexpected websocket message: {other:?}");
                    }
                }
            }
        }
    }
}

/// Build the observation of one reply.
fn reply_step(reply: &Value) -> StepResult {
    if reply["ok"].as_bool() != Some(true) {
        let name = reply["error"]["kind"].as_str().unwrap_or("bad_request");
        return StepResult::new(Observation::error(error_kind(name)), None);
    }
    let op = reply["op"].as_str().unwrap_or_default();
    let offset = reply["offset"].as_str().map(str::to_string);
    match op {
        "create" | "fork" => {
            let status = if reply["created"].as_bool() == Some(true) {
                201
            } else {
                200
            };
            let observation = Observation::status(status)
                .with_next_offset(offset.is_some())
                .with_closed(reply["closed"].as_bool().unwrap_or(false));
            StepResult::new(observation, offset)
        }
        "append" | "close" => {
            let observation = Observation::status(204)
                .with_next_offset(offset.is_some())
                .with_closed(reply["closed"].as_bool().unwrap_or(false))
                .with_duplicate(reply["duplicate"].as_bool().unwrap_or(false));
            StepResult::new(observation, offset)
        }
        "read" => {
            let body: Vec<u8> = reply["chunks"].as_array().map_or_else(Vec::new, |chunks| {
                chunks
                    .iter()
                    .filter_map(Value::as_str)
                    .flat_map(decode)
                    .collect()
            });
            let observation = Observation::status(200)
                .with_body(body)
                .with_next_offset(offset.is_some())
                .with_up_to_date(reply["up_to_date"].as_bool().unwrap_or(false))
                .with_closed(reply["closed"].as_bool().unwrap_or(false));
            StepResult::new(observation, offset)
        }
        other => panic!("unexpected websocket reply op: {other}"),
    }
}

// The schema exchange is synchronous; the trait is async for one uniform call
// shape across transports.
#[allow(clippy::unused_async_trait_impl)]
impl Driver for WsDriver {
    async fn create(
        &mut self,
        stream: &str,
        content_type: &str,
        ttl_ms: Option<u64>,
    ) -> StepResult {
        let mut message = json!({
            "op": "create",
            "stream": stream,
            "content_type": content_type,
        });
        if let Some(ttl) = ttl_ms {
            message["ttl_ms"] = json!(ttl);
        }
        let reply = self.request(message);
        reply_step(&reply)
    }

    async fn append(
        &mut self,
        stream: &str,
        data: &[u8],
        content_type: &str,
        producer: Option<&Producer>,
        stream_seq: Option<&str>,
        close: bool,
    ) -> StepResult {
        let mut message = json!({
            "op": "append",
            "stream": stream,
            "data": encode(data),
            "content_type": content_type,
            "close": close,
        });
        if let Some(seq) = stream_seq {
            message["stream_seq"] = json!(seq);
        }
        if let Some(producer) = producer {
            message["producer_id"] = json!(producer.id);
            message["producer_epoch"] = json!(producer.epoch);
            message["producer_seq"] = json!(producer.seq);
        }
        let reply = self.request(message);
        reply_step(&reply)
    }

    async fn close(&mut self, stream: &str, data: &[u8], _content_type: &str) -> StepResult {
        let reply = self.request(json!({
            "op": "close",
            "stream": stream,
            "data": encode(data),
        }));
        reply_step(&reply)
    }

    async fn read(&mut self, stream: &str, offset: Option<&str>) -> StepResult {
        let mut message = json!({
            "op": "read",
            "stream": stream,
        });
        if let Some(offset) = offset {
            message["offset"] = json!(offset);
        }
        let reply = self.request(message);
        reply_step(&reply)
    }

    async fn fork(&mut self, child: &str, source: &str, at: Option<&str>) -> StepResult {
        let mut message = json!({
            "op": "fork",
            "stream": child,
            "from": source,
        });
        if let Some(at) = at {
            message["offset"] = json!(at);
        }
        let reply = self.request(message);
        reply_step(&reply)
    }

    async fn wait(&mut self, duration: core::time::Duration) {
        foundation_core::valtron::timer::sleep_for(duration, &foundation_context::Ctx::host())
            .await;
    }
}
