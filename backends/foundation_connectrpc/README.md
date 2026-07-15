# foundation_connectrpc

ConnectRPC for the EWE platform — the [Connect protocol](https://connectrpc.com/docs/protocol),
gRPC, and gRPC-Web implemented natively on the platform's foundation crates. An
independent Rust port with [connect-go](https://github.com/connectrpc/connect-go)
as the design reference — **not** a wrapper around any existing crate. There is no
tower, hyper, or tokio here: HTTP and wire types come from `foundation_netio` /
`foundation_http`, execution from valtron, errors from `foundation_errstacks`.

Speak **Connect**, **gRPC**, or **gRPC-Web** — one router, one set of handlers,
one client type — over JSON, protobuf (buffa), or Arrow.

## Status

Built feature-by-feature (see `specifications/41-connectrpc`). What is proven
end-to-end today, over a real loopback socket:

| Area | State |
|---|---|
| Connect protocol (unary + streaming) | ✅ working |
| gRPC-Web protocol | ✅ working |
| gRPC protocol over HTTP/2 (h2c) — unary + all streaming | ✅ working |
| HTTP/1.1 transport (client `H1Transport` + server `ConnectRpcServe`) | ✅ working |
| HTTP/2 transport (client `H2Transport` + server `ConnectRpcServeH2`) | ✅ working |
| Codecs: protobuf (buffa), JSON, Arrow | ✅ working |
| Compression: gzip (+ optional zstd, brotli) | ✅ working |
| Auth middleware (`auth` feature) | ✅ working |
| Code generation: proto (build.rs / protoc) + code-first macro | ✅ working |
| gRPC status trailers (client-visible) | ⏳ trailers consumed by h2 pump, not forwarded |
| HTTP/3, WebSocket transports | ⏳ pending |

The [`examples/`](examples/) directory holds runnable, self-contained programs,
each standing up a server and calling it over a real TCP socket:

| Example | Protocol | Transport | Modes |
|---|---|---|---|
| [`unary_echo`](examples/unary_echo/) | Connect | H1Transport (HTTP/1.1) | unary |
| [`server_streaming`](examples/server_streaming/) | Connect | H1Transport (HTTP/1.1) | server-stream |
| [`h2_echo`](examples/h2_echo/) | Connect | H2Transport (h2c) | unary, server-stream, client-stream, bidi |
| [`grpc_echo`](examples/grpc_echo/) | gRPC | H2Transport (h2c) | unary, server-stream, client-stream, bidi |
| [`codecs`](examples/codecs/) | Connect | H1Transport | unary (multi-codec) |
| [`code_first_service`](examples/code_first_service/) | Connect (code-first) | H1Transport | unary |
| [`code_first_streaming`](examples/code_first_streaming/) | Connect (code-first) | H1Transport | server-stream |

## Quick start

```toml
[dependencies]
foundation_connectrpc = "0.0.1"   # default = ["rpc_multi"]
```

Run the bundled examples:

```sh
# Connect protocol over HTTP/1.1
cargo run -p foundation_connectrpc --example unary_echo
cargo run -p foundation_connectrpc --example server_streaming

# Connect + gRPC over HTTP/2 cleartext (all four RPC modes)
cargo run -p foundation_connectrpc --example h2_echo
cargo run -p foundation_connectrpc --example grpc_echo

# Code-first services (proc macro)
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

Full-duplex bidi needs a full-duplex transport — `H2Transport` (HTTP/2 cleartext
or TLS) enables it; unary, server-streaming, and client-streaming work over both
HTTP/1.1 and HTTP/2.

## Server

Register procedures on a `Router`, freeze it into a `ConnectRpcServe`, and serve
it from a `foundation_http` server. See `examples/unary_echo/` for the full
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

For HTTP/2, use `ConnectRpcServeH2` with `HttpApp::new_h2_serve()` instead:

```rust
use foundation_connectrpc::ConnectRpcServeH2;
use foundation_http::shared::serve::h2::H2Serve;

let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(router.into_handler()));
let mut app = HttpApp::new_h2_serve();
app.route_any_h2("/my.Svc/Method", rpc);
```

## Client

```rust
use std::sync::Arc;
use std::time::Duration;
use foundation_connectrpc::{
    Client, ClientOptions, Ctx, H1Transport, ProcedureCodecs, Request,
};
use foundation_connectrpc::transport::Transport;
use foundation_netio::simple_http::client::SimpleHttpClient;

// HTTP/1.1 transport (Connect + gRPC-Web)
let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(SimpleHttpClient::from_system()));

// HTTP/2 cleartext transport (Connect + gRPC, all four modes including bidi)
// let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());

let client: Client<EchoMessage, EchoMessage> = Client::new(
    transport,
    "http://127.0.0.1:8080/demo.EchoService/Echo",   // base URL + procedure path
    ProcedureCodecs::<EchoMessage, EchoMessage>::defaults(),
    ClientOptions::new(),                             // .with_connect() / .with_grpc() / .with_grpc_web() / .with_codec("json") / .with_timeout(..)
)?;

let ctx = Ctx::background().with_deadline(Duration::from_secs(10));
let resp = client.unary(ctx, Request::new(EchoMessage { /* .. */ })).await?;
```

`Client` also exposes `.server_stream()`, `.client_stream()`, and `.bidi_stream()`,
returning stream handles you drain with `.receive().await`.

### Protocol selection

The only difference between Connect and gRPC from user code is
`ClientOptions`. The server-side `Router` dispatches both automatically:

```rust
// Connect (the default):
ClientOptions::new().with_connect()

// gRPC over HTTP/2:
ClientOptions::new().with_grpc().with_codec("json")

// gRPC-Web over HTTP/1.1:
ClientOptions::new().with_grpc_web()
```

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

| Protocol | Selector | Transport | State |
|---|---|---|---|---|
| Connect | `ClientOptions::new().with_connect()` (default) | H1Transport or H2Transport | ✅ |
| gRPC-Web | `.with_grpc_web()` | H1Transport | ✅ |
| gRPC | `.with_grpc()` | H2Transport (HTTP/2 required) | ✅ unary + streaming |

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
| **3 — code-first** | Rust trait | `#[service]` proc macro | In-file expansion (no separate file) |

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

> **Note:** The generated code references runtime types by their real crate path
> (`foundation_connectrpc::Ctx`, `foundation_connectrpc::Router`, …), so your crate
> just needs `foundation_connectrpc` as a regular dependency — **no alias**.

> **How to find the prost-build filename:** If you're unsure what prost-build
> named its output, run `cargo build` once then list the directory:
> ```sh
> ls target/debug/build/<your-crate>-*/out/
> ```
> You'll see your package-named `.rs` file(s) alongside `_connectrpc.rs`.

---

### Mode 2 — protoc plugin

`protoc` can run multiple plugins in one invocation. The binary
`protoc-gen-connect-ewe` generates *service* code; pair it with `--prost_out`
(from prost-build's `protoc-gen-prost`) for *message* types, and you get both
from a single command.

Install the plugins once:

```sh
cargo install foundation_connectrpc_codegen   # provides protoc-gen-connect-ewe
cargo install protoc-gen-prost                # provides protoc-gen-prost
```

Then run `protoc`:

```sh
protoc \
    --prost_out=src/gen \
    --connect-ewe_out=src/gen \
    --proto_path=proto \
    proto/greet.proto
```

This one `protoc` call produces two files in `src/gen/`:

| Plugin | Flag | Generates | Output file |
|---|---|---|---|
| `protoc-gen-prost` | `--prost_out` | Message structs with `buffa::Message` impls | `<proto-package>.rs` (e.g. `connectrpc.greet.v1.rs`) |
| `protoc-gen-connect-ewe` | `--connect-ewe_out` | Service trait, client, registration, procedure constants | `_connectrpc.rs` |

Include both in your crate:

```rust
// src/lib.rs
pub mod gen {
    // Service stubs.
    pub mod _connectrpc;
    // Message types — prost uses dots in filenames, which `mod` can't name
    // directly, so use `include!` inside a module block.
    pub mod connectrpc_greet_v1 {
        include!("connectrpc.greet.v1.rs");
    }
}
```

> **Service-only:** If you already have message types from another source,
> omit `--prost_out`:
> ```sh
> protoc --connect-ewe_out=src/gen --proto_path=proto proto/greet.proto
> ```

The generated `_connectrpc.rs` is identical to what Mode 1 produces.

---

### Mode 3 — code-first (proc macro)

No `.proto` file at all. Define your service as a plain Rust trait and annotate
it with `#[service]`. The macro expands **in the same file** — it does not write a
separate `.rs` file. Everything is generated at compile time as part of normal
proc-macro expansion.

The generated code refers to the runtime crate by its **real name**
(`foundation_connectrpc::…`) — so there is **no crate alias**. Import the `service`
macro and the types you use:

```rust
use foundation_connectrpc::{service, Ctx, Request, Response, ConnectResult};

#[service(package = "demo.greet.v1", codecs(json))]
pub trait GreetService {
    async fn greet(
        &self,
        _ctx: Ctx,
        _req: Request<GreetRequest>,
    ) -> ConnectResult<Response<GreetResponse>>;
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
| R8 | **Descriptor macro** | `#[macro_export] macro_rules! greet_service_rpc_definitions { (common) => { ... }, (server) => { ... }, (client) => { ... } }` | Captures the expansion in filtered arms for cross-crate re-generation via `generate!`. |

**Name derivation — every generated name follows mechanically from the trait's
`Ident`.** There are three conversion rules, all standard Rust convention:

| Trait name | → snake_case | → PascalCase composition | → SCREAMING_SNAKE_CASE |
|---|---|---|---|
| `GreetService` | `greet_service` | `GreetService` + `Client` → `GreetServiceClient` | `GREETSERVICE_NAME` |
| `MyApi` | `my_api` | `MyApi` + `ClientExt` → `MyApiClientExt` | `MYAPI_NAME` |
| `GetURL` | `get_url` | `GetURL` + `Handler` → `UnimplementedGetURLHandler` | `GETURL_NAME` |

Where each convention is used:

| Convention | Applied to |
|---|---|
| **snake_case** | Registration function (`register_greet_service`), descriptor macro (`greet_service_rpc_definitions`), client field names (`greet: Client<…>`) |
| **PascalCase** | Trait name (unchanged), client struct (`GreetServiceClient`), client trait (`GreetServiceClientExt`), unimplemented handler (`UnimplementedGreetServiceHandler`) |
| **SCREAMING_SNAKE_CASE** | Service name constant (`GREETSERVICE_NAME`), procedure path constants (`GREET`) |

> **`{ server, client }` in `generate!` — what these labels mean:**
>
> The `generate!` macro was designed so that a consuming crate could ask for
> *only* the artifacts it needs rather than always getting all 8. The labels
> map to the R1-R8 items from the table above:
>
> | Label | Artifacts it selects | Typical consumer |
> |---|---|---|
> | `server` | R1 (procedure constants), R2 (name), R3 (trait), R4 (registration fn), R5 (unimplemented handler) | The crate that implements the service |
> | `client` | R1 (procedure constants), R6 (typed client struct), R7 (client trait) | A crate that only calls the service |
>
> **Today's behavior:** Filtering is not yet implemented. The macro parses
> `{ server, client }` but always emits every artifact regardless. The
> recommended practice is to write `{ server, client }` when you need both
> sides, or just `{ server }` / `{ client }` when your crate only plays one
> role — that way your intent is recorded for when filtering lands.

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

See `examples/code_first_service/` for the full runnable version.

#### Implementing the service — another crate (cross-crate)

When the trait is defined in one crate (e.g. an API crate) and implemented in
another (e.g. a server crate), you need to re-materialise the generated
artifacts in the consuming crate. This is what `generate!` does.

**Step 1 — the API crate** depends only on `foundation_connectrpc`. Annotate a
`pub trait` with `#[service]`:

```rust
// my-api/src/lib.rs
use foundation_connectrpc::{service, Ctx, Request, Response, ConnectResult};

#[service(package = "my.api.v1", codecs(json))]
pub trait MyApi {
    async fn do_thing(
        &self,
        ctx: Ctx,
        req: Request<MyRequest>,
    ) -> ConnectResult<Response<MyResponse>>;
}
```

**What Crate A exports:** Alongside the trait and the in-crate generated items
(see [the artifact table](#what-the-macro-generates)), the `#[service]`
attribute also emits a hidden `#[macro_export]` descriptor macro with
**filtered arms** so `generate!` only emits what you ask for:

```
#[macro_export]
macro_rules! my_api_rpc_definitions {
    (common) => { /* service name const + procedure paths */ };
    (server) => { /* trait + register fn + unimplemented handler */ };
    (client) => { /* typed client struct + client trait */ };
    ()        => { /* everything — fallback for empty `{}` */ };
}
```

The macro name is derived from the trait name: `MyApi` → snake_case →
`my_api` + `_rpc_definitions`. Crate B will invoke it through `generate!`.

**Step 2 — the server crate** depends on **both** `my-api` and
`foundation_connectrpc`. Use `generate!` to re-expand the descriptor macro into a
module of your choosing:

```rust
// my-server/src/main.rs
use foundation_connectrpc::generate;

// Re-expand the service artifacts into a `my_svc` module.
// `my_api::my_api_rpc_definitions` — the descriptor macro exported by Crate A.
// `{ server, client }` — ask for both server-side and client-side artifacts
// (see the note under "What the macro generates" above for what each label selects).
generate!(my_api::my_api_rpc_definitions => mod my_svc {
    server, client
});
```

**What lands in `my_svc`:** The same 8 items the `#[service]` attribute
generates — re-expanded afresh, so the trait, client, registration fn, and
procedure constants all resolve within the consuming crate's dependency context.
Everything is namespaced under the module you chose (`my_svc`):

| Item | Generated name (from trait `MyApi`) |
|---|---|
| Procedure constants | `my_svc::procedure::DO_THING` |
| Service name constant | `my_svc::MYAPI_NAME` |
| Service trait | `my_svc::MyApi` |
| Registration function | `my_svc::register_my_api` |
| Unimplemented handler | `my_svc::UnimplementedMyApiHandler` |
| Typed client | `my_svc::MyApiClient` |
| Client trait | `my_svc::MyApiClientExt` |

> **Name derivation rule:** Every generated name is deterministic — it's the
> trait name (`MyApi`) converted via standard Rust conventions:
>   - snake_case (`to_snake_case("MyApi")` → `my_api`) for functions and the
>     descriptor macro
>   - PascalCase for struct/trait/enum names (`MyApi` + `Client` → `MyApiClient`)
>   - SCREAMING_SNAKE_CASE for constants (`MYAPI_NAME`)
>
> You never guess these names; they follow mechanically from the trait's
> `Ident`.

**Step 3 — implement and wire up:**

```rust
// Continuing in my-server/src/main.rs

use my_svc::{MyApi, MyApiClient, register_my_api};

struct MyServer;

impl MyApi for MyServer {
    async fn do_thing(
        &self,
        ctx: foundation_connectrpc::Ctx,
        req: foundation_connectrpc::Request<MyRequest>,
    ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<MyResponse>> {
        // ...
    }
}

// Server:
let mut router = foundation_connectrpc::Router::new();
register_my_api(&mut router, Arc::new(MyServer));

// Client:
let client = MyApiClient::new(transport, &base_url, ClientOptions::new().with_codec("json"))?;
let resp = client.do_thing(ctx, Request::new(MyRequest { /* ... */ })).await?;
```

> **Why not just `use` the items from Crate A?** The descriptor macro approach
> means Crate B *regenerates* the items rather than sharing them. This avoids
> Crate A needing to re-export every generated artifact (which would pollute its
> public API with client structs and registration fns it doesn't need), and
> ensures the generated code's `foundation_connectrpc::` paths resolve against Crate B's
> own dependency graph.
>
> Under the hood: `#[service]` wraps everything in a `macro_rules!` that
> captures the full token stream. `generate!` invokes that macro inside a
> `pub mod <name> { ... }` block. No files are touched — it's pure compile-time
> proc-macro expansion.

#### Streaming methods in code-first

All four RPC kinds are supported. The macro classifies each method by inspecting
its parameter and return types syntactically:

```rust
#[service(package = "demo.v1", codecs(json))]
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

> **Note:** For server-streaming and bidi-streaming methods the code-first macro
> rewrites `impl Stream` return types to `Pin<Box<dyn Stream + Send>>` to satisfy
> `Send` bounds on the Router's handler futures. Your impl returns
> `Ok(Box::pin(stream))` — see the `code_first_streaming` example.

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
        ctx: foundation_connectrpc::Ctx,
        req: foundation_connectrpc::Request<GreetRequest>,
    ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<GreetResponse>>;
}

// ── R4: Registration function ─────────────────────────────────────────────
pub fn register_greet_service<S: GreetService>(
    router: &mut foundation_connectrpc::Router,
    service: std::sync::Arc<S>,
) {
    // Calls router.unary(procedure::GREET, codecs, handler, opts) for each method
}

// ── R5: Unimplemented handler ─────────────────────────────────────────────
pub struct UnimplementedGreetServiceHandler;
impl GreetService for UnimplementedGreetServiceHandler {}

// ── R6: Typed client ──────────────────────────────────────────────────────
pub struct GreetServiceClient {
    greet: foundation_connectrpc::Client<GreetRequest, GreetResponse>,
}
impl GreetServiceClient {
    pub fn new(
        transport: Arc<dyn foundation_connectrpc::Transport>,
        base_url: &str,
        options: foundation_connectrpc::ClientOptions,
    ) -> foundation_connectrpc::ConnectResult<Self>;

    pub async fn greet(
        &self,
        ctx: foundation_connectrpc::Ctx,
        request: foundation_connectrpc::Request<GreetRequest>,
    ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<GreetResponse>>;
}

// ── R7: Client trait ──────────────────────────────────────────────────────
pub trait GreetServiceClientExt: Send + Sync + 'static {
    async fn greet(
        &self,
        ctx: foundation_connectrpc::Ctx,
        request: foundation_connectrpc::Request<GreetRequest>,
    ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<GreetResponse>>;
}
impl GreetServiceClientExt for GreetServiceClient { /* delegates to inner Client */ }

// ── R8: Descriptor macro (Mode 3 only) ────────────────────────────────────
#[macro_export]
macro_rules! greet_service_rpc_definitions { (common) => { ... }; (server) => { ... }; (client) => { ... }; }
```

> **Note:** The generated code uses the crate's **real path** as its prefix
> throughout (`foundation_connectrpc::Ctx`, `foundation_connectrpc::Router`, …), so
> the consuming crate just needs `foundation_connectrpc` as a dependency — no alias.

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
