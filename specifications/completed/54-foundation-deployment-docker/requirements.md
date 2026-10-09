# Requirements — Spec 54: foundation_deployment_docker

## Summary

Implement `foundation_deployment_docker` — a Docker Engine API provider that implements the same
`Deployable` trait surface as other providers in `foundation_deployment`, but communicates with
Docker via our own `DynNetClient` + `PreparedRequestBuilder` + valtron async tasks instead of depending on bollard.

All crates are enhancement and if we need to break existing compatibility, we own the code we can make it better as necessary.

Bollard: /home/darkvoid/Boxxed/@formulas/src.rust/src.Containers/src.fussybeaver/bollard/

## Why

Spec 53 (docker-container-testbed) learned that bollard is deeply intertwined with tokio — its
transport layer (hyper), streaming decoders, and BuildKit gRPC all assume a tokio reactor. Our
platform uses valtron for async orchestration, and pulling in bollard's 55+ transitive tokio/hyper
deps creates runtime conflicts and executor mismatches.

## Approach

Generate the initial API surface using `gen_api` (the `foundation_codegentools` binary), which
wraps `foundation_openapi::UnifiedGenerator`. Workflow:

1. **Vendored spec** → `artefacts/cloud_providers/docker/docker-engine-v1.53.json`
2. **Analyze** → `gen_api analyze --provider docker --spec artefacts/.../docker-engine-v1.53.json`
   (shows endpoint groups, type counts, shared resources)
3. **Generate** → `gen_api generate --provider docker --output-dir backends/foundation_deployment_docker/src`
   (produces `shared/`, `containers/`, `images/`, `networks/`, `volumes/`, `system/`, `exec/` modules
   with types from `$ref` schemas, `{OperationId}Args` structs, and `{op}_request()` functions
   returning `impl TaskIterator<Ready = Result<ApiResponse<T>, ApiError>>`)
4. **Refine** → hand-write what the generator can't do: `DockerClient` (Unix socket setup, auth,
   version negotiation), streaming endpoints (logs/events via `SendSafeBodyBytesIterator` +
   `StreamIteratorExt`), `Deployable` impls, `LogFrameDecoder` (~50 lines for Docker's 8-byte
   multiplexed log frames)

The generator handles ~90% of the HTTP CRUD surface. We hand-write the Docker-specific glue.

For the BuildKit RPC API, implement on top of `foundation_connectrpc` using bollard's protobuf
definitions as a guide.

## Target API surface (from bollard 0.21 / Docker API v1.53)

| Domain | Priority | Endpoints |
|--------|----------|-----------|
| **Containers** | P0 | create, start, stop, remove, inspect, logs, exec, wait |
| **Images** | P0 | pull, push, inspect, list, remove, build |
| **Networks** | P1 | create, remove, inspect, list, connect, disconnect |
| **Volumes** | P1 | create, remove, inspect, list |
| **System** | P1 | version, info, ping, events, df |
| **Exec** | P0 | create, start, inspect (part of containers) |
| **BuildKit** | P2 | build via gRPC (registry, export, secrets, sshforward) |
| **Swarm/Services** | P3 | defer — not needed for testbed use case |
| **Plugins/Secrets/Configs** | P3 | defer — not needed for testbed use case |

## Transport targets

- **Unix socket** (`/var/run/docker.sock`) — primary
- **TCP/HTTP** (`DOCKER_HOST=tcp://...`) — secondary
- **SSH** (`ssh://user@host`) — deferred (needs `foundation_sshkit`)
- **TLS** (`DOCKER_CERT_PATH` with client certs) — deferred

## Key decisions

See decisions directory for detailed analysis:

- **[01 — No bollard dependency](decisions/01-no-bollard-dependency.md)** — Why we replicate types instead of depending on bollard
- **[02 — Docker OpenAPI spec source](decisions/02-docker-openapi-spec.md)** — Where we get the official Docker Engine API spec
- **[03 — Type replication strategy](decisions/03-type-replication-strategy.md)** — How we replicate bollard's generated types
- **[04 — HTTP via SimpleHttpClient](decisions/04-http-via-simple-http-client.md)** — HTTP client strategy
- **[05 — Valtron TaskIterator format](decisions/05-valtron-task-iterator.md)** — Async execution pattern
- **[06 — BuildKit via foundation_connectrpc](decisions/06-buildkit-connectrpc.md)** — RPC/BuildKit strategy
- **[07 — Unix socket transport](decisions/07-unix-socket-transport.md)** — How we talk to the Docker socket
- **[08 — Streaming response handling](decisions/08-streaming-responses.md)** — Log/events stream decoding
