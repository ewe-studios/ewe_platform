---
feature: "Docker API type replication via gen_api code generation"
description: "Vendor Docker Engine API spec v1.53 from local moby checkout; run gen_api generate to produce types, args structs, and async fn client functions; hand-refine streaming endpoints and Unix socket wiring"
status: "in-progress"
priority: "high"
phase: 1
depends_on: ["01-dynnetclient-codegen-alignment", "02-unix-socket-transport"]
estimated_effort: "large"
created: 2026-07-12
---
# Feature 03: Docker API type replication via `gen_api`

## Why

The Docker Engine API v1.53 has ~200 types in its schemas and ~80+ endpoints
across containers, images, networks, volumes, system, and exec domains.
Hand-writing all of them is tedious, error-prone, and duplicates work the spec
already encodes.

`foundation_openai`'s `UnifiedGenerator` (wrapped by `gen_api`) already produces
this surface for Cloudflare, Stripe, Supabase, and others. Docker follows the
same pattern: generate first, refine second.

## What to do

### 1. Vendor the spec

Copy `/home/darkvoid/Boxxed/@formulas/src.rust/src.Containers/src.moby/moby/api/docs/v1.53.yaml`
into `foundation_deployment_docker/specs/docker-engine-v1.53.yaml`.

### 2. Analyze the spec

```bash
cargo run --bin ewe_platform gen_api analyze --provider docker \
  --spec backends/foundation_deployment_docker/specs/docker-engine-v1.53.yaml
```

Expected output: grouped endpoints (containers, images, networks, volumes,
system, exec), shared resource types, type counts.

### 3. Generate initial code

```bash
cargo run --bin ewe_platform gen_api generate --provider docker \
  --output-dir backends/foundation_deployment_docker/src
```

Produces:
```
foundation_deployment_docker/src/
  generated/
    shared/mod.rs          # ApiError, ApiResponse, shared resource types
    containers/mod.rs      # Container types + async fn client functions
    images/mod.rs          # Image types + async fn client functions
    networks/mod.rs        # Network types + async fn client functions
    volumes/mod.rs         # Volume types + async fn client functions
    system/mod.rs          # System types (version, info, ping, events, df)
    exec/mod.rs            # Exec types (create, start, inspect)
```

The generator emits `async fn` with `DynNetClient` + `PreparedRequestBuilder`
(per Feature 01 / Decision 05). Non-streaming endpoints use
`body_reader::collect_exchange()`. Streaming endpoints return
`split_exchange()` tuples.

### 4. Hand-refine

The generator handles ~90% of the CRUD surface. Hand-write what it can't do:

| Area | Generated? | Refinement |
|------|-----------|------------|
| Type structs | Yes | Fix `#[serde(flatten)] HashMap` fallbacks with proper fields (see bollard models) |
| Args structs | Yes | Add convenience constructors where needed |
| Non-streaming `*_request()` functions | Yes | Verify status code handling, body JSON parsing |
| Streaming endpoints (logs, events, stats) | No — hand-written | Use `split_exchange()` pattern, attach `LogFrameDecoder` |
| `DockerClient` struct | No — hand-written | Unix socket setup, auth, version negotiation |
| `Deployable` impls | No — hand-written | Container lifecycle: create → start → wait healthy |
| BuildKit stub | No — hand-written | Feature 05 covers this separately |

### 5. Streaming endpoint hand-writing

Endpoints that stream: logs, events, stats, pull progress, build output.

Each follows the `split_exchange()` pattern (Decision 08):
```rust
pub fn container_logs(
    client: DynNetClient,
    container_id: &str,
    follow: bool,
) -> (
    CollectorStreamIterator<Result<(Status, SimpleHeaders), Arc<dyn Error + Send + Sync>>, ()>,
    CollectorStreamIterator<Result<Bytes, Arc<dyn Error + Send + Sync>>, ()>,
    Box<dyn TaskIterator<...> + Send>,
) {
    let builder = PreparedRequestBuilder::get(&url)?
        .query("stdout", Some("1"))
        .query("stderr", Some("1"))
        .query("follow", Some(if follow { "1" } else { "0" }));
    body_reader::split_exchange(client, builder)
}
```

The `LogFrameDecoder` (~50 lines) handles Docker's 8-byte multiplexed log format.

### 6. `DockerClient` hand-writing

```rust
pub struct DockerClient {
    http: DynNetClient,
    base_url: String,
    api_version: String,
}

impl DockerClient {
    pub fn connect_unix(socket_path: impl Into<PathBuf>) -> Result<Self, DockerError> {
        let http = HttpClientBuilder::new()
            .unix_socket(socket_path)
            .build();
        Ok(Self { http, base_url: "http://localhost".into(), api_version: "v1.53".into() })
    }

    pub fn connect_with_defaults() -> Result<Self, DockerError> {
        Self::connect_unix(resolve_docker_socket())
    }

    pub async fn negotiate_version(&mut self) -> Result<(), DockerError> {
        // GET /version → parse ApiVersion → min(client, server)
    }
}
```

## Target API surface (from bollard / Docker API v1.53)

| Domain | Priority | Endpoints |
|--------|----------|-----------|
| **Containers** | P0 | create, start, stop, remove, inspect, logs, exec, wait |
| **Images** | P0 | pull, push, inspect, list, remove, build |
| **Exec** | P0 | create, start, inspect |
| **Networks** | P1 | create, remove, inspect, list, connect, disconnect |
| **Volumes** | P1 | create, remove, inspect, list |
| **System** | P1 | version, info, ping, events, df |
| **BuildKit** | P2 | build via connectrpc (Feature 05) |
| **Swarm/Services** | P3 | defer — not needed for testbed |

## Scope

| Crate | Change |
|-------|--------|
| `foundation_deployment_docker` | Vendored spec, generated types, hand-refined streaming + client |

## Verification

- `cargo check -p foundation_deployment_docker` — compiles
- All P0 endpoint functions type-check (containers, images, exec)
- Streaming endpoint functions type-check (logs, events)
- `gen_api generate` output is committed and reviewable in git diff
- `DockerClient::connect_with_defaults()` compiles and resolves socket path

## Acceptance criteria

1. All ~200 Docker API types generated and compiling with correct serde derives
2. All ~80 endpoint functions generated as `async fn`
3. Streaming endpoints (logs, events, stats) hand-written using `split_exchange()`
4. `DockerClient` struct compiles with Unix socket + version negotiation
5. `Deployable` trait implementation compiles for containers
