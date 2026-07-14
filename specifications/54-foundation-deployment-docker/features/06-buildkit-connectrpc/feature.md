---
feature: "BuildKit gRPC client via foundation_connectrpc"
description: "Vend 17 BuildKit proto files from local moby checkout; generate Rust types via foundation_connectrpc_codegen; implement async Client<Req,Res> over Unix socket H1Transport for Solve (bidi), Status (server-stream), Info (unary), DiskUsage, Prune"
status: "in-progress"
priority: "low"
phase: 2
depends_on: ["02-unix-socket-transport", "04-type-replication"]
estimated_effort: "large"
created: 2026-07-12
updated: 2026-07-14
---
# Feature 06: BuildKit via foundation_connectrpc

## Implementation reconciliation (2026-07-14)

The doc below is the original design sketch; the implementation corrected it in
several places — trust this list where they disagree:

- **`Control/Solve` is UNARY**, not bidi. The `SolveRequest` carries everything;
  progress arrives on the separate `Status` server-stream. (The sketch's
  `solve(ctx, reqs: impl Stream)` shape was wrong.)
- **gRPC cannot ride `H1Transport`** — it needs HTTP/2. `BuildKitClient::connect_tcp`
  uses `H2Transport` (h2c prior-knowledge over TCP) and is validated against a
  real buildkitd v0.31.1. h2c-over-Unix-socket is still an open transport gap;
  `connect(socket_path)` currently wires H1 and cannot do gRPC.
- **The Session sidecar is the real work** (absent from the sketch): buildkitd
  dials *back* into a client-run gRPC server tunneled inside the
  `Control/Session` bidi. Implemented in `src/buildkit/session.rs`
  (`SessionServer` + `DirFileSync` DiffCopy + `HealthService`); protocol
  deep-dive + session-death root cause in
  `backends/foundation_deployment_docker/specs/buildkit/README.md`.
- **End-to-end dockerfile build is GREEN** (`build_dockerfile_end_to_end`):
  session stays open, context served via FileSync, `dockerfile.v0` solves.
- Codegen went through `buffa-build` (messages) + `foundation_connectrpc_codegen`
  (service traits/clients/register fns), per-service, in `build.rs`.

**Outstanding for completion** (tracked in progress.md Phase 3): gRPC over the
Unix socket, Status consumed during a build, exporters + `FileSend/Send`,
inline Dockerfile, Auth (P1), Secrets/SSH/Gateway/build-history (P2), gRPC
error-trailer decoding client-side, `H2PooledTransport` finish-or-delete,
diagnostics → `tracing`.

## Why

BuildKit is Docker's next-generation build system. Unlike the classic `docker build`
(HTTP API), BuildKit communicates over gRPC on a Unix socket. It provides efficient
layer caching, parallel execution, SSH forwarding, secret management, multi-platform
builds, and OCI export.

This is P2 — the testbed primarily pulls existing images. BuildKit is needed for
image builds and cache management.

## Proto sources

Vend 17 proto files from `/home/darkvoid/Boxxed/@formulas/src.rust/src.Containers/src.moby/buildkit/`
into `foundation_deployment_docker/specs/buildkit/`, preserving Go-style import paths:

```
specs/buildkit/
  github.com/moby/buildkit/
    api/services/control/control.proto
    api/types/worker.proto
    frontend/gateway/pb/gateway.proto
    session/auth/auth.proto
    session/exporter/exporter.proto
    session/filesync/filesync.proto
    session/secrets/secrets.proto
    session/sshforward/ssh.proto
    session/upload/upload.proto
    solver/errdefs/errdefs.proto
    solver/pb/ops.proto
    sourcepolicy/pb/policy.proto
    sourcepolicy/policysession/policysession.proto
    util/apicaps/pb/caps.proto
    util/stack/stack.proto
```

Plus vendored containerd protos needed for type references:
```
  github.com/containerd/containerd/
    api/types/platform.proto
    api/types/descriptor.proto
    api/types/mount.proto
```

Run `foundation_connectrpc_codegen` to produce Rust types + `ProcedureCodecs` +
`buffa::Message` impls.

## BuildKit services to implement

| Service | Method | RPC type | Priority |
|---------|--------|----------|----------|
| `Control/Solve` | Solve | bidi-stream | P0 |
| `Control/Status` | Status | server-stream | P0 |
| `Control/Info` | Info | unary | P0 |
| `Control/DiskUsage` | DiskUsage | unary | P1 |
| `Control/Prune` | Prune | unary | P1 |
| `Control/ListWorkers` | ListWorkers | unary | P2 |
| `Control/ListenBuildHistory` | ListenBuildHistory | server-stream | P2 |
| `Control/UpdateBuildHistory` | UpdateBuildHistory | unary | P2 |
| `FileSend/Send` | Send | unary | P1 |
| `AuthProvider/Credentials` | Credentials | unary | P1 |
| `SecretProvider/GetSecret` | GetSecret | unary | P2 |
| `SshProvider/CheckAgent` | CheckAgent | unary | P2 |
| `Gateway/Solve` | Solve | bidi-stream | P2 |
| `Gateway/ResolveImageConfig` | ResolveImageConfig | unary | P2 |

P0 = needed for basic `docker build` parity. P1 = needed for privates repos +
context upload. P2 = advanced features.

## Implementation

### 1. Transport

```rust
use foundation_connectrpc::{Client, ClientOptions, Ctx, H1Transport, ProcedureCodecs};
use foundation_connectrpc::shared::transport::Transport;
use foundation_netio::HttpClientBuilder;

fn buildkit_transport(socket_path: &str) -> Arc<dyn Transport> {
    let http = HttpClientBuilder::new()
        .unix_socket(socket_path)
        .build();
    Arc::new(H1Transport::new(http))
}
```

Same Unix socket transport as Docker daemon (Feature 02). `H1Transport` wraps
`DynNetClient` — no custom transport needed.

### 2. Control client

```rust
/// BuildKit control client over Unix socket.
pub struct BuildKitClient {
    solve: Client<SolveRequest, SolveResponse>,
    status: Client<StatusRequest, StatusResponse>,
    info: Client<InfoRequest, InfoResponse>,
}

impl BuildKitClient {
    pub fn connect(socket_path: impl AsRef<Path>) -> Result<Self, BuildKitError> {
        let transport = buildkit_transport(socket_path.as_ref());
        let path = socket_path.as_ref().to_string_lossy();

        Ok(Self {
            solve: Client::new(
                Arc::clone(&transport),
                "http://localhost/moby.buildkit.v1.Control/Solve",
                ProcedureCodecs::<SolveRequest, SolveResponse>::defaults(),
                ClientOptions::new().with_grpc(),
            )?,
            status: Client::new(
                Arc::clone(&transport),
                "http://localhost/moby.buildkit.v1.Control/Status",
                ProcedureCodecs::<StatusRequest, StatusResponse>::defaults(),
                ClientOptions::new().with_grpc(),
            )?,
            info: Client::new(
                transport,
                "http://localhost/moby.buildkit.v1.Control/Info",
                ProcedureCodecs::<InfoRequest, InfoResponse>::defaults(),
                ClientOptions::new().with_grpc(),
            )?,
        })
    }

    /// Execute a build (bidi-stream).
    pub async fn solve(
        &self,
        ctx: Ctx,
        reqs: impl Stream<Item = SolveRequest>,
    ) -> ConnectResult<BidiStream<SolveRequest, SolveResponse>> {
        self.solve.bidi_stream(ctx, reqs).await
    }

    /// Stream build status.
    pub async fn status(
        &self,
        ctx: Ctx,
        req: StatusRequest,
    ) -> ConnectResult<ServerStream<StatusResponse>> {
        self.status.server_stream(ctx, Request::new(req)).await
    }

    /// Get server info.
    pub async fn info(&self, ctx: Ctx) -> ConnectResult<InfoResponse> {
        self.info.unary(ctx, Request::new(InfoRequest::default())).await
            .map(|r| r.msg)
    }
}
```

### 3. Solve (bidi-streaming build)

```rust
async fn build_image(
    client: &BuildKitClient,
    definition: LlbDefinition,
) -> ConnectResult<()> {
    let ctx = Ctx::background().with_deadline(Duration::from_secs(600));
    let reqs = futures::stream::once(async { SolveRequest {
        definition: Some(definition),
        ..Default::default()
    }});

    let mut stream = client.solve(ctx, reqs).await?;

    while let Some(resp) = stream.receive().await? {
        for vertex in &resp.vertexes {
            tracing::info!(name=%vertex.name, "vertex: {:?}", vertex.state);
        }
        for log in &resp.logs {
            tracing::info!("{}", String::from_utf8_lossy(&log.msg));
        }
    }

    stream.response_trailers().await?;
    Ok(())
}
```

### 4. Dockerfile support

BuildKit's frontend gateway supports inline Dockerfiles — no build context
tarball needed for simple builds:

```rust
async fn build_dockerfile(
    client: &BuildKitClient,
    dockerfile: &str,
    image_name: &str,
) -> ConnectResult<()> {
    let req = SolveRequest {
        frontend: "dockerfile.v0".into(),
        frontend_attrs: vec![
            ("filename".into(), "Dockerfile".into()),
            ("inline".into(), dockerfile.to_string()),
        ],
        exporter: format!("image.name={image_name}"),
        ..Default::default()
    };

    let reqs = futures::stream::once(async { req });
    let mut stream = client.solve(Ctx::background(), reqs).await?;
    while let Some(resp) = stream.receive().await? { /* ... */ }
    Ok(())
}
```

## Scope

| Crate | Change |
|-------|--------|
| `foundation_deployment_docker` | Vendored protos, generated types, `BuildKitClient` |

## Verification

- Proto code generation produces compiling Rust types
- `BuildKitClient::connect("/run/buildkit/buildkitd.sock")` compiles
- All 4 RPC shapes type-check (unary, server-stream, client-stream, bidi)
- `bidi_stream` type signature accepts `impl Stream<Item = SolveRequest>`
- Inline Dockerfile build compiles

## Acceptance criteria

1. 17 proto files vendored, codegen produces compiling Rust code
2. `BuildKitClient` wraps `Client<Req, Res>` per service method
3. `Solve` (bidi), `Status` (server-stream), `Info` (unary) work over Unix socket
4. Inline Dockerfile support works (no tarball needed)
5. P1 methods (DiskUsage, Prune, FileSend, AuthProvider/Credentials) compile
