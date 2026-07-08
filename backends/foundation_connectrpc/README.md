# foundation_connectrpc

ConnectRPC for the EWE platform — the [Connect protocol](https://connectrpc.com/docs/protocol)
(and gRPC-Web) implemented natively on the platform's foundation crates. An
independent Rust port with [connect-go](https://github.com/connectrpc/connect-go)
as the design reference — **not** a wrapper around any existing crate. There is no
tower, hyper, or tokio here: HTTP and wire types come from `foundation_netio` /
`foundation_http`, execution from valtron, errors from `foundation_errstacks`.

Speak **Connect**, **gRPC-Web**, and (as the HTTP/2 substrate lands) **gRPC** —
one router, one set of handlers, one client type — over JSON, protobuf (buffa),
or Arrow.

## Status

Built feature-by-feature (see `specifications/41-connectrpc`). What is proven
end-to-end today, over a real loopback socket:

| Area | State |
|---|---|
| Connect protocol (unary + streaming) | ✅ working |
| gRPC-Web protocol | ✅ working |
| HTTP/1.1 transport (client `H1Transport` + server `ConnectRpcServe`) | ✅ working |
| Codecs: protobuf (buffa), JSON, Arrow | ✅ working |
| Compression: gzip (+ optional zstd, brotli) | ✅ working |
| Auth middleware (`auth` feature) | ✅ working |
| Code generation: proto (build.rs / protoc) + code-first macro | ✅ unary proven; ⚠️ streaming code-first pending |
| gRPC protocol / HTTP/2, HTTP/3, WebSocket transports | ⏳ pending (needs the HTTP/2 substrate) |

The `examples/` directory holds runnable, self-contained programs for the first
three rows — each stands up a server and calls it.

## Quick start

```toml
[dependencies]
foundation_connectrpc = "0.0.1"   # default = ["rpc_multi"]
```

Run the bundled examples:

```sh
cargo run -p foundation_connectrpc --example unary_echo
cargo run -p foundation_connectrpc --example server_streaming
cargo run -p foundation_connectrpc --example code_first_service
```

## The four RPC kinds

Handlers are plain async closures/functions (Decision 04). `Ctx` is passed **by
value** (owned, cheap-`Clone`, Arc-backed) and every RPC-surface signature returns
`ConnectResult<T>` (`= Result<T, ErrorTrace<ConnectError>>`).

| Kind | Handler signature |
|---|---|
| **Unary** | `Fn(Ctx, Request<Req>) -> ConnectResult<Response<Res>>` |
| **Server-streaming** | `Fn(Ctx, Request<Req>) -> ConnectResult<impl Stream<Item = ConnectResult<Res>>>` |
| **Client-streaming** | `Fn(Ctx, RequestStream<Req>) -> ConnectResult<Response<Res>>` |
| **Bidi-streaming** | `Fn(Ctx, RequestStream<Req>) -> ConnectResult<impl Stream<Item = ConnectResult<Res>>>` |

(Full-duplex bidi needs a full-duplex transport — HTTP/2 or WebSocket, both
pending; unary, server-streaming, and half-duplex client-streaming work over
HTTP/1.1.)

## Server

Register procedures on a `Router`, freeze it into a `ConnectRpcServe`, and serve
it from a `foundation_http` server. See `examples/unary_echo.rs` for the full
runnable version.

```rust
use std::sync::Arc;
use foundation_connectrpc::{
    ConnectRpcServe, HandlerOptions, IdempotencyLevel, ProcedureCodecs,
    Request, Response, Router,
};

let mut router = Router::new();
router.unary(
    "/demo.EchoService/Echo",                          // procedure path (leading slash)
    ProcedureCodecs::<EchoMessage, EchoMessage>::defaults(), // proto + JSON codecs
    |_ctx, req: Request<EchoMessage>| async move { Ok(Response::new(req.msg)) },
    HandlerOptions::new().with_idempotency(IdempotencyLevel::NoSideEffects),
);
let serve = Arc::new(ConnectRpcServe::new(router.into_handler()));
// hand `serve` to a foundation_http HttpServer (see the example)
```

`Router` also has `.server_stream(...)`, `.client_stream(...)`, and
`.bidi_stream(...)`. `HandlerOptions` carries per-procedure interceptors,
size limits, idempotency, compression, and pipe depth.

## Client

```rust
use std::sync::Arc;
use std::time::Duration;
use foundation_connectrpc::{
    Client, ClientOptions, Ctx, H1Transport, ProcedureCodecs, Request,
};
use foundation_connectrpc::transport::Transport;
use foundation_netio::simple_http::client::SimpleHttpClient;

let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(SimpleHttpClient::from_system()));
let client: Client<EchoMessage, EchoMessage> = Client::new(
    transport,
    "http://127.0.0.1:8080/demo.EchoService/Echo",   // base URL + procedure path
    ProcedureCodecs::<EchoMessage, EchoMessage>::defaults(),
    ClientOptions::new(),                             // .with_connect() / .with_grpc_web() / .with_codec("json") / .with_timeout(..)
)?;

let ctx = Ctx::background().with_deadline(Duration::from_secs(10));
let resp = client.unary(ctx, Request::new(EchoMessage { /* .. */ })).await?;
```

`Client` also exposes `.server_stream()`, `.client_stream()`, and `.bidi_stream()`,
returning stream handles you drain with `.receive().await`.

## Codecs

The **per-procedure `ProcedureCodecs` table is the single codec authority**
(Decision 02) — codec names are the wire Content-Type tokens; there is no
request-time registry.

| Codec | Wire name | Type bound |
|---|---|---|
| `ProtoCodec` | `proto` | `buffa::Message` |
| `JsonCodec` | `json` | `serde::Serialize + DeserializeOwned` (**serde only — no protobuf**) |
| `ArrowCodec` (feature `arrow`) | `arrow` | `ToArrow + FromArrow` |

- `ProcedureCodecs::defaults()` = `(ProtoCodec, JsonCodec)` — the codegen default;
  requires `buffa::Message` because it includes proto. **`proto` is the default
  wire codec** unless the client selects otherwise (`ClientOptions::with_codec("json")`).
- `ProcedureCodecs::of((JsonCodec,))` / `::only(JsonCodec)` — a JSON-only route
  that needs **only serde**, no buffa, no protobuf.
- Proto message types get their `Message` impl from codegen (there is no derive).
  For hand-written proto types, the buffa surface is re-exported as
  `foundation_connectrpc::buffa` (incl. `buffa::bytes`) so you need no direct
  buffa dependency.

## Protocols

`ClientOptions` selects the protocol; the router serves all of them at once.

| Protocol | Selector | State |
|---|---|---|
| Connect | `ClientOptions::new().with_connect()` (default) | ✅ |
| gRPC-Web | `.with_grpc_web()` | ✅ |
| gRPC | `.with_grpc()` | ⏳ requires HTTP/2 (pending) |

## Code generation

Three modes (Decision 10). Proto codegen lives in the **build-time companion
crate** [`foundation_connectrpc_codegen`](../foundation_connectrpc_codegen) (the
`tonic` / `tonic-build` split) — add it under `[build-dependencies]`. The
code-first macros ship in this crate.

**Mode 1 — `build.rs`:**

```rust
// build.rs
fn main() {
    foundation_connectrpc_codegen::build::Config::new()
        .files(&["proto/greet.proto"])
        .includes(&["proto/"])
        .compile()
        .unwrap();
}
```

**Mode 2 — protoc plugin** (binary `rpc-gen-proto`; mapped explicitly since it is
not a `protoc-gen-*` name):

```sh
protoc --connect-ewe_out=. \
       --plugin=protoc-gen-connect-ewe="$(command -v rpc-gen-proto)" \
       service.proto
```

**Mode 3 — code-first**, no `.proto` at all: annotate a Rust trait with
`#[connectrpc::service]`. It generates the procedure constants, the service trait
(with default `unimplemented` bodies), a `register_<svc>` function, and a typed
client. A `codecs(json)` service needs only serde. See
`examples/code_first_service.rs`.

```rust
use foundation_connectrpc as connectrpc;

#[connectrpc::service(package = "demo.greet.v1", codecs(json))]
pub trait GreetService {
    async fn greet(&self, _ctx: connectrpc::Ctx, _req: connectrpc::Request<GreetRequest>)
        -> connectrpc::ConnectResult<connectrpc::Response<GreetResponse>>;
}
```

## Authentication (feature `auth`)

Port of authn-go's middleware onto `foundation_auth`. An async `AuthFunc` produces
a concrete `AuthInfo`; `AuthzInterceptor`, `JwtAuthenticator`,
`CompositeAuthenticator`, and `PerProcedureAuth` compose over it. Requires the
`auth` feature (on by default via `rpc`).

## Feature flags

| Feature | Effect |
|---|---|
| `rpc` | Full single-threaded library surface: all codecs + auth + all compressors |
| `rpc_multi` | `rpc` + the multi-threaded valtron executor (**default**) |
| `arrow` | Arrow columnar codec (`foundation_arrow`) |
| `auth` | Auth middleware (`foundation_auth`) |
| `zstd`, `brotli` | Extra compressors (gzip is always available) |

For a lean build, `default-features = false` plus `features = ["rpc"]` drops the
multi executor; dropping `rpc` in favour of individual flags drops
`foundation_arrow` / `foundation_auth`.

## Error model

Every RPC-surface signature returns `ConnectResult<T> = Result<T, ErrorTrace<ConnectError>>`
(Decision 03). `ConnectError` carries a Connect `Code`; domain layers below the
surface keep typed errors (`CodecError`, `CompressionError`, `TransportError`)
mapped in at the boundary. The full `errstacks` trace is preserved for logging and
telemetry.

## Built on

`foundation_core` (valtron execution + async readiness), `foundation_http` /
`foundation_netio` (HTTP server/client + wire types), `foundation_errstacks`
(error traces), `buffa` (protobuf), optional `foundation_arrow` (columnar) and
`foundation_auth` (authn).

## License

Apache-2.0
