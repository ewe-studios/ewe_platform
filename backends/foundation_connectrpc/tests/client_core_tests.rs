//! Client-core tests (F24): `Client<Req,Res>`, `ClientOptions`, per-call `Ctx`
//! derivation, and the four stream facades (Decision 07).
//!
//! HOW: A `MockTransport` that pre-wires valtron pipes lets the test drive the
//! client's async RPC methods. The mock server is spawned as a separate valtron
//! task so the client and server run concurrently via the executor.

use std::sync::Arc;

use bytes::Bytes;
use foundation_core::valtron::{self, Pipe, PipeReceiver, PipeSender};
use foundation_core::valtron::valtron_test;
use foundation_netio::shared::http::{
    Proto, RequestDescriptor, SimpleHeaders, Status,
};

use foundation_connectrpc::client::{
    Client, ClientOptions, ProtocolSelection,
};
use foundation_connectrpc::codec::{Codec, CodecError, CodecFor, ProcedureCodecs};
use foundation_connectrpc::context::Ctx;
use foundation_connectrpc::error::Code;
use foundation_connectrpc::message::Request;
use foundation_connectrpc::transport::{
    Transport, TransportCapabilities, TransportError, TransportStream,
    head_stream_from_pipe, body_stream_from_pipe,
};

// ============================================================================
// IdentityCodec — a pass-through codec for `Vec<u8>` messages
// ============================================================================

/// Pass-through codec: marshal/unmarshal just copies bytes. Wire name is `"proto"`
/// to match the default in `ClientOptions`.
struct IdentityCodec;

impl Codec for IdentityCodec {
    fn name(&self) -> &str { "proto" }
    fn is_binary(&self) -> bool { true }
}

impl CodecFor<Vec<u8>> for IdentityCodec {
    fn marshal(&self, message: &Vec<u8>) -> Result<Bytes, CodecError> {
        Ok(Bytes::copy_from_slice(message))
    }
    fn unmarshal(&self, data: Bytes) -> Result<Vec<u8>, CodecError> {
        if data.is_empty() {
            return Err(CodecError::ZeroLength { codec: "identity" });
        }
        Ok(data.to_vec())
    }
    fn marshal_stable(&self, message: &Vec<u8>) -> Result<Bytes, CodecError> {
        Ok(Bytes::copy_from_slice(message))
    }
    fn marshal_append(&self, buf: &mut Vec<u8>, message: &Vec<u8>) -> Result<(), CodecError> {
        buf.extend_from_slice(message);
        Ok(())
    }
}

// ============================================================================
// MockTransport — pipe-backed mock that stores client-side pipes for open()
// ============================================================================

struct MockPipes {
    req_tx: PipeSender<Bytes>,
    head_rx: PipeReceiver<(Status, SimpleHeaders)>,
    resp_rx: PipeReceiver<Bytes>,
}

/// A pipe-backed mock transport. Pre-configure with pipes before use.
struct MockTransport {
    caps: TransportCapabilities,
    pipes: Arc<std::sync::Mutex<Option<MockPipes>>>,
}

impl MockTransport {
    /// Build a mock transport that supports everything (h1+h2, duplex, streaming).
    fn full_duplex() -> Self {
        Self {
            caps: TransportCapabilities {
                request_streaming: true,
                full_duplex: true,
                h2_trailers: true,
                http_versions: &[Proto::HTTP11, Proto::HTTP20],
                multiplexed: false,
            },
            pipes: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Build an h1-only mock (no duplex, no h2 trailers).
    fn h1_only() -> Self {
        Self {
            caps: TransportCapabilities {
                request_streaming: true,
                full_duplex: false,
                h2_trailers: false,
                http_versions: &[Proto::HTTP11],
                multiplexed: false,
            },
            pipes: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Pre-configure the pipes for the next `open()` call. Returns `ServerPipes`
    /// the test uses to inspect the request and send responses.
    fn prepare(&self) -> ServerPipes {
        let (req_tx, req_rx) = Pipe::with_depth(4);
        let (head_tx, head_rx) = Pipe::with_depth(1);
        let (resp_tx, resp_rx) = Pipe::with_depth(4);

        *self.pipes.lock().unwrap() = Some(MockPipes {
            req_tx,
            head_rx,
            resp_rx,
        });

        ServerPipes {
            req_rx,
            head_tx,
            resp_tx,
        }
    }
}

impl Transport for MockTransport {
    fn capabilities(&self) -> TransportCapabilities {
        self.caps.clone()
    }

    fn open(&self, _request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        let pipes = self
            .pipes
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| TransportError::Protocol("mock: no pipes prepared".into()))?;

        let (trailer_tx, trailer_rx) = Pipe::<SimpleHeaders>::with_depth(1);
        trailer_tx.close();
        Ok(TransportStream {
            send_body: Arc::new(pipes.req_tx),
            head: head_stream_from_pipe(pipes.head_rx),
            recv_body: body_stream_from_pipe(pipes.resp_rx),
            trailers: trailer_rx,
        })
    }
}

/// The "server" (test) side of a MockTransport exchange.
struct ServerPipes {
    /// Request body bytes sent by the client.
    req_rx: PipeReceiver<Bytes>,
    /// Response head to deliver to the client.
    head_tx: PipeSender<(Status, SimpleHeaders)>,
    /// Response body bytes to deliver to the client.
    resp_tx: PipeSender<Bytes>,
}

impl ServerPipes {
    /// Await and return the complete request body the client sent. Uses `&self`
    /// because `PipeReceiver::receive` uses interior mutability.
    async fn read_request_body(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        while let Some(chunk) = self.req_rx.receive().await {
            buf.extend_from_slice(&chunk);
        }
        buf
    }

    /// Send a successful unary response head + body. The `resp_tx` pipe is
    /// closed when `ServerPipes` is dropped, which signals end-of-stream to
    /// the client.
    async fn send_unary_response(&self, body: Bytes) {
        let _ = self
            .head_tx
            .send((Status::OK, SimpleHeaders::new()))
            .await;
        let _ = self.resp_tx.send(body).await;
    }
}

// ============================================================================
// Sync tests (no valtron needed)
// ============================================================================

#[test]
fn identity_codec_roundtrip() {
    let codec = IdentityCodec;
    let msg = b"hello world".to_vec();
    let encoded = codec.marshal(&msg).unwrap();
    assert_eq!(&encoded[..], b"hello world");
    let decoded = codec.unmarshal(encoded).unwrap();
    assert_eq!(decoded, b"hello world");
}

#[test]
fn protocol_selection_to_kind() {
    use foundation_connectrpc::ProtocolKind;
    assert_eq!(ProtocolSelection::Connect.to_kind(), ProtocolKind::Connect);
    assert_eq!(ProtocolSelection::Grpc.to_kind(), ProtocolKind::Grpc);
    assert_eq!(ProtocolSelection::GrpcWeb.to_kind(), ProtocolKind::GrpcWeb);
}

#[test]
fn client_options_builder_defaults() {
    let opts = ClientOptions::new();
    let _opts = opts
        .with_codec("json")
        .with_send_gzip()
        .with_read_max_bytes(65536)
        .with_send_max_bytes(65536)
        .with_connect()
        .with_timeout(std::time::Duration::from_secs(30));
}

#[test]
fn client_init_unknown_codec() {
    let transport = MockTransport::full_duplex();
    let codecs = ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec);

    let result = Client::new(
        Arc::new(transport),
        "http://localhost:8080",
        codecs,
        ClientOptions::new().with_codec("nonexistent"),
    );

    match result {
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            assert!(msg.contains("codec") || msg.contains("nonexistent"),
                "expected a codec-not-found error, got: {e}");
        }
        Ok(_) => panic!("expected error for unknown codec"),
    }
}

#[test]
fn client_init_grpc_on_h1_fails() {
    let transport = MockTransport::h1_only();
    let codecs = ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec);

    let result = Client::new(
        Arc::new(transport),
        "http://localhost:8080",
        codecs,
        ClientOptions::new().with_grpc(),
    );

    assert!(result.is_err(), "expected error for gRPC on h1-only transport");
}

// ============================================================================
// Async tests via valtron tasks
// ============================================================================

/// Run the mock server and a client.unary call concurrently.
///
/// WHY a valtron task instead of `futures::join!`: valtron tasks are sequential —
/// when a task parks (e.g. on a pipe read), the entire future tree suspends, so
/// `join!` branches cannot make progress. Spawning the server as a separate
/// valtron task lets the executor schedule them concurrently.
fn spawn_unary_server(
    server: ServerPipes,
    expected_req: Vec<u8>,
    resp_body: Vec<u8>,
) {
    let _ = foundation_core::valtron::send(foundation_core::valtron::from_future(async move {
        let req_body = server.read_request_body().await;
        assert_eq!(req_body, expected_req);
        server
            .send_unary_response(Bytes::from(resp_body))
            .await;
    }));
}

#[valtron_test]
async fn client_unary_connect_round_trip() {
    let mock = MockTransport::full_duplex();
    let server = mock.prepare();
    let transport: Arc<dyn Transport> = Arc::new(mock);

    let codecs = ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec);
    let client = Client::new(
        Arc::clone(&transport),
        "http://localhost:8080",
        codecs,
        ClientOptions::new(),
    )
    .unwrap();

    spawn_unary_server(server, b"ping".to_vec(), b"pong".to_vec());

    let ctx = Ctx::background().with_deadline(std::time::Duration::from_secs(10));
    let request = Request::new(b"ping".to_vec());

    let response = client.unary(ctx, request).await.unwrap();
    assert_eq!(response.msg, b"pong".to_vec());
}

#[valtron_test]
async fn client_unary_error_status() {
    let mock = MockTransport::full_duplex();
    let server = mock.prepare();
    let transport: Arc<dyn Transport> = Arc::new(mock);

    let codecs = ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec);
    let client = Client::new(
        Arc::clone(&transport),
        "http://localhost:8080",
        codecs,
        ClientOptions::new(),
    )
    .unwrap();

    // Spawn the server as a valtron task.
    let _ = valtron::send(valtron::from_future(async move {
        let _req_body = server.read_request_body().await;
        let _ = server
            .head_tx
            .send((Status::NotFound, SimpleHeaders::new()))
            .await;
        let _ = server.resp_tx.send(Bytes::from_static(b"not found")).await;
    }));

    let ctx = Ctx::background().with_deadline(std::time::Duration::from_secs(10));
    let request = Request::new(b"ping".to_vec());

    match client.unary(ctx, request).await {
        Err(e) => {
            assert_eq!(e.current_context().code(), Code::NotFound);
        }
        Ok(_) => panic!("expected error for 404 status"),
    }
}

/// Wrap bytes in a gRPC envelope: flags=0x00 | 4-byte BE len | payload.
fn grpc_envelope(payload: &[u8]) -> Vec<u8> {
    let mut envelope = vec![0u8; 5];
    envelope[1..5].copy_from_slice(&(payload.len() as u32).to_be_bytes());
    envelope.extend_from_slice(payload);
    envelope
}

#[valtron_test]
async fn client_unary_with_grpc_web() {
    let mock = MockTransport::full_duplex();
    let server = mock.prepare();
    let transport: Arc<dyn Transport> = Arc::new(mock);

    let codecs = ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec);
    let client = Client::new(
        Arc::clone(&transport),
        "http://localhost:8080",
        codecs,
        ClientOptions::new().with_grpc_web(),
    )
    .unwrap();

    // gRPC-Web unary wraps the request in a single envelope. The client's
    // encode_unary_request handles this — the mock sees the envelope on the wire.
    let req_envelope = grpc_envelope(b"ping");
    let resp_envelope = grpc_envelope(b"pong");
    spawn_unary_server(server, req_envelope, resp_envelope);

    let ctx = Ctx::background().with_deadline(std::time::Duration::from_secs(10));
    let request = Request::new(b"ping".to_vec());

    // The client unwraps the response envelope; the caller sees b"pong".
    let response = client.unary(ctx, request).await.unwrap();
    assert_eq!(response.msg, b"pong".to_vec());
}

// ============================================================================
// Client construction + config tests (sync)
// ============================================================================

#[test]
fn client_construction_succeeds_with_defaults() {
    let mock = MockTransport::full_duplex();
    let transport: Arc<dyn Transport> = Arc::new(mock);
    let codecs = ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec);
    let _client = Client::new(
        transport,
        "https://api.example.com",
        codecs,
        ClientOptions::new(),
    )
    .unwrap();
}

#[test]
fn client_config_debug() {
    let opts = ClientOptions::new()
        .with_timeout(std::time::Duration::from_secs(5))
        .with_codec("json");
    let debug_str = format!("{:?}", opts);
    assert!(debug_str.contains("ClientOptions"));
    assert!(debug_str.contains("timeout"));
}
