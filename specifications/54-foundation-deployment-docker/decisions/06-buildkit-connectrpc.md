# 06 — BuildKit via foundation_connectrpc

**Date:** 2026-07-10
**Status:** Resolved

## Decision

Implement BuildKit RPC on top of `foundation_connectrpc` using the protobuf definitions from
bollard's `codegen/proto/resources/` as reference. This is deferred to P2 (after HTTP API surface
is complete).

## What is BuildKit

BuildKit is Docker's next-generation build system. Unlike the classic `docker build` (which uses
the HTTP API), BuildKit communicates over **gRPC** on a Unix socket. It supports:
- Efficient layer caching
- Parallel build execution
- SSH agent forwarding for private git deps
- Secret management (no secrets in image layers)
- Multi-platform builds
- OCI image export

## BuildKit gRPC services

From bollard's proto files, the BuildKit control API exposes:

| Service | Method | Purpose |
|---------|--------|---------|
| **ControlClient** | `Solve` | Execute a build |
| | `Status` | Stream build status/progress |
| | `DiskUsage` | Report cache usage |
| | `Prune` | Clean build cache |
| | `Info` | BuildKit server info |
| **FileSend** | `Send` | Send build context files |
| **AuthProvider** | `Credentials` | Registry credentials |
| **SecretProvider** | `GetSecret` | Build secrets |
| **SshProvider** | `CheckAgent` | SSH agent forwarding |

## Why foundation_connectrpc

`foundation_connectrpc` implements the Connect protocol (Connect, gRPC, gRPC-Web) with:
- `ProtoCodec` for protobuf message encoding
- `H1Transport` / `H2Transport` for HTTP transport
- `Client` with `ClientStream`, `ServerStream`, `BidiStream` support
- Unix socket transport via custom `Transport` implementation

This maps directly to BuildKit's gRPC interface without needing `tonic`.

## Implementation approach

### Phase 1: Proto code generation

```
foundation_deployment_docker/codegen/
  buildkit/
    moby/buildkit/v1/control.proto    # Copied from bollard
    moby/buildkit/v1/secrets.proto
    ...
  gen.rs                               # Build script using foundation_connectrpc_codegen
```

Use `foundation_connectrpc_codegen` to generate Rust types from the proto files.

### Phase 2: ConnectRPC client

```rust
// BuildKit control client
pub struct BuildKitClient {
    client: Client,  // foundation_connectrpc::Client
}

impl BuildKitClient {
    pub fn connect_unix(socket: &Path) -> Result<Self, BuildKitError> {
        // Create Unix socket transport → connect to buildkitd
    }

    pub async fn solve(&self, req: SolveRequest) -> Result<SolveResponse, ConnectError> {
        // gRPC unary call
    }

    pub fn status(&self, req: StatusRequest) -> impl Stream<Item = StatusResponse> {
        // gRPC server streaming
    }
}
```

### Phase 3: Build API integration

Wire BuildKit into the Docker image build flow:

```rust
// When buildkit feature enabled, docker.build_image() uses BuildKit
fn build_image_with_buildkit(
    docker: &DockerClient,
    buildkit: &BuildKitClient,
    options: BuildImageOptions,
    context: TarContext,
) -> impl TaskIterator<Ready = Result<BuildInfo, DockerApiError>, ...> {
    // 1. Upload context to BuildKit via FileSend
    // 2. Call Solve with build options
    // 3. Stream status updates
    // 4. Return build info
}
```

## Deferral rationale

BuildKit is P2 because:
1. The testbed use case primarily pulls existing images, not builds new ones
2. The HTTP API surface (containers, images, networks, volumes) is the priority
3. `foundation_connectrpc` is still maturing — we want it stable before building on it
4. BuildKit adds significant proto dependency surface (~10 proto files)

## Proto file sources

Copy from bollard's `codegen/proto/resources/`:

```
moby/
  buildkit/
    v1/
      control.proto              # Main build control API
      secrets.proto              # Secret management
      ssh.proto                  # SSH forwarding
      types/
        worker.proto             # Worker types
      sourcepolicy/
        policy.proto             # Source policies
  filesync/
    v1/
      auth.proto                 # Registry auth
      filesync.proto             # File sync
      upload.proto               # Upload provider
grpc/
  health/
    v1/
      health.proto               # Health checks
pb/
  ops.proto                      # Build operations
```
