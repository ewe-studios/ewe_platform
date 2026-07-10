# Requirements — Spec 54: foundation_deployment_docker

## Summary

Implement `foundation_deployment_docker` — a Docker Engine API provider that implements the same
`Deployable` trait surface as other providers in `foundation_deployment`, but communicates with
Docker via our own `SimpleHttpClient` + valtron task format instead of depending on bollard.

## Why

Spec 53 (docker-container-testbed) learned that bollard is deeply intertwined with tokio — its
transport layer (hyper), streaming decoders, and BuildKit gRPC all assume a tokio reactor. Our
platform uses valtron for async orchestration, and pulling in bollard's 55+ transitive tokio/hyper
deps creates runtime conflicts and executor mismatches.

## Approach

Use `foundation_openapi::UnifiedGenerator` to generate the initial API surface from the Docker
Engine OpenAPI spec, then hand-refine where needed. This gives us proper types + TaskIterator
client functions from the spec, with bollard's source as reference for Docker-specific quirks.

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
