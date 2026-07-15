# 06 — BuildKit via foundation_connectrpc

**Date:** 2026-07-10
**Updated:** 2026-07-12 (local moby checkout, accurate connectrpc surface)
**Status:** Resolved

## Decision

Implement BuildKit RPC on `foundation_connectrpc` using protobuf definitions from
our **local moby checkout** at
`/home/darkvoid/Boxxed/@formulas/src.rust/src.Containers/src.moby/buildkit/`.

The connectrpc `Transport` trait is the byte-level seam — we implement a
Unix-socket-backed transport that speaks HTTP/1.1 (or HTTP/2 for gRPC streaming)
to `buildkitd`. The typed `Client<SolveRequest, SolveResponse>` handles codec
marshalling and protocol framing.

## Proto file sources (first-party)

```
/home/darkvoid/Boxxed/@formulas/src.rust/src.Containers/src.moby/buildkit/
├── api/
│   ├── services/control/control.proto     # Main build control (Solve, Status, Info, etc.)
│   └── types/worker.proto                 # Worker types
├── frontend/gateway/pb/gateway.proto      # Frontend gateway (LLB → BuildKit bridge)
├── session/
│   ├── auth/auth.proto                    # Registry authentication
│   ├── exporter/exporter.proto            # Build export (tar, OCI, etc.)
│   ├── filesync/filesync.proto            # File sync (COPY context)
│   ├── secrets/secrets.proto              # Build secrets
│   ├── sshforward/ssh.proto               # SSH agent forwarding
│   └── upload/upload.proto                # Upload provider
├── solver/
│   ├── errdefs/errdefs.proto              # Solver error definitions
│   └── pb/ops.proto                       # Build operations (LLB → DAG)
├── sourcepolicy/
│   ├── pb/policy.proto                    # Source policies
│   └── policysession/policysession.proto  # Policy session
└── util/
    ├── apicaps/pb/caps.proto              # API capability negotiation
    └── stack/stack.proto                  # Stack traces
```

17 first-party proto files. The vendored containerd protos (`vendor/github.com/containerd/`)
provide types referenced by BuildKit (platform, descriptor, mount).

`foundation_connectrpc_codegen` auto-generates `ProcedureCodecs`, message types,
and `buffa::Message` impls from proto files — the same tooling used for all
connectrpc services. No hand-written codec code needed.

## The connectrpc client surface (how BuildKit plugs in)

### `Client<Req, Res>` — typed per-procedure RPC client

```rust
pub struct Client<Req, Res> {
    transport: Arc<dyn Transport>,    // byte-level transport seam
    config: ClientConfig,             // frozen options (protocol, codec, compression, …)
    protocol: Box<dyn ProtocolClient>, // Connect / gRPC / gRPC-Web
    codecs: ProcedureCodecs<Req, Res>,// marshal/unmarshal per codec name
}

impl<Req: Send + 'static, Res: Send + 'static> Client<Req, Res> {
    pub fn new(
        transport: Arc<dyn Transport>,
        url: &str,
        codecs: ProcedureCodecs<Req, Res>,
        options: ClientOptions,
    ) -> ConnectResult<Self>;

    // ── The four async RPC methods ──────────────────────────────
    pub async fn unary(&self, ctx: Ctx, request: Request<Req>) -> ConnectResult<Response<Res>>;
    pub async fn server_stream(&self, ctx: Ctx, request: Request<Req>) -> ConnectResult<ServerStream<Res>>;
    pub async fn client_stream(&self, ctx: Ctx, reqs: impl Stream<Item = Req>) -> ConnectResult<Response<Res>>;
    pub async fn bidi_stream(&self, ctx: Ctx, reqs: impl Stream<Item = Req>) -> ConnectResult<BidiStream<Req, Res>>;
}
```

### `Transport` trait — the byte-level seam

```rust
pub trait Transport: Send + Sync {
    fn capabilities(&self) -> TransportCapabilities;
    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError>;
}

pub struct TransportStream {
    pub send_body: Arc<dyn SendBody>,     // push Bytes, close
    pub head: HeadStream,                  // Pin<Box<dyn Stream<Item = Result<(Status, Headers), Error>>>>
    pub recv_body: BodyStream,             // Pin<Box<dyn Stream<Item = Result<Bytes, Error>>>>
    pub trailers: Pipe<SimpleHeaders>,     // trailing headers
}
```

`open()` is **synchronous** — it spawns the byte pump on valtron and returns
pipe halves immediately. The caller is already in a valtron context.

### `H1Transport` — HTTP/1.1 transport (what we use for Unix sockets)

```rust
pub struct H1Transport {
    client: Arc<dyn HttpClient>,  // foundation_netio's HttpClient
}

impl Transport for H1Transport {
    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        // 1. PreparedRequest from RequestDescriptor
        // 2. self.client.open_exchange(prepared) → HttpExchangeClientTask
        // 3. split_collect_until_map(head) + split_collector_map(body)
        // 4. valtron::send(continuation)
        // 5. Return TransportStream with pipe halves
    }
}
```

### The pattern: unary RPC

Taken from `examples/unary_echo/main.rs`:

```rust
#[valtron(seed = 1, threads = 4)]
async fn main() {
    let transport: Arc<dyn Transport> = Arc::new(
        H1Transport::new(Arc::new(NativeHttpClient::from_system()))
    );
    let client: Client<EchoMessage, EchoMessage> = Client::new(
        transport,
        "http://127.0.0.1:1234/demo.EchoService/Echo",
        ProcedureCodecs::<EchoMessage, EchoMessage>::defaults(),
        ClientOptions::new(),
    ).expect("build client");

    let ctx = Ctx::background().with_deadline(Duration::from_secs(10));
    let request = Request::new(EchoMessage { id: 7, text: "hello".into() });
    let response = client.unary(ctx, request).await.unwrap();
}
```

## BuildKit client implementation

### Transport: Unix socket

BuildKitd listens on a Unix socket (`/run/buildkit/buildkitd.sock` by default).
We need a `Transport` impl that opens HTTP/1.1 connections over that socket.

`H1Transport` wraps `Arc<dyn HttpClient>` — if `DynNetClient` can connect to
Unix sockets (decision 07), `H1Transport` already works for BuildKit. No custom
transport needed.

### Client setup

```rust
use foundation_connectrpc::{
    Client, ClientOptions, Ctx, H1Transport, ProcedureCodecs, Request, Response,
};
use foundation_connectrpc::shared::transport::Transport;
use foundation_netio::{DynNetClient, HttpClientBuilder};

/// Connect to buildkitd over a Unix socket.
async fn buildkit_client(
    socket_path: &str,
) -> ConnectResult<Client<SolveRequest, SolveResponse>> {
    // Build a DynNetClient that connects over the Unix socket.
    let http: DynNetClient = HttpClientBuilder::new()
        .unix_socket(socket_path)       // hypothetical — see decision 07
        .build();

    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(http));

    Client::new(
        transport,
        "http://localhost/moby.buildkit.v1.Control",
        ProcedureCodecs::<SolveRequest, SolveResponse>::defaults(),
        ClientOptions::new().with_grpc(),   // BuildKit speaks gRPC
    )
}
```

### Solve (bidi-streaming build)

```rust
async fn build_image(
    client: &Client<SolveRequest, SolveResponse>,
    ctx: &Ctx,
    definition: LlbDefinition,
) -> ConnectResult<()> {
    // bidi_stream: send SolveRequests, receive SolveResponses
    let reqs = futures::stream::iter([
        SolveRequest { definition: Some(definition), ..Default::default() },
    ]);

    let mut stream = client.bidi_stream(ctx.clone(), reqs).await?;

    // Stream build progress.
    while let Some(resp) = stream.receive().await? {
        for vertex in resp.vertexes {
            tracing::info!(name = %vertex.name, "BuildKit vertex: {:?}", vertex.status);
        }
        for log in resp.logs {
            tracing::info!("BuildKit: {}", String::from_utf8_lossy(&log.msg));
        }
    }

    stream.response_trailers().await?;
    Ok(())
}
```

### Info (unary)

```rust
async fn buildkit_info(
    client: &Client<InfoRequest, InfoResponse>,
) -> ConnectResult<InfoResponse> {
    let ctx = Ctx::background().with_deadline(Duration::from_secs(5));
    let resp = client.unary(ctx, Request::new(InfoRequest::default())).await?;
    Ok(resp.msg)
}
```

### Status (server-streaming)

```rust
async fn build_status(
    client: &Client<StatusRequest, StatusResponse>,
    build_ref: String,
) -> ConnectResult<()> {
    let ctx = Ctx::background();
    let mut stream = client.server_stream(
        ctx,
        Request::new(StatusRequest { ref_: build_ref }),
    ).await?;

    while let Some(status) = stream.receive().await? {
        tracing::info!("Build status: {:?}", status);
    }

    Ok(())
}
```

## What is BuildKit

BuildKit is Docker's next-generation build system. Unlike the classic `docker build`
(which uses the HTTP API), BuildKit communicates over gRPC on a Unix socket.

BuildKit's frontend gateway supports inline Dockerfiles — a `String` Dockerfile
definition can be sent directly in the `SolveRequest` without needing a build
context tarball. This enables runtime-defined builds from code-generated
Dockerfiles with no temp file staging.

- **Efficient layer caching** — content-addressable, deduplicated
- **Parallel build execution** — independent stages run concurrently
- **SSH agent forwarding** — private git deps without embedding keys
- **Secret management** — no secrets in image layers
- **Multi-platform builds** — cross-compile in a single build
- **OCI image export** — standards-compliant output

## Proto code generation

```
foundation_deployment_docker/
  specs/
    buildkit/                       # Copied from local moby checkout
      github.com/moby/buildkit/
        api/services/control/control.proto
        api/types/worker.proto
        session/auth/auth.proto
        session/filesync/filesync.proto
        ...
```

Use `foundation_connectrpc_codegen` to generate Rust types + `ProcedureCodecs`
from the proto files. The proto import paths use Go-style fully-qualified names
(`github.com/moby/buildkit/api/types/worker.proto`) — preserve the directory
structure and set the proto include path to `specs/buildkit/`.

## Deferral rationale

BuildKit is P2 because:

1. The testbed use case primarily pulls existing images, not builds new ones
2. The HTTP API surface (containers, images, networks, volumes) is the priority
3. BuildKit adds proto dependency surface (~17 files) with cross-referencing
   Go-style import paths
4. The bidi-streaming `Solve` call exercises the most complex connectrpc codepath —
   we own connectrpc, so building on it finds the gaps and we fix them

## 2026-07-14 update — implementation reality (supersedes sketches above)

Several sketch details above were wrong or incomplete once implemented. The
authoritative facts:

### Message codegen — `buffa-build`, not prost, not `foundation_connectrpc_codegen::build::Config`

`ProcedureCodecs::defaults()` bounds on `buffa::Message + Serialize +
DeserializeOwned`. `foundation_connectrpc_codegen::build::Config` generates
message types via **prost** (`prost::Message`) — the wrong trait. The correct
tool is **`buffa-build`** (crates.io `buffa-build`/`buffa-types`/`protoc-gen-buffa`
v0.8.1 — `buffa` is the same runtime our `ProtoCodec` uses):

```rust
// build.rs, gated #[cfg(feature = "buildkit")] (build scripts get feature cfgs)
buffa_build::Config::new()
    .files(&[control, worker, ops, policy, status])   // vendored .proto paths
    .includes(&["specs/buildkit"])                     // include root
    .generate_json(true)                               // adds serde for Connect-JSON
    .compile()?;                                       // shells to protoc (v34.1)
```

`buffa-types` must enable `features = ["json"]` (WKT serde). Emits one file per
proto package into `OUT_DIR`; `src/buildkit/generated.rs` hand-wires the package
module tree (`pb`, `google::rpc`, `moby::buildkit::v1{,::types,::sourcepolicy}`)
— the generated code uses relative `super::super::super::pb::…` paths, so the
nesting is load-bearing. Generated fields are **proto-cased** (`StatusRequest.Ref`,
`SolveRequest.Frontend`, `InfoResponse.buildkitVersion`) with a hidden
`__buffa_unknown_fields` (construct with `..Default::default()`).

### RPC shapes (from `control.proto`, not the earlier guess)

`Solve` is **unary** (`rpc Solve(SolveRequest) returns (SolveResponse)`), *not*
bidi. `Status` is server-stream, `Session` is the bidi. Implemented on
`foundation_connectrpc` `Client<Req,Res>` + `H1Transport` over the Unix socket:
`info`/`solve`/`disk_usage`/`list_workers` (unary), `status`/`prune`
(server-stream). All with `ProcedureCodecs::defaults()`.

### The remaining hard part — the `Session` sidecar (build-context callback)

A Dockerfile build needs the client to serve gRPC *back* to buildkitd: the
`dockerfile.v0` frontend reads the Dockerfile + context via **FileSync**
(`moby.filesync.v1.FileSync/DiffCopy`, fsutil packet protocol), plus optional
**Auth** / **Secrets** / **SSH** services. buildkitd multiplexes these calls over
the single `Control/Session` bidi stream (each `BytesMessage` carries a chunk of
a tunnelled h2 connection); `SolveRequest.Session` ties a solve to that stream.

**Concrete plan (design fixed, execution pending):**
1. Vendor + `buffa-build` the session protos: `session/filesync/filesync.proto`,
   `session/auth/auth.proto`, `session/secrets/secrets.proto`,
   `session/sshforward/ssh.proto`, and the fsutil `types/*.proto` (from
   `github.com/tonistiigi/fsutil`).
2. **Bridge** (mechanism confirmed against the APIs): `Control/Session` is a
   connectrpc bidi RPC (`BidiStream<BytesMessage, BytesMessage>`) whose
   `send`/`receive` are **async**; `OverlayReadWrite` is **sync** `Read`/`Write`.
   So: `BidiStream::split()` into `(BidiSender, BidiReceiver)`, run a valtron pump
   task that drains `BidiReceiver` into a sync byte buffer (feeds `Read`) and
   drains a sync outbound buffer into `BidiSender` (fed by `Write`), each side
   framing/deframing `BytesMessage{data}`. Wrap that sync duplex as
   `OverlayReadWrite` → `Connection::Overlay` (the non-fd byte-stream variant, no
   `AsRawFd` — unlike `Completion`) → single-shot `Acceptor` →
   `HttpServer::serve_with_acceptor`, so our own HTTP/2 server serves the session
   gRPC over the tunnel. Note: gRPC to buildkitd needs HTTP/2 — the *client*
   Control calls already use `H2Transport` (`connect_tcp`); H1 cannot carry gRPC.
3. Implement the FileSync `DiffCopy` handler to stream a local build-context dir
   as fsutil packets; stub Auth/Secrets/SSH to "none".
4. Drive `Solve` with the session id + `frontend=dockerfile.v0`; observe progress
   via `Status`.
5. Integration test needs a reachable buildkitd Control endpoint — dockerd embeds
   buildkit behind the `/session` + `/grpc` hijack (no `/run/buildkit/buildkitd.sock`
   by default), so the testbed must either run standalone `buildkitd` or reach the
   embedded one via the Docker hijack.

## Related

- **[02 — Docker Engine API Spec Source](02-docker-openapi-spec.md)** — Same local checkout
- **[04 — HTTP via DynNetClient + PreparedRequestBuilder](04-http-via-simple-http-client.md)** — HTTP transport
- **[07 — Unix socket transport](07-unix-socket-transport.md)** — Unix socket support
- **[Feature 06 — BuildKit via foundation_connectrpc](../features/06-buildkit-connectrpc/feature.md)** — Implementation plan
