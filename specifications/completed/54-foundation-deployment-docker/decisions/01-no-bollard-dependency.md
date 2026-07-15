# 01 — No Bollard Dependency

**Date:** 2026-07-10
**Status:** Resolved

## Decision

Do **not** depend on `bollard` as a library. Instead, replicate the Docker Engine API types
(input/output structs) ourselves — using bollard's source as a reference — and implement all
HTTP API calls via `foundation_netio::DynNetClient` and `PreparedRequestBuilder`.

## Why

Bollard is deeply intertwined with tokio in ways that conflict with our valtron-based async
model:

### 1. Hyper transport dependency

Bollard's HTTP layer is built on `hyper 1.x`, which requires tokio as its executor. Every API
call runs through `hyper_util::client::legacy::Client<TokioExecutor>`. This means:
- Bollard **requires** a tokio runtime to be running when its futures are polled
- Our valtron executor manages threads synchronously — there is no global tokio reactor
- Running bollard under valtron would require spawning a separate tokio runtime per call
  (the `block_on` pattern we used in spec 53), which works but is fragile and adds overhead

### 2. Streaming response decoders

Bollard's streaming (`process_into_stream`, `process_into_stream_string`) uses `tokio-util` codecs
(`FramedRead`, `JsonLineDecoder`, `NewlineLogOutputDecoder`). These depend on `tokio::io::AsyncRead`
which again assumes tokio. We'd either need to replicate these decoders in our own types or wrap
tokio's async I/O.

### 3. BuildKit gRPC (tonic)

When the `buildkit` feature is enabled, bollard pulls in `tonic 0.14` for BuildKit protobuf RPC.
Tonic is even more deeply tokio-bound than hyper — its channel implementation is `tonic::transport::Channel`
which is a tokio-native type.

### 4. Dependency weight

Bollard adds ~55 transitive dependencies. Our platform already has its own HTTP client
(`DynNetClient`), TLS connector (`SSLConnector`), and streaming infrastructure. Pulling
bollard means pulling duplicate functionality:
- `hyper` vs our `DynNetClient` (different HTTP client stacks)
- `tokio` (97 lockfile entries already, bollard adds more feature requirements)
- `tower-service`, `tokio-util`, `tokio-stream` (we don't use tower)

### 5. Type generation mismatch

Bollard generates its types via swagger-codegen (Maven/Java) into `bollard-stubs`. The generated
types use patterns like `Option<Box<T>>` for nullable fields and builder types for every options
struct. These don't match our manual type style (used in cloudflare provider), and we can't
customize the generator without forking the swagger-codegen templates.

## What we do instead

1. **Parse the Docker Engine OpenAPI spec** using `foundation_openapi::SpecProcessor` to extract
   endpoint metadata and type definitions
2. **Hand-write the input/output structs** that we need (P0 first: containers, images, exec, system)
   guided by bollard's `models` and `query_parameters` modules as reference
3. **Implement HTTP endpoints** as `async fn` using `DynNetClient`
4. **Implement streaming decoders** ourselves (Docker's 8-byte multiplexed log format, JSON-line
   streams) using `SharedByteBufferStream` which our platform already uses
5. **Implement BuildKit RPC** later via `foundation_connectrpc` using bollard's protobuf files
   as reference (`codegen/proto/resources/*.proto`)

## Related decisions

- **[02 — Docker OpenAPI spec source](02-docker-openapi-spec.md)** — Where we get the spec
- **[03 — Type replication strategy](03-type-replication-strategy.md)** — How we write the types
- **[04 — HTTP via DynNetClient + PreparedRequestBuilder](04-http-via-simple-http-client.md)** — HTTP client pattern
