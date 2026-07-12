# 06 — BuildKit via foundation_connectrpc

**Date:** 2026-07-10
**Updated:** 2026-07-12 (local moby checkout, async fn)
**Status:** Resolved

## Decision

Implement BuildKit RPC on top of `foundation_connectrpc` using the protobuf
definitions from our **local moby checkout** at
`/home/darkvoid/Boxxed/@formulas/src.rust/src.Containers/src.moby/buildkit/`.

This is deferred to P2 (after HTTP API surface is complete). All RPC functions
use `async fn` — consistent with decision 05 (generator emits `async fn`).

## Proto file sources (first-party)

The BuildKit protos live directly in the moby source tree — not as vendored copies
in bollard:

```
/home/darkvoid/Boxxed/@formulas/src.rust/src.Containers/src.moby/buildkit/
├── api/
│   ├── services/control/control.proto     # Main build control (Solve, Status, Info, etc.)
│   └── types/worker.proto                 # Worker types
├── cache/contenthash/checksum.proto       # Content-addressable checksums
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

17 first-party proto files. No vendor directory needed for the core API.

## What is BuildKit

BuildKit is Docker's next-generation build system. Unlike the classic `docker build`
(which uses the HTTP API), BuildKit communicates over **gRPC** on a Unix socket:

- **Efficient layer caching** — content-addressable, deduplicated
- **Parallel build execution** — independent stages run concurrently
- **SSH agent forwarding** — private git deps without embedding keys
- **Secret management** — no secrets in image layers
- **Multi-platform builds** — cross-compile in a single build
- **OCI image export** — standards-compliant output

## BuildKit gRPC services

| Service | Method | Type | Purpose |
|---------|--------|------|---------|
| **Control** | `Solve` | bidi-stream | Execute a build |
| | `Status` | server-stream | Stream build status/progress |
| | `DiskUsage` | unary | Report cache usage |
| | `Prune` | unary | Clean build cache |
| | `Info` | unary | BuildKit server info |
| | `ListWorkers` | unary | Connected workers |
| | `ListenBuildHistory` | server-stream | Build history events |
| | `UpdateBuildHistory` | unary | Update build record |
| **FileSend** (session) | `Send` | unary | Send build context files |
| **AuthProvider** (session) | `Credentials` | unary | Registry credentials |
| **SecretProvider** (session) | `GetSecret` | unary | Build secrets |
| **SshProvider** (session) | `CheckAgent` | unary | SSH agent forwarding |
| **Gateway** (frontend) | `Solve` | bidi-stream | Frontend LLB solve |
| | `ResolveImageConfig` | unary | Resolve image config |
| **ContentHash** (cache) | `Checksums` | server-stream | Content checksumming |

## Why foundation_connectrpc

`foundation_connectrpc` implements the Connect protocol (Connect, gRPC, gRPC-Web):

- `ProtoCodec` for protobuf message encoding
- `H1Transport` / `H2Transport` for HTTP transport
- Unix socket transport via custom `Transport` implementation
- `async fn` client API — consistent with decision 05
- Client with `ClientStream`, `ServerStream`, `BidiStream` support

This maps directly to BuildKit's gRPC interface without needing `tonic`.

## Implementation approach

### Phase 1: Proto code generation

```
foundation_deployment_docker/
  specs/
    buildkit/                       # Copied from local moby checkout
      api/services/control/control.proto
      api/types/worker.proto
      session/auth/auth.proto
      session/filesync/filesync.proto
      session/secrets/secrets.proto
      session/sshforward/ssh.proto
      session/upload/upload.proto
      session/exporter/exporter.proto
      solver/pb/ops.proto
      sourcepolicy/pb/policy.proto
      ...
```

Use `foundation_connectrpc_codegen` to generate Rust types from the proto files.
The proto import paths are already structured correctly in the moby tree.

### Phase 2: ConnectRPC client (async fn)

```rust
/// BuildKit control client over Unix socket.
pub struct BuildKitClient {
    client: connectrpc::Client,
}

impl BuildKitClient {
    /// Connect to buildkitd via Unix socket.
    pub async fn connect_unix(socket: impl AsRef<Path>) -> Result<Self, BuildKitError> {
        let transport = H1Transport::with_unix_socket(socket);
        let client = connectrpc::Client::new(transport);
        Ok(Self { client })
    }

    /// Execute a build. Returns a bidi stream for sending inputs
    /// and receiving status updates.
    pub async fn solve(
        &self,
        req: SolveRequest,
    ) -> Result<
        impl Stream<Item = Result<SolveResponse, ConnectError>> + Send,
        ConnectError,
    > {
        self.client.bidi_streaming("/moby.buildkit.v1.Control/Solve", req).await
    }

    /// Stream build status.
    pub async fn status(
        &self,
        req: StatusRequest,
    ) -> Result<
        impl Stream<Item = Result<StatusResponse, ConnectError>> + Send,
        ConnectError,
    > {
        self.client.server_streaming("/moby.buildkit.v1.Control/Status", req).await
    }

    /// Get server info.
    pub async fn info(&self) -> Result<InfoResponse, ConnectError> {
        self.client.unary("/moby.buildkit.v1.Control/Info", &InfoRequest::default()).await
    }

    /// Report disk usage.
    pub async fn disk_usage(&self) -> Result<DiskUsageResponse, ConnectError> {
        self.client.unary("/moby.buildkit.v1.Control/DiskUsage", &DiskUsageRequest::default()).await
    }

    /// Prune build cache.
    pub async fn prune(&self, req: PruneRequest) -> Result<PruneResponse, ConnectError> {
        self.client.unary("/moby.buildkit.v1.Control/Prune", req).await
    }
}
```

### Phase 3: Docker build integration

Wire BuildKit into the image build flow:

```rust
impl DockerClient {
    /// Build an image using BuildKit (async, P2).
    #[cfg(feature = "buildkit")]
    pub async fn build_image_with_buildkit(
        &self,
        options: BuildImageOptions,
        context: TarContext,
    ) -> Result<BuildInfo, BuildKitError> {
        let buildkit = BuildKitClient::connect_unix(
            self.buildkit_socket.as_deref().unwrap_or_else(|| Path::new("/run/buildkit/buildkitd.sock"))
        ).await?;

        // 1. Upload build context via FileSend session
        let ctx_ref = buildkit.upload_context(context).await?;

        // 2. Call Solve with build options
        let req = SolveRequest {
            definition: options.to_llb_definition()?,
            frontend: "dockerfile.v0".into(),
            // ...
        };

        // 3. Stream build progress
        let mut stream = buildkit.solve(req).await?;
        while let Some(status) = stream.next().await {
            match status {
                Ok(SolveResponse { vertex, logs, .. }) => {
                    tracing::info!(?vertex, "BuildKit: {}", String::from_utf8_lossy(&logs));
                }
                Err(e) => return Err(BuildKitError::Solve(e)),
            }
        }

        // 4. Return build info
        Ok(BuildInfo { /* ... */ })
    }
}
```

## Deferral rationale

BuildKit is P2 because:

1. The testbed use case primarily pulls existing images, not builds new ones
2. The HTTP API surface (containers, images, networks, volumes) is the priority
3. BuildKit adds significant proto dependency surface (~17 proto files) with
   cross-referencing Go-style import paths
4. The bidi-streaming `Solve` call exercises the most complex connectrpc codepath —
   we own connectrpc, so building on it finds the gaps and we fix them

## Proto import strategy

BuildKit protos import each other with paths like:

```protobuf
import "github.com/moby/buildkit/api/types/worker.proto";
import "github.com/moby/buildkit/solver/pb/ops.proto";
```

These are **fully-qualified Go-style import paths** — they reflect the Go module
structure, not filesystem paths. The `foundation_connectrpc_codegen` generator
must either:

- **Option A**: Map import paths to local files (maintain an import→path table)
- **Option B**: Vend the protos with their directory structure preserved and set
  the proto include path to the repo root

**Chosen: Option B** — simpler, matches how the moby tree is already structured.
Copy the protos preserving their `github.com/moby/buildkit/...` directory layout:

```
specs/buildkit/
  github.com/moby/buildkit/
    api/services/control/control.proto
    api/types/worker.proto
    session/auth/auth.proto
    ...
```

Set `--proto_path specs/buildkit/` during code generation.

## vendored containerd protos

BuildKit vendors containerd protos under `vendor/github.com/containerd/` — these
are NOT BuildKit-specific but are needed for type references (e.g., `containerd
types/platform.proto` for platform definitions). We only need a subset:

| Proto | Needed for |
|-------|-----------|
| `api/types/platform.proto` | Platform type used in SolveRequest |
| `api/types/descriptor.proto` | OCI descriptor type |
| `api/types/mount.proto` | Mount definitions |

Copy only the needed files, not the entire containerd API surface.

## Related decisions

- **[02 — Docker Engine API Spec Source](02-docker-openapi-spec.md)** — Same local checkout
- **[05 — Valtron TaskIterator Format](05-valtron-task-iterator.md)** — async fn pattern
- **[04 — HTTP via DynNetClient + PreparedRequestBuilder](04-http-via-simple-http-client.md)** — HTTP transport
