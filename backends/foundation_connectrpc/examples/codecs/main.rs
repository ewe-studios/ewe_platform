//! Every codec combination, over one server (requires the `arrow` feature — on
//! by default via `rpc`).
//!
//! Run with:
//!
//! ```sh
//! cargo run -p foundation_connectrpc --example codecs
//! ```
//!
//! The per-procedure `ProcedureCodecs` table is the single codec authority
//! (Decision 02) — codec names are the wire Content-Type tokens. One echo handler
//! is registered five times, each under a different table:
//!
//! | Procedure     | Codec table                                  | Type bound reached |
//! |---------------|----------------------------------------------|--------------------|
//! | proto + json  | `ProcedureCodecs::defaults()`                | Message + serde    |
//! | json          | `of((JsonCodec,))`                           | serde only         |
//! | arrow         | `only(ArrowCodec)`                           | ToArrow + FromArrow|
//! | arrow + json  | `of((JsonCodec, ArrowCodec))`                | serde + Arrow      |
//! | proto+json+arrow | `of((ProtoCodec, JsonCodec, ArrowCodec))` | all three          |
//!
//! `Sample` below is a **tri-family** message — a buffa `Message` (+ serde) that
//! is also `ToArrow + FromArrow` — so a single type serves every table. The client
//! selects the wire codec per call with `ClientOptions::with_codec(..)`.

use std::sync::Arc;
use std::time::Duration;

use foundation_connectrpc::buffa::bytes::{Buf, BufMut};
use foundation_connectrpc::buffa::encoding::{decode_varint, encode_varint, skip_field, Tag, WireType};
use foundation_connectrpc::buffa::{DecodeContext, DecodeError, DefaultInstance, Message, SizeCache};

use foundation_arrow::arrow_array::{Array, Int32Array, RecordBatch};
use foundation_arrow::arrow_schema::{DataType, Field, Schema};
use foundation_arrow::{ArrowSchema, FromArrow, IpcResult, ToArrow};

use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;
use foundation_netio::http::SimpleHttpClient;

use foundation_connectrpc::transport::Transport;
use foundation_connectrpc::{
    ArrowCodec, Client, ClientOptions, ConnectResult, ConnectRpcServe, Ctx, H1Transport,
    HandlerOptions, JsonCodec, ProcedureCodecs, ProtoCodec, Request, Response, Router,
};

// ── Procedure paths, one per codec combination ──────────────────────────────

const P_PROTO_JSON: &str = "/codecs.EchoService/ProtoJson";
const P_JSON: &str = "/codecs.EchoService/Json";
const P_ARROW: &str = "/codecs.EchoService/Arrow";
const P_ARROW_JSON: &str = "/codecs.EchoService/ArrowJson";
const P_ALL: &str = "/codecs.EchoService/All";

const ALL_PROCEDURES: [&str; 5] = [P_PROTO_JSON, P_JSON, P_ARROW, P_ARROW_JSON, P_ALL];

// ── The tri-family message ──────────────────────────────────────────────────

#[derive(Clone, Default, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
struct Sample {
    value: i32,
}

// buffa Message (for the proto codec).
impl DefaultInstance for Sample {
    fn default_instance() -> &'static Self {
        static I: std::sync::OnceLock<Sample> = std::sync::OnceLock::new();
        I.get_or_init(Sample::default)
    }
}
impl Message for Sample {
    fn compute_size(&self, _c: &mut SizeCache) -> u32 {
        0
    }
    fn write_to(&self, _c: &mut SizeCache, buf: &mut impl BufMut) {
        Tag::new(1, WireType::Varint).encode(buf);
        encode_varint(self.value as u64, buf);
    }
    fn merge_field(&mut self, tag: Tag, buf: &mut impl Buf, _c: DecodeContext<'_>) -> Result<(), DecodeError> {
        if tag.field_number() == 1 {
            self.value = decode_varint(buf)? as i32;
            Ok(())
        } else {
            skip_field(tag, buf)
        }
    }
    fn clear(&mut self) {
        *self = Self::default();
    }
}

// Arrow (for the arrow codec): a single `value: int32` column.
impl ArrowSchema for Sample {
    fn schema() -> Schema {
        Schema::new(Self::fields())
    }
    fn fields() -> Vec<Field> {
        vec![Field::new("value", DataType::Int32, false)]
    }
}
impl ToArrow for Sample {
    fn to_arrow(&self) -> IpcResult<RecordBatch> {
        RecordBatch::try_new(Self::schema_ref(), vec![Arc::new(Int32Array::from(vec![self.value]))])
    }
    fn to_arrow_batch(values: &[Self]) -> IpcResult<RecordBatch> {
        let col = Int32Array::from(values.iter().map(|s| s.value).collect::<Vec<_>>());
        RecordBatch::try_new(Self::schema_ref(), vec![Arc::new(col)])
    }
}
impl FromArrow for Sample {
    fn from_arrow(batch: &RecordBatch) -> IpcResult<Self> {
        let col = batch.column(0).as_any().downcast_ref::<Int32Array>().expect("int32 column");
        Ok(Self { value: col.value(0) })
    }
    fn from_arrow_batch(batch: &RecordBatch) -> IpcResult<Vec<Self>> {
        let col = batch.column(0).as_any().downcast_ref::<Int32Array>().expect("int32 column");
        Ok((0..col.len()).map(|i| Self { value: col.value(i) }).collect())
    }
}

// ── Server: one echo handler, five codec tables ─────────────────────────────

async fn echo(_ctx: Ctx, req: Request<Sample>) -> ConnectResult<Response<Sample>> {
    Ok(Response::new(req.msg))
}

fn echo_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    let opts = || HandlerOptions::new();

    // proto + json (the codegen default)
    router.unary(P_PROTO_JSON, ProcedureCodecs::<Sample, Sample>::defaults(), echo, opts());
    // json only (serde, no protobuf)
    router.unary(P_JSON, ProcedureCodecs::<Sample, Sample>::of((JsonCodec,)), echo, opts());
    // arrow only (columnar)
    router.unary(P_ARROW, ProcedureCodecs::<Sample, Sample>::only(ArrowCodec), echo, opts());
    // arrow + json
    router.unary(P_ARROW_JSON, ProcedureCodecs::<Sample, Sample>::of((JsonCodec, ArrowCodec)), echo, opts());
    // proto + json + arrow (tri-family)
    router.unary(P_ALL, ProcedureCodecs::<Sample, Sample>::of((ProtoCodec, JsonCodec, ArrowCodec)), echo, opts());

    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

fn start_server() -> (std::net::SocketAddr, Arc<OnSignal>) {
    let mut app = HttpApp::new_serve();
    let serve = echo_serve();
    for proc in ALL_PROCEDURES {
        app.router.add_route_any(proc, &serve);
    }

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

// ── Client: call each procedure with the matching wire codec ────────────────

/// Build a per-procedure client with `codecs`, call `unary` selecting `wire`, and
/// assert the echo round-trips.
async fn round_trip(
    transport: Arc<dyn Transport>,
    url: String,
    codecs: ProcedureCodecs<Sample, Sample>,
    wire: &str,
    value: i32,
) {
    let client = Client::new(transport, &url, codecs, ClientOptions::new().with_codec(wire))
        .expect("build client");
    let ctx = Ctx::background().with_deadline(Duration::from_secs(10));
    let out = client
        .unary(ctx, Request::new(Sample { value }))
        .await
        .expect("unary call");
    assert_eq!(out.msg.value, value, "echo mismatch on {wire}");
    println!("  {wire:<5} → {url}  (value={})", out.msg.value);
}

#[valtron(seed = 1, threads = 4)]
async fn main() {
    let (addr, shutdown) = start_server();
    let base = format!("http://127.0.0.1:{}", addr.port());
    let t = || -> Arc<dyn Transport> { Arc::new(H1Transport::new(Arc::new(SimpleHttpClient::from_system()))) };

    println!("codec combinations, each verified over a real socket:");

    // proto+json → exercise proto on the wire
    round_trip(t(), format!("{base}{P_PROTO_JSON}"), ProcedureCodecs::defaults(), "proto", 1).await;
    // json → serde only
    round_trip(t(), format!("{base}{P_JSON}"), ProcedureCodecs::of((JsonCodec,)), "json", 2).await;
    // arrow → columnar
    round_trip(t(), format!("{base}{P_ARROW}"), ProcedureCodecs::only(ArrowCodec), "arrow", 3).await;
    // arrow+json → exercise arrow (json also available)
    round_trip(t(), format!("{base}{P_ARROW_JSON}"), ProcedureCodecs::of((JsonCodec, ArrowCodec)), "arrow", 4).await;
    // proto+json+arrow → exercise all three by codec name
    round_trip(t(), format!("{base}{P_ALL}"), ProcedureCodecs::of((ProtoCodec, JsonCodec, ArrowCodec)), "proto", 5).await;
    round_trip(t(), format!("{base}{P_ALL}"), ProcedureCodecs::of((ProtoCodec, JsonCodec, ArrowCodec)), "json", 6).await;
    round_trip(t(), format!("{base}{P_ALL}"), ProcedureCodecs::of((ProtoCodec, JsonCodec, ArrowCodec)), "arrow", 7).await;

    println!("all codec combinations verified ✓");
    shutdown.turn_on();
}
