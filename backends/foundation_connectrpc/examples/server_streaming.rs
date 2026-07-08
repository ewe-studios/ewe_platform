//! Server-streaming RPC over a real loopback socket.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p foundation_connectrpc --example server_streaming
//! ```
//!
//! One request in, a *stream* of responses out. The handler returns any
//! `futures::Stream<Item = ConnectResult<Res>>`; the client drains it with
//! `stream.receive().await` until it yields `None`. On the wire this is the
//! Connect streaming framing (5-byte enveloped frames + a terminating
//! EndStream frame); the library handles all of that for you.

use std::sync::Arc;
use std::time::Duration;

// buffa (and its `bytes` re-export) reached through foundation_connectrpc.
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
    Client, ClientOptions, ConnectResult, ConnectRpcServe, Ctx, H1Transport, HandlerOptions,
    ProcedureCodecs, Request, Router,
};

const PROCEDURE: &str = "/demo.CountService/Count";

// ── Messages (normally generated — see `unary_echo.rs` for the note) ─────────

#[derive(Clone, Default, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
struct CountRequest {
    /// How many items to stream back.
    count: i32,
}

#[derive(Clone, Default, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
struct CountItem {
    index: i32,
    label: String,
}

// Full `Message` impls for both types. NOTE: the default codec is **proto**
// (`ProcedureCodecs::defaults()` installs `(ProtoCodec, JsonCodec)` and the
// client defaults to `"proto"`), so these proto encodings are the ones on the
// wire here — a stub impl would silently drop fields. Real services get these
// from codegen; select JSON explicitly with `ClientOptions::new().with_codec("json")`.
impl DefaultInstance for CountRequest {
    fn default_instance() -> &'static Self {
        static INST: std::sync::OnceLock<CountRequest> = std::sync::OnceLock::new();
        INST.get_or_init(CountRequest::default)
    }
}
impl Message for CountRequest {
    fn compute_size(&self, _c: &mut SizeCache) -> u32 {
        0
    }
    fn write_to(&self, _c: &mut SizeCache, buf: &mut impl BufMut) {
        Tag::new(1, WireType::Varint).encode(buf);
        encode_varint(self.count as u64, buf);
    }
    fn merge_field(
        &mut self,
        tag: Tag,
        buf: &mut impl Buf,
        _c: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        match tag.field_number() {
            1 => {
                self.count = decode_varint(buf)? as i32;
                Ok(())
            }
            _ => skip_field(tag, buf),
        }
    }
    fn clear(&mut self) {
        *self = Self::default();
    }
}

impl DefaultInstance for CountItem {
    fn default_instance() -> &'static Self {
        static INST: std::sync::OnceLock<CountItem> = std::sync::OnceLock::new();
        INST.get_or_init(CountItem::default)
    }
}
impl Message for CountItem {
    fn compute_size(&self, _c: &mut SizeCache) -> u32 {
        0
    }
    fn write_to(&self, _c: &mut SizeCache, buf: &mut impl BufMut) {
        Tag::new(1, WireType::Varint).encode(buf);
        encode_varint(self.index as u64, buf);
        Tag::new(2, WireType::LengthDelimited).encode(buf);
        encode_varint(self.label.len() as u64, buf);
        buf.put_slice(self.label.as_bytes());
    }
    fn merge_field(
        &mut self,
        tag: Tag,
        buf: &mut impl Buf,
        _c: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        match tag.field_number() {
            1 => {
                self.index = decode_varint(buf)? as i32;
                Ok(())
            }
            2 => {
                let len = decode_varint(buf)? as usize;
                let mut b = vec![0u8; len];
                buf.copy_to_slice(&mut b);
                self.label = String::from_utf8(b).map_err(|_| DecodeError::InvalidUtf8)?;
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

fn count_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.server_stream(
        PROCEDURE,
        ProcedureCodecs::<CountRequest, CountItem>::defaults(),
        |_ctx: Ctx, req: Request<CountRequest>| async move {
            let n = req.msg.count.max(0);
            let items: Vec<ConnectResult<CountItem>> = (0..n)
                .map(|i| Ok(CountItem { index: i, label: format!("item-{i}") }))
                .collect();
            Ok(futures::stream::iter(items))
        },
        HandlerOptions::new(),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

fn start_server() -> (std::net::SocketAddr, Arc<OnSignal>) {
    let mut app = HttpApp::new_serve();
    let serve = count_serve();
    app.router.add_route_any(PROCEDURE, &serve);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    let server =
        HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), ServerConfig::defaults());

    let shutdown = Arc::new(OnSignal::new());
    let shutdown_thread = shutdown.clone();
    std::thread::spawn(move || server.serve_with_listener(&listener, &shutdown_thread));
    while std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_err() {}

    (addr, shutdown)
}

// ── Entry point ──────────────────────────────────────────────────────────────

#[valtron(seed = 1, threads = 4)]
async fn main() {
    let (addr, shutdown) = start_server();

    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(SimpleHttpClient::from_system()));
    let url = format!("http://127.0.0.1:{}{PROCEDURE}", addr.port());
    let client: Client<CountRequest, CountItem> = Client::new(
        transport,
        &url,
        ProcedureCodecs::<CountRequest, CountItem>::defaults(),
        ClientOptions::new(),
    )
    .expect("build client");

    let ctx = Ctx::background().with_deadline(Duration::from_secs(10));
    let mut stream = client
        .server_stream(ctx, Request::new(CountRequest { count: 3 }))
        .await
        .expect("open server stream");

    let mut received = Vec::new();
    while let Some(item) = stream.receive().await.expect("receive item") {
        println!("streamed: index={} label={:?}", item.index, item.label);
        received.push(item);
    }

    assert_eq!(received.len(), 3, "expected 3 streamed items");
    assert_eq!(received[2].label, "item-2");
    println!("server-stream round-trip verified ✓ ({} items)", received.len());

    shutdown.turn_on();
}
