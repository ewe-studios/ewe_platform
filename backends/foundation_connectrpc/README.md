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
three rows — each stands up a server and calls it over a real TCP socket.

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

Full-duplex bidi needs a full-duplex transport — HTTP/2 or WebSocket, both
pending; unary, server-streaming, and half-duplex client-streaming work over
HTTP/1.1.

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

| Codec | Wire name | Type bounds |
|---|---|---|
| `ProtoCodec` | `proto` | `buffa::Message + Serialize + DeserializeOwned` |
| `JsonCodec` | `json` | `Serialize + DeserializeOwned` (**serde only — no protobuf**) |
| `ArrowCodec` (feature `arrow`) | `arrow` | `ToArrow + FromArrow` |

### Constructing codec tables

| Expression | Codecs installed | When to use |
|---|---|---|
| `ProcedureCodecs::defaults()` | `(ProtoCodec, JsonCodec)` | Proto-generated or hand-written `buffa::Message` types. **`proto` is the default wire codec.** |
| `ProcedureCodecs::of((JsonCodec,))` | `(JsonCodec,)` | JSON-only — **only serde needed**, no buffa, no protobuf. |
| `ProcedureCodecs::only(JsonCodec)` | `(JsonCodec,)` | Same as above, alternate spelling. |
| `ProcedureCodecs::of((JsonCodec, ArrowCodec))` | `(JsonCodec, ArrowCodec)` | JSON + Arrow columnar. |

Proto message types get their `Message` impl from codegen (there is no derive).
For hand-written proto types, the buffa surface is re-exported as
`foundation_connectrpc::buffa` (incl. `buffa::bytes`) so you need no direct
buffa dependency.

### Client codec selection

The client's default wire codec is `proto`. To use a different codec (or when
the server only registers a JSON-only table), specify it explicitly:

```rust
ClientOptions::new().with_codec("json")
```

## Protocols

`ClientOptions` selects the protocol; the router serves all of them at once.

| Protocol | Selector | State |
|---|---|---|
| Connect | `ClientOptions::new().with_connect()` (default) | ✅ |
| gRPC-Web | `.with_grpc_web()` | ✅ |
| gRPC | `.with_grpc()` | ⏳ requires HTTP/2 (pending) |

---

## Code generation

There are three ways to define a ConnectRPC service. All three produce the
**same runtime surface** — procedure path constants, a service trait, a
registration function, an unimplemented handler, and a typed client — so
proto-first and code-first users converge on identical server and client code.

| Mode | Input | Mechanism | Output |
|---|---|---|---|
| **1 — build.rs** | `.proto` files | `build.rs` helper (prost-build + codegen) | `.rs` file in `OUT_DIR` |
| **2 — protoc plugin** | `.proto` files | `protoc` with `--plugin=protoc-gen-connect-ewe` | `.rs` file on disk |
| **3 — code-first** | Rust trait | `#[connectrpc::service]` proc macro | In-file expansion (no separate file) |

The proto codegen (Modes 1–2) lives in the **build-time companion crate**
[`foundation_connectrpc_codegen`](../foundation_connectrpc_codegen) — the same
split as `tonic` / `tonic-build`. Add it under `[build-dependencies]`. The
code-first macro (Mode 3) ships in this crate.

---

### Mode 1 — `build.rs`

Add `foundation_connectrpc_codegen` as a build dependency **and**
`foundation_connectrpc` as a regular dependency, then write a `build.rs` that
compiles your `.proto` files and generates both message types (via prost-build)
and service stubs in one pass:

```toml
# Cargo.toml
[dependencies]
foundation_connectrpc = "0.0.1"

[build-dependencies]
foundation_connectrpc_codegen = "0.0.1"
```

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

**What this produces:**

1. **Message types** — prost-build compiles your `.proto` messages into Rust
   structs with `buffa::Message` impls. The output filename follows the
   **protobuf package name**: a proto with `package connectrpc.greet.v1;`
   produces `connectrpc.greet.v1.rs`.
2. **Service stubs** — the codegen writes `_connectrpc.rs` (always this exact
   name), containing the service trait, registration function, typed client,
   and procedure constants (see [Generated artifacts](#generated-artifacts)
   below for the full list).

Both files are written to `OUT_DIR` — a directory **set automatically by
Cargo** during compilation (typically
`target/debug/build/<your-crate>-<hash>/out/`). You never configure it; Cargo
sets the `OUT_DIR` environment variable when running `build.rs`, and
`env!("OUT_DIR")` reads it back during compilation of your crate.

**Include the generated code** in your crate:

```rust
// src/lib.rs (or any module)
//
// Service stubs — always named _connectrpc.rs.
include!(concat!(env!("OUT_DIR"), "/_connectrpc.rs"));
// Message types — filename matches the proto package (dots preserved).
// For `package connectrpc.greet.v1;` in greet.proto, the filename is:
include!(concat!(env!("OUT_DIR"), "/connectrpc.greet.v1.rs"));
```

> **Note:** The generated code references runtime types as `connectrpc::Ctx`,
> `connectrpc::Router`, etc. Your crate must have `foundation_connectrpc` as a
> regular dependency and alias it:
> ```rust
> use foundation_connectrpc as connectrpc;
> ```

> **How to find the prost-build filename:** If you're unsure what prost-build
> named its output, run `cargo build` once then list the directory:
> ```sh
> ls target/debug/build/<your-crate>-*/out/
> ```
> You'll see your package-named `.rs` file(s) alongside `_connectrpc.rs`.

---

### Mode 2 — protoc plugin

The binary `protoc-gen-connect-ewe` (from `foundation_connectrpc_codegen`) follows
the standard `protoc-gen-*` naming convention, so **protoc discovers it
automatically from PATH** — no `--plugin=` mapping needed:

```sh
protoc \
    --connect-ewe_out=. \
    --proto_path=proto \
    proto/greet.proto
```

**How this works:**

1. `protoc` parses your `.proto` files and resolves imports using `--proto_path`.
2. For `--connect-ewe_out`, protoc looks for a binary named `protoc-gen-connect-ewe`
   on `PATH` and spawns it, piping the parsed file descriptors to its stdin.
3. The plugin generates the service stubs and writes them back as a
   `CodeGeneratorResponse` on stdout.
4. `protoc` writes the generated file to the output directory (`.`) — by
   default named `_connectrpc.rs`.

**You also need a separate step for message types.** This plugin only generates
*service* code. Use `--prost_out` (from prost-build's protoc plugin) or a manual
`prost-build` step for the message structs:

```sh
# Generate message types (prost) + service stubs (connect-ewe) in one command:
protoc \
    --prost_out=. \
    --connect-ewe_out=. \
    --proto_path=proto \
    proto/greet.proto
```

The generated `_connectrpc.rs` is identical to what Mode 1 produces — a
standalone Rust source file you `include!` or add to your crate.

---

### Mode 3 — code-first (proc macro)

No `.proto` file at all. Define your service as a plain Rust trait and annotate
it with `#[connectrpc::service]`. The macro expands **in the same file** — it
does not write a separate `.rs` file. Everything is generated at compile time as
part of normal proc-macro expansion.

```rust
use foundation_connectrpc as connectrpc;

#[connectrpc::service(package = "demo.greet.v1", codecs(json))]
pub trait GreetService {
    async fn greet(
        &self,
        _ctx: connectrpc::Ctx,
        _req: connectrpc::Request<GreetRequest>,
    ) -> connectrpc::ConnectResult<connectrpc::Response<GreetResponse>>;
}
```

#### Attribute arguments

| Argument | Required? | Default | Description |
|---|---|---|---|
| `package = "..."` | **Yes** | — | Protobuf-style package name (e.g. `"acme.greet.v1"`). |
| `codecs(...)` | No | `json` | Comma-separated codec families: `json`, `proto`, `arrow`. |

#### What the macro generates

The annotation on `GreetService` expands into **all of the following items**
in the same module where the trait is defined:

| # | Generated item | Example (for `GreetService`) | Purpose |
|---|---|---|---|
| R1 | **Procedure path constants** | `pub mod procedure { pub const GREET: &str = "/demo.greet.v1.GreetService/Greet"; }` | Paths you pass to `Router` registration and `HttpApp` routing. |
| R2 | **Service name constant** | `pub const GREETSERVICE_NAME: &str = "demo.greet.v1.GreetService";` | Fully-qualified service name. |
| R3 | **The service trait** | `pub trait GreetService: Send + Sync + 'static { ... }` | Your trait, with `Send + Sync + 'static` bounds added, default `unimplemented` bodies, and `async fn` desugared to `-> impl Future<Output = ...> + Send`. |
| R4 | **Registration function** | `pub fn register_greet_service(router: &mut Router, service: Arc<S>)` where `S: GreetService` | Registers every RPC method on a `Router` in one call. |
| R5 | **Unimplemented handler** | `pub struct UnimplementedGreetServiceHandler;` + `impl GreetService for UnimplementedGreetServiceHandler {}` | A stub that returns `ConnectError::unimplemented` for every method — useful as a placeholder during development. |
| R6 | **Typed client struct** | `pub struct GreetServiceClient { greet: Client<GreetReq, GreetResp>, ... }` | A client with one async method per RPC. Its `new(transport, base_url, options)` constructor builds one `Client<Req, Res>` per method. |
| R7 | **Client trait** | `pub trait GreetServiceClientExt: Send + Sync + 'static { ... }` | A trait mirroring the client methods — enables mocking and dependency injection in tests. |
| R8 | **Descriptor macro** | `#[macro_export] macro_rules! greet_service_tokens { () => { ... } }` | Captures the full expansion for cross-crate re-generation via `generate!`. |

#### Implementing the service — same crate

The generated trait, registration function, and client all appear in the same
module. Implement the trait on your own type and call the registration function:

```rust
// ── In the same module where #[service] was applied ──

struct Greeter;

impl GreetService for Greeter {
    async fn greet(
        &self,
        _ctx: Ctx,
        req: Request<GreetRequest>,
    ) -> ConnectResult<Response<GreetResponse>> {
        Ok(Response::new(GreetResponse {
            message: format!("Hello, {}!", req.msg.name),
        }))
    }
}

// Wire it up:
let mut router = Router::new();
register_greet_service(&mut router, Arc::new(Greeter));
let serve = Arc::new(ConnectRpcServe::new(router.into_handler()));

// Use the typed client:
let client = GreetServiceClient::new(
    transport,
    &base_url,
    ClientOptions::new().with_codec("json"),
)?;
let resp = client.greet(ctx, Request::new(GreetRequest { name: "world".into() })).await?;
```

See `examples/code_first_service.rs` for the full runnable version.

#### Implementing the service — another crate (cross-crate)

When the trait is defined in one crate (e.g. an API crate) and implemented in
another (e.g. a server crate), use the `generate!` macro:

**Crate A — the API crate** (depends on `foundation_connectrpc`):

```rust
// Crate A: src/lib.rs
use foundation_connectrpc as connectrpc;

#[connectrpc::service(package = "my.api.v1", codecs(json))]
pub trait MyApi {
    async fn do_thing(
        &self,
        ctx: connectrpc::Ctx,
        req: connectrpc::Request<MyRequest>,
    ) -> connectrpc::ConnectResult<connectrpc::Response<MyResponse>>;
}
```

The `#[service]` annotation exports a descriptor macro named
`my_api_tokens` (derived from the snake_case trait name). Crate B invokes it
with `generate!`:

**Crate B — the server crate** (depends on crate A + `foundation_connectrpc`):

```rust
// Crate B: src/server.rs
use foundation_connectrpc as connectrpc;

// Re-expand the service artifacts inside a new module:
connectrpc::generate!(my_api::my_api_tokens => mod my_svc {
    server, client
});

// Now the generated items live in `my_svc`:
use my_svc::{GreetService, GreetServiceClient, register_greet_service};

struct MyServer;
impl MyApi for MyServer { /* ... */ }

let mut router = Router::new();
register_greet_service(&mut router, Arc::new(MyServer));
```

> **How this works internally:** `#[service]` wraps its entire expansion in a
> `#[macro_export] macro_rules! <name>_tokens { () => { ... } }`. The
> `generate!` macro invokes that descriptor macro inside a new `mod { ... }`
> block. No files are written — it's all proc-macro expansion at compile time.

#### Streaming methods in code-first

All four RPC kinds are supported. The macro classifies each method by inspecting
its parameter and return types syntactically:

```rust
#[connectrpc::service(package = "demo.v1", codecs(json))]
pub trait StreamingService {
    // Unary: Request<T> → ConnectResult<Response<T>>
    async fn unary(&self, ctx: Ctx, req: Request<Req>) -> ConnectResult<Response<Res>>;

    // Server-streaming: Request<T> → ConnectResult<impl Stream<Item = ConnectResult<T>>>
    async fn server_stream(&self, ctx: Ctx, req: Request<Req>)
        -> ConnectResult<impl Stream<Item = ConnectResult<Res>>>;

    // Client-streaming: impl Stream<Item = ConnectResult<T>> → ConnectResult<Response<T>>
    async fn client_stream(&self, ctx: Ctx, reqs: impl Stream<Item = ConnectResult<Req>>)
        -> ConnectResult<Response<Res>>;

    // Bidi-streaming: impl Stream<Item = ...> → ConnectResult<impl Stream<Item = ...>>
    async fn bidi(&self, ctx: Ctx, reqs: impl Stream<Item = ConnectResult<Req>>)
        -> ConnectResult<impl Stream<Item = ConnectResult<Res>>>;
}
```

> **⚠️ Streaming limitation:** The code-first macro currently box-stream-rewrites
> server/bidi-streaming signatures to `Pin<Box<dyn Stream + Send>>` return types
> (to satisfy `Send` bounds on Router's handler futures). This rewrite has a
> known issue (E0562) that is being resolved — streaming code-first is marked
> **pending** in the status table. Unary code-first is fully proven.

#### Codec selection in code-first

The `codecs(...)` argument controls which `ProcedureCodecs` expression is
generated in the registration function and client constructor:

| `codecs(...)` | Generated `ProcedureCodecs` expression | Message type requirements |
|---|---|---|
| `codecs(json)` (default) | `ProcedureCodecs::of((JsonCodec,))` | `Serialize + DeserializeOwned` |
| `codecs(proto)` | `ProcedureCodecs::defaults()` | `buffa::Message + Serialize + DeserializeOwned` |
| `codecs(arrow)` | `ProcedureCodecs::only(ArrowCodec)` | `ToArrow + FromArrow` |
| `codecs(json, arrow)` | `ProcedureCodecs::of((JsonCodec, ArrowCodec))` | serde + arrow bounds |

---

### Generated artifacts — unified reference

All three modes generate the same set of artifacts. Here they are for a service
named `GreetService` in package `demo.greet.v1` with one RPC method `Greet`:

```
// ── R1: Procedure path constants ──────────────────────────────────────────
pub mod procedure {
    /// Procedure path — always includes a leading slash.
    pub const GREET: &str = "/demo.greet.v1.GreetService/Greet";
}

// ── R2: Service name constant ─────────────────────────────────────────────
pub const GREETSERVICE_NAME: &str = "demo.greet.v1.GreetService";

// ── R3: Service trait ─────────────────────────────────────────────────────
pub trait GreetService: Send + Sync + 'static {
    async fn greet(
        &self,
        ctx: connectrpc::Ctx,
        req: connectrpc::Request<GreetRequest>,
    ) -> connectrpc::ConnectResult<connectrpc::Response<GreetResponse>>;
}

// ── R4: Registration function ─────────────────────────────────────────────
pub fn register_greet_service<S: GreetService>(
    router: &mut connectrpc::Router,
    service: std::sync::Arc<S>,
) {
    // Calls router.unary(procedure::GREET, codecs, handler, opts) for each method
}

// ── R5: Unimplemented handler ─────────────────────────────────────────────
pub struct UnimplementedGreetServiceHandler;
impl GreetService for UnimplementedGreetServiceHandler {}

// ── R6: Typed client ──────────────────────────────────────────────────────
pub struct GreetServiceClient {
    greet: connectrpc::Client<GreetRequest, GreetResponse>,
}
impl GreetServiceClient {
    pub fn new(
        transport: Arc<dyn connectrpc::Transport>,
        base_url: &str,
        options: connectrpc::ClientOptions,
    ) -> connectrpc::ConnectResult<Self>;

    pub async fn greet(
        &self,
        ctx: connectrpc::Ctx,
        request: connectrpc::Request<GreetRequest>,
    ) -> connectrpc::ConnectResult<connectrpc::Response<GreetResponse>>;
}

// ── R7: Client trait ──────────────────────────────────────────────────────
pub trait GreetServiceClientExt: Send + Sync + 'static {
    async fn greet(
        &self,
        ctx: connectrpc::Ctx,
        request: connectrpc::Request<GreetRequest>,
    ) -> connectrpc::ConnectResult<connectrpc::Response<GreetResponse>>;
}
impl GreetServiceClientExt for GreetServiceClient { /* delegates to inner Client */ }

// ── R8: Descriptor macro (Mode 3 only) ────────────────────────────────────
#[macro_export]
macro_rules! greet_service_tokens { () => { /* full expansion */ }; }
```

> **Important:** The generated code uses `connectrpc::` as a path prefix
> throughout (e.g. `connectrpc::Ctx`, `connectrpc::Router`). Your crate must
> bring the runtime crate into scope under that exact name:
> ```rust
> use foundation_connectrpc as connectrpc;
> ```

---

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
