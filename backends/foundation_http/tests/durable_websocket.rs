//! End-to-end tests for the Durable Streams WebSocket transport.
//!
//! Each test starts a real `HttpServer` with a WebSocket route, connects with
//! the platform WebSocket client and drives the protocol over JSON text
//! frames. The server and the client run in one process.

#![cfg(all(feature = "multi", unix))]

use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use foundation_core::durable::{DurableStream, MemoryVfs};
use foundation_core::valtron::initialize_pool;
use foundation_http::native::durable_ws::{
    DurableWsConfig, DurableWsHandler, WsOp, WsReply, WsRequest,
};
use foundation_http::native::serve::Serve;
use foundation_http::native::server::{HttpServer, KeepAliveConfig, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::ShutdownSignal;
use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::shared::http::SimpleMethod;
use foundation_netio::websocket::{WebSocketClient, WebSocketEvent, WebSocketMessage};
use serde_json::{json, Value};
use serial_test::serial;

/// Start one in-process server on an ephemeral loopback port.
fn start_server(app: HttpApp<Arc<dyn Serve>>) -> (std::net::SocketAddr, Arc<ShutdownSignal>) {
    let shutdown = Arc::new(ShutdownSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind failed");
    let addr = listener.local_addr().expect("local_addr failed");

    let config = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));
    let bind_addr = format!("127.0.0.1:{}", addr.port());
    let server = HttpServer::with_config(app, &bind_addr, config);

    let shutdown_thread = shutdown.clone();
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &shutdown_thread);
    });

    (addr, shutdown)
}

/// Build an application that mounts the WebSocket transport on `GET /ws`.
fn durable_ws_app(config: DurableWsConfig) -> HttpApp<Arc<dyn Serve>> {
    let ctx = Ctx::host();
    let _ = ctx.get_or_insert(DurableStream::new(Arc::new(MemoryVfs::new()), &ctx));
    let _ = ctx.get_or_insert(config);
    let mut app = HttpApp::with_context(ctx);
    app.route::<DurableWsHandler>(SimpleMethod::GET, "/ws");
    app
}

/// One WebSocket client with a send handle.
struct WsClient {
    client: WebSocketClient,
    delivery: foundation_netio::websocket::MessageDelivery,
}

impl WsClient {
    fn connect(addr: std::net::SocketAddr) -> Self {
        let url = format!("ws://127.0.0.1:{}/ws", addr.port());
        let (client, delivery) = WebSocketClient::connect(
            SystemDnsResolver,
            url,
            Duration::from_secs(10),
            Duration::from_millis(10),
        )
        .expect("websocket connect");
        Self { client, delivery }
    }

    fn send(&self, value: &Value) {
        self.delivery
            .send(WebSocketMessage::Text(value.to_string()))
            .expect("websocket send");
    }

    fn recv(&mut self) -> Value {
        for event in self.client.messages() {
            match event.expect("websocket event") {
                WebSocketEvent::Skip => {}
                WebSocketEvent::Message(WebSocketMessage::Text(text)) => {
                    return serde_json::from_str(&text).expect("a JSON reply");
                }
                WebSocketEvent::Message(other) => panic!("unexpected message: {other:?}"),
            }
        }
        panic!("the WebSocket stream ended before a reply arrived");
    }

    fn recv_until(&mut self, id: u64, op: &str) -> Value {
        loop {
            let reply = self.recv();
            if reply["id"].as_u64() == Some(id) && reply["op"].as_str() == Some(op) {
                return reply;
            }
        }
    }

    fn recv_close(&mut self) -> (u16, String) {
        for event in self.client.messages() {
            match event.expect("websocket event") {
                WebSocketEvent::Skip => {}
                WebSocketEvent::Message(WebSocketMessage::Close(code, reason)) => {
                    return (code, reason);
                }
                WebSocketEvent::Message(other) => panic!("unexpected message: {other:?}"),
            }
        }
        panic!("the WebSocket stream ended before a Close frame arrived");
    }
}

/// Concatenate the base64 chunks of a read reply.
fn decode_chunks(reply: &Value) -> Vec<u8> {
    reply["chunks"]
        .as_array()
        .expect("a chunks list")
        .iter()
        .flat_map(|chunk| {
            base64::engine::general_purpose::STANDARD
                .decode(chunk.as_str().expect("chunk text"))
                .expect("base64 chunk")
        })
        .collect()
}

/// Create, append, read byte-exact, head, fork, delete and close over frames.
#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn websocket_core_roundtrip() {
    let _guard = initialize_pool(42, Some(8));
    let (addr, shutdown) = start_server(durable_ws_app(DurableWsConfig::default()));
    let mut client = WsClient::connect(addr);

    client.send(&json!({
        "id": 1,
        "op": "create",
        "stream": "ws/roundtrip",
        "content_type": "application/octet-stream"
    }));
    let created = client.recv();
    assert_eq!(created["id"].as_u64(), Some(1));
    assert_eq!(created["ok"], true);
    assert_eq!(created["op"].as_str(), Some("create"));
    assert_eq!(created["status"].as_u64(), Some(201));
    assert_eq!(created["created"], true);
    assert!(
        created["offset"].is_string(),
        "create reports the tail offset"
    );

    client.send(&json!({
        "id": 2,
        "op": "append",
        "stream": "ws/roundtrip",
        "data": base64::engine::general_purpose::STANDARD.encode(b"hello "),
        "content_type": "application/octet-stream"
    }));
    let first = client.recv();
    assert_eq!(first["op"].as_str(), Some("append"));
    assert_eq!(first["status"].as_u64(), Some(204));

    client.send(&json!({
        "id": 3,
        "op": "append",
        "stream": "ws/roundtrip",
        "data": base64::engine::general_purpose::STANDARD.encode(b"world"),
        "content_type": "application/octet-stream"
    }));
    let second = client.recv();
    assert_eq!(second["status"].as_u64(), Some(204));
    let tail = second["offset"].as_str().expect("tail offset").to_string();

    client.send(&json!({"id": 4, "op": "read", "stream": "ws/roundtrip"}));
    let read = client.recv();
    assert_eq!(read["op"].as_str(), Some("read"));
    assert_eq!(read["up_to_date"], true);
    assert_eq!(read["offset"].as_str(), Some(tail.as_str()));
    assert_eq!(
        decode_chunks(&read),
        b"hello world",
        "the read is byte-exact"
    );

    client.send(&json!({"id": 5, "op": "head", "stream": "ws/roundtrip"}));
    let head = client.recv();
    assert_eq!(head["op"].as_str(), Some("head"));
    assert_eq!(
        head["content_type"].as_str(),
        Some("application/octet-stream")
    );
    assert_eq!(head["offset"].as_str(), Some(tail.as_str()));
    assert_eq!(head["closed"], false);

    client.send(&json!({
        "id": 6,
        "op": "fork",
        "stream": "ws/fork",
        "from": "ws/roundtrip"
    }));
    let fork = client.recv();
    assert_eq!(fork["op"].as_str(), Some("fork"));
    assert_eq!(fork["status"].as_u64(), Some(201));
    assert_eq!(fork["created"], true);

    client.send(&json!({"id": 7, "op": "read", "stream": "ws/fork"}));
    assert_eq!(decode_chunks(&client.recv()), b"hello world");

    client.send(&json!({"id": 8, "op": "delete", "stream": "ws/fork"}));
    let deleted = client.recv();
    assert_eq!(deleted["op"].as_str(), Some("delete"));
    assert_eq!(deleted["soft_deleted"], false);

    client.send(&json!({"id": 9, "op": "close", "stream": "ws/roundtrip"}));
    let closed = client.recv();
    assert_eq!(closed["op"].as_str(), Some("close"));
    assert_eq!(closed["closed"], true);

    client.send(&json!({"id": 10, "op": "read", "stream": "ws/roundtrip"}));
    let after_close = client.recv();
    assert_eq!(after_close["closed"], true);
    assert_eq!(after_close["up_to_date"], true);
    assert_eq!(decode_chunks(&after_close), b"hello world");

    client.send(&json!({
        "id": 11,
        "op": "append",
        "stream": "ws/roundtrip",
        "data": base64::engine::general_purpose::STANDARD.encode(b"x"),
        "content_type": "application/octet-stream"
    }));
    let refused = client.recv();
    assert_eq!(refused["ok"], false);
    assert_eq!(refused["error"]["kind"].as_str(), Some("closed"));
    assert_eq!(refused["error"]["status"].as_u64(), Some(409));

    shutdown.turn_on();
}

/// A live read receives an append that lands after it opened.
#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn live_read_receives_an_append_after_open() {
    let _guard = initialize_pool(42, Some(8));
    let (addr, shutdown) = start_server(durable_ws_app(DurableWsConfig::default()));
    let mut reader = WsClient::connect(addr);

    reader.send(&json!({
        "id": 1,
        "op": "create",
        "stream": "ws/live",
        "content_type": "application/octet-stream"
    }));
    assert_eq!(reader.recv()["status"].as_u64(), Some(201));

    reader.send(&json!({
        "id": 10,
        "op": "read",
        "stream": "ws/live",
        "offset": "now",
        "live": true
    }));
    let opened = reader.recv();
    assert_eq!(opened["id"].as_u64(), Some(10));
    assert_eq!(opened["op"].as_str(), Some("read"));
    assert_eq!(
        opened["chunks"].as_array().expect("chunks").len(),
        0,
        "the first live frame has no chunks"
    );
    assert_eq!(opened["up_to_date"], true);
    assert_eq!(opened["closed"], false);

    // A second connection appends while the first is live.
    let mut writer = WsClient::connect(addr);
    writer.send(&json!({
        "id": 11,
        "op": "append",
        "stream": "ws/live",
        "data": base64::engine::general_purpose::STANDARD.encode(b"second"),
        "content_type": "application/octet-stream"
    }));
    assert_eq!(writer.recv()["status"].as_u64(), Some(204));

    let data = reader.recv_until(10, "data");
    assert_eq!(decode_chunks(&data), b"second");
    assert_eq!(data["up_to_date"], true);

    reader.send(&json!({"id": 12, "op": "cancel", "read": 10}));
    let end = reader.recv_until(10, "end");
    assert_eq!(end["cancelled"], true);

    shutdown.turn_on();
}

/// A live read ends when the stream closes, after its last page.
#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn live_read_ends_when_the_stream_closes() {
    let _guard = initialize_pool(42, Some(8));
    let (addr, shutdown) = start_server(durable_ws_app(DurableWsConfig::default()));
    let mut reader = WsClient::connect(addr);

    reader.send(&json!({
        "id": 1,
        "op": "create",
        "stream": "ws/close",
        "content_type": "application/octet-stream"
    }));
    assert_eq!(reader.recv()["status"].as_u64(), Some(201));

    reader.send(&json!({
        "id": 30,
        "op": "read",
        "stream": "ws/close",
        "offset": "now",
        "live": true
    }));
    assert_eq!(reader.recv()["op"].as_str(), Some("read"));

    let mut writer = WsClient::connect(addr);
    writer.send(&json!({
        "id": 31,
        "op": "append",
        "stream": "ws/close",
        "data": base64::engine::general_purpose::STANDARD.encode(b"final"),
        "content_type": "application/octet-stream",
        "close": true
    }));
    let appended = writer.recv();
    assert_eq!(appended["status"].as_u64(), Some(204));
    assert_eq!(appended["closed"], true);

    let data = reader.recv_until(30, "data");
    assert_eq!(decode_chunks(&data), b"final");
    let end = reader.recv_until(30, "end");
    assert_eq!(end["closed"], true);

    shutdown.turn_on();
}

/// The frame size cap is configuration: an oversized request closes the socket.
#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn frame_cap_is_configuration() {
    let _guard = initialize_pool(42, Some(8));
    let (addr, shutdown) = start_server(durable_ws_app(
        DurableWsConfig::default().with_max_frame_bytes(64),
    ));
    let mut client = WsClient::connect(addr);

    client.send(&json!({
        "id": 1,
        "op": "create",
        "stream": "ws/frame-cap/is-longer-than-sixty-four-bytes",
        "content_type": "application/octet-stream"
    }));
    let (code, reason) = client.recv_close();
    assert_eq!(code, 1009, "an oversized frame closes with message too big");
    assert_ne!(reason.as_str(), "", "the close frame carries a reason");

    shutdown.turn_on();
}

/// The request and reply schema is public and round-trips as JSON.
#[test]
fn schema_round_trips() {
    let request = WsRequest::parse(r#"{"id":7,"op":"append","stream":"a","data":"aGk="}"#)
        .expect("a valid request");
    assert_eq!(request.id, 7);
    match request.op {
        WsOp::Append {
            stream,
            data,
            close,
            ..
        } => {
            assert_eq!(stream, "a");
            assert_eq!(data.as_deref(), Some("aGk="));
            assert!(!close);
        }
        other => panic!("unexpected operation: {other:?}"),
    }

    let reply = WsReply::transport_error(7, "bad_request", "no live read");
    let value: Value =
        serde_json::from_str(&reply.to_text().expect("reply text")).expect("reply JSON");
    assert_eq!(value["id"].as_u64(), Some(7));
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["kind"].as_str(), Some("bad_request"));
}
