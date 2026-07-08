//! Unary RPC over a real loopback socket — the "hello world" of
//! `foundation_connectrpc`.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p foundation_connectrpc --example unary_echo
//! ```
//!
//! What it shows, end to end:
//!   1. **Server** — build a [`Router`], register one unary procedure with a
//!      handler, wrap it in [`ConnectRpcServe`], and serve it from a real
//!      `HttpServer` bound to a loopback port (the per-connection task is the
//!      server-side connection owner, Decision 11).
//!   2. **Client** — build an [`H1Transport`] and a typed [`Client`], then call
//!      `client.unary(ctx, request)` and await the typed response.
//!
//! The whole thing runs under one valtron pool via `#[valtron]` (the engine
//! equivalent of `#[tokio::main]`); the blocking `HttpServer` loop runs on its
//! own OS thread so the client task can make progress concurrently.

use std::sync::Arc;
use std::time::Duration;

// buffa (and its `bytes` re-export) reached through foundation_connectrpc — no
// direct buffa/bytes dependency needed in a consumer crate.
use foundation_connectrpc::buffa::bytes::{Buf, BufMut};
use foundation_connectrpc::buffa::encoding::{decode_varint, encode_varint, skip_field, Tag, WireType};
use foundation_connectrpc::buffa::{DecodeContext, DecodeError, DefaultInstance, Message, SizeCache};

use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;
use foundation_netio::simple_http::client::SimpleHttpClient;

use foundation_connectrpc::transport::Transport;
use foundation_connectrpc::{
    ClientOptions, ConnectResult, ConnectRpcServe, Ctx, H1Transport, HandlerOptions,
    IdempotencyLevel, ProcedureCodecs, Request, Response, Router, Client,
};

/// The fully-qualified procedure path (leading slash, `/<pkg>.<Service>/<Method>`).
const PROCEDURE: &str = "/demo.EchoService/Echo";

// ── The message type ─────────────────────────────────────────────────────────
//
// In real use this is *generated* — either from a `.proto` by
// `foundation_connectrpc_codegen`, or from a Rust trait by
// `#[service]`. `JsonCodec: CodecFor<M>` is bounded on
// `M: buffa::Message + Serialize + DeserializeOwned`, so we hand-write a minimal
// `Message` impl here. The JSON wire path only touches the serde impls; the
// protobuf methods below satisfy the bound and are exercised by the proto codec.

#[derive(Clone, Default, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
struct EchoMessage {
    id: i32,
    text: String,
}

impl DefaultInstance for EchoMessage {
    fn default_instance() -> &'static Self {
        static INST: std::sync::OnceLock<EchoMessage> = std::sync::OnceLock::new();
        INST.get_or_init(EchoMessage::default)
    }
}

impl Message for EchoMessage {
    fn compute_size(&self, _c: &mut SizeCache) -> u32 {
        0
    }
    fn write_to(&self, _c: &mut SizeCache, buf: &mut impl BufMut) {
        Tag::new(1, WireType::Varint).encode(buf);
        encode_varint(self.id as u64, buf);
        Tag::new(2, WireType::LengthDelimited).encode(buf);
        encode_varint(self.text.len() as u64, buf);
        buf.put_slice(self.text.as_bytes());
    }
    fn merge_field(
        &mut self,
        tag: Tag,
        buf: &mut impl Buf,
        _c: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        match tag.field_number() {
            1 => {
                self.id = decode_varint(buf)? as i32;
                Ok(())
            }
            2 => {
                let len = decode_varint(buf)? as usize;
                let mut b = vec![0u8; len];
                buf.copy_to_slice(&mut b);
                self.text = String::from_utf8(b).map_err(|_| DecodeError::InvalidUtf8)?;
                Ok(())
            }
            _ => skip_field(tag, buf),
        }
    }
    fn clear(&mut self) {
        *self = Self::default();
    }
}

// ── Server ───────────────────────────────────────────────────────────────────

/// Build the ConnectRPC serve handler: one unary `Echo` procedure that returns
/// the request message unchanged.
fn echo_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.unary(
        PROCEDURE,
        // The codec table for this procedure — `defaults()` installs the proto +
        // JSON codecs; the wire Content-Type selects which one is used.
        ProcedureCodecs::<EchoMessage, EchoMessage>::defaults(),
        // The handler: `(Ctx, Request<Req>) -> impl Future<Output = ConnectResult<Response<Res>>>`.
        |_ctx: Ctx, req: Request<EchoMessage>| async move { Ok(Response::new(req.msg)) },
        HandlerOptions::new().with_idempotency(IdempotencyLevel::NoSideEffects),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

/// Start the server on a free loopback port and return `(addr, shutdown)`.
fn start_server() -> (std::net::SocketAddr, Arc<OnSignal>) {
    let mut app = HttpApp::new_serve();
    let serve = echo_serve();
    app.router.add_route_any(PROCEDURE, &serve);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");

    let server =
        HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), ServerConfig::defaults());

    let shutdown = Arc::new(OnSignal::new());
    let shutdown_thread = shutdown.clone();
    // The blocking accept loop runs on its own OS thread.
    std::thread::spawn(move || server.serve_with_listener(&listener, &shutdown_thread));

    // Wait until the port accepts connections.
    while std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_err() {}

    (addr, shutdown)
}

// ── Entry point ──────────────────────────────────────────────────────────────

#[valtron(seed = 1, threads = 4)]
async fn main() {
    let (addr, shutdown) = start_server();

    // Build the client stack: an HTTP/1.1 transport + a typed per-procedure client.
    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(SimpleHttpClient::from_system()));
    let url = format!("http://127.0.0.1:{}{PROCEDURE}", addr.port());
    let client: Client<EchoMessage, EchoMessage> = Client::new(
        transport,
        &url,
        ProcedureCodecs::<EchoMessage, EchoMessage>::defaults(),
        ClientOptions::new(),
    )
    .expect("build client");

    // Per-call context (carries deadline + cancellation) and the request message.
    let ctx = Ctx::background().with_deadline(Duration::from_secs(10));
    let request = Request::new(EchoMessage { id: 7, text: "hello connectrpc".into() });

    let response: ConnectResult<Response<EchoMessage>> = client.unary(ctx, request).await;
    match response {
        Ok(resp) => {
            println!("unary echo OK: id={} text={:?}", resp.msg.id, resp.msg.text);
            assert_eq!(resp.msg, EchoMessage { id: 7, text: "hello connectrpc".into() });
            println!("round-trip verified ✓");
        }
        Err(err) => {
            eprintln!("unary echo FAILED: {err:?}");
            shutdown.turn_on();
            std::process::exit(1);
        }
    }

    shutdown.turn_on();
}
