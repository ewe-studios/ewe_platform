---
feature: "Wire Module Restructure — Deep Client Split"
description: "Split all foundation_core::wire modules into shared (wasm-compatible) and native (socket/TLS-dependent) submodules with deep client split"
status: "implemented"
priority: "high"
depends_on: ["01-core-wasm-compat"]
estimated_effort: "large"
created: 2026-05-16
last_updated: 2026-05-16
author: "Main Agent"
tasks:
  completed: 12
  uncompleted: 0
  total: 12
  completion_percentage: 100%
---

# Feature: Wire Module Restructure — Deep Client Split

## Overview

The initial pass gated entire `simple_http/`, `event_source/`, `websocket/`, and `http_stream/` behind `not(target_arch = "wasm32")`. This is too coarse — most files are pure logic. We need to extract every shared type so wasm32 code can use them without pulling in `RawStream`, `HttpConnectionPool`, or `SSLConnector`.

**Goal:** Split every wire module into shared (always compiled) and native (gated) submodules.

## Feature Flag

```toml
wire-native = ["std"]
default = ["standard", "std", "ssl", "compression", "wire-native"]
```

**Gate pattern:** `#[cfg(not(target_arch = "wasm32"))]` on native-only modules.

---

## Module 1: simple_http (top level)

### Shared (always compiled)

| File | Public types | Native types? |
|------|-------------|--------------|
| `errors.rs` | `HttpReaderError`, `ChunkStateError`, `Http11RenderError`, `SimpleHttpError`, etc. | No |
| `impls.rs` | `Http11`, `RenderHttp`, `HttpSendResponseReader`, `HttpRequestReader`, `IncomingResponseMapper`, `SendSafeBody` (enum variants), `SimpleIncomingRequest`, `SimpleIncomingRequestBuilder`, `Status`, `Proto`, `ChunkedData`, `LineFeed`, etc. | No — but imports `ContentLengthEnforcingIterator`, `Extensions` from shared |
| `client_classifier.rs` | `ClientClassifier` | No |
| `latency_tracker.rs` | `LatencyTracker` | No |
| `load_tracker.rs` | `LoadTracker` | No |
| `sse.rs` | `SseParser` | No |
| `timeout.rs` | `TimeoutCalculator`, `TimeoutConfig`, `TimeoutContext` | No — imports `ClientClassifier` (shared) |
| `shared.rs` | `Extensions`, `ContentLengthEnforcingIterator` | No |

### simple_http/client — deep split

**Shared client files (zero native type references):**

| File | Public types | Notes |
|------|-------------|-------|
| `dns.rs` | `DnsResolver`, `SystemDnsResolver`, `MockDnsResolver` | Pure trait + mock impl |
| `cookie.rs` | `Cookie`, `CookieJar`, `CookieStore` | Pure RFC 6265 cookie logic |
| `control.rs` | `RequestControlSignal` | Atomic state coordination |
| `compression.rs` | `ContentEncoding`, `DecompressingReader`, `CompressionConfig` | Pure decompression logic |
| `intro.rs` | `ResponseIntro` | Wraps `Status`/`Proto` |
| `middleware.rs` | `Middleware`, `MiddlewareChain` | Pure trait + onion-chain execution |
| `redirects.rs` | `resolve_redirect`, `follow_redirect_chain` | Pure URL resolution |

**Split client files (shared types extracted, native parts gated):**

| File | Shared types | Native types |
|------|-------------|-------------|
| `request.rs` | `PreparedRequest` | `ClientRequestBuilder<R>` (holds `Arc<HttpConnectionPool<R>>`) |
| `client.rs` | `ClientConfig` | `SimpleHttpClient<R>` (holds `Arc<HttpConnectionPool<R>>`) |
| `proxy.rs` | `ProxyProtocol`, `ProxyAuth`, `ProxyConfig` | `HttpConnectionPool` impl blocks (CONNECT tunnel) |
| `api.rs` | None (type aliases) | `ClientRequestState`, `FinalizedResponse`, `ClientRequest<R>` (all reference `HttpConnectionPool`) |

**Native client files (entirely gated):**

| File | Native types used | Reason |
|------|-------------------|--------|
| `connection.rs` | `RawStream`, `Connection`, `SSLConnector` | `HttpClientConnection` wraps raw socket |
| `pool.rs` | `RawStream`, `SharedByteBufferStream<RawStream>` | Stores live socket connections |
| `tls_task.rs` | `Connection`, `RawStream`, `SSLConnector` | TLS handshake over raw sockets |
| `tasks/` (all 5 files) | `RawStream`, `HttpClientConnection`, `HttpConnectionPool` | Read/write from sockets |

---

## Module 2: event_source

### Shared (always compiled)

| File | Public types | Notes |
|------|-------------|-------|
| `core.rs` | `Event`, `SseEvent`, `SseEventBuilder`, `ParseResult` | Pure types, zero imports |
| `parser.rs` | `SseParser` | Pure SSE parsing logic, zero imports |
| `error.rs` | `EventSourceError`, `SseParseError` | Pure error types, only `std::fmt` |
| `response.rs` | `SseResponse` | Builds HTTP SSE responses — imports simple_http pure types only |
| `writer.rs` | `EventWriter` | Writes SSE events to `std::io::Write` |

### Native (gated behind `not(target_arch = "wasm32")`)

| File | Native types used | Reason |
|------|-------------------|--------|
| `consumer.rs` | `HttpConnectionPool`, `EventSourceTask`, `ReconnectingEventSourceTask` | Uses connection pool and native task for streaming |
| `task.rs` | `RawStream`, `HttpClientConnection`, `HttpConnectionPool`, `HttpResponseReader<_, RawStream>` | Establishes HTTP connections via pool, reads from socket |
| `reconnecting_task.rs` | `DnsResolver`, depends on `EventSourceTask` | Wraps native task with reconnection logic |

---

## Module 3: websocket

### Shared (always compiled)

| File | Public types | Notes |
|------|-------------|-------|
| `assembler.rs` | `MessageAssembler` | Pure message assembly from frames |
| `batch_writer.rs` | `BatchFrameWriter` | Pure frame encoding/batching |
| `error.rs` | `WebSocketError` | Pure error types |
| `frame.rs` | `WebSocketFrame`, `Opcode`, `generate_mask` | Pure frame encoding/decoding |
| `handshake.rs` | `build_upgrade_request`, `compute_accept_key`, `generate_websocket_key` | Pure HTTP handshake logic |
| `message.rs` | `WebSocketMessage` | Pure message enum |

### Native (gated behind `not(target_arch = "wasm32")`)

| File | Native types used | Reason |
|------|-------------------|--------|
| `connection.rs` | `RawStream`, `SharedByteBufferStream<RawStream>` | Wraps raw socket for read/write |
| `task.rs` | `RawStream`, `HttpClientConnection`, `HttpConnectionPool`, `HttpResponseReader<_, RawStream>` | Establishes connection via pool, reads frames from socket |
| `reconnecting_task.rs` | `DnsResolver`, depends on `WebSocketTask` | Wraps native task with reconnection logic |
| `server.rs` | `RawStream`, `SharedByteBufferStream<RawStream>` | Server-side WebSocket connection over raw socket |

---

## Module 4: http_stream (entirely native)

| File | Native types used |
|------|-------------------|
| `mod.rs` | `RawStream`, `ClientEndpoint`, `DataStreamError`, `ReconnectingStream` |

---

## Implementation

### Directory Structure

```
wire/
├── mod.rs                      # #[cfg(not(wasm32))] gate on all wire submodules
├── simple_http/
│   ├── mod.rs                  # declares shared + gated client
│   ├── errors.rs               # always compiled
│   ├── impls.rs                # always compiled
│   ├── client_classifier.rs    # always compiled
│   ├── latency_tracker.rs      # always compiled
│   ├── load_tracker.rs         # always compiled
│   ├── sse.rs                  # always compiled
│   ├── timeout.rs              # always compiled
│   ├── shared.rs               # Extensions, ContentLengthEnforcingIterator
│   └── client/
│       ├── mod.rs              # re-exports shared + gated native
│       ├── shared/
│       │   ├── mod.rs          # re-exports all shared client types
│       │   ├── dns.rs          # DnsResolver, SystemDnsResolver
│       │   ├── cookie.rs       # Cookie, CookieJar
│       │   ├── control.rs      # RequestControlSignal
│       │   ├── compression.rs  # ContentEncoding, DecompressingReader
│       │   ├── intro.rs        # ResponseIntro
│       │   ├── middleware.rs   # Middleware, MiddlewareChain
│       │   ├── redirects.rs    # redirect resolution
│       │   ├── request.rs      # PreparedRequest
│       │   ├── config.rs       # ClientConfig (extracted from client.rs)
│       │   └── proxy.rs        # ProxyConfig, ProxyProtocol, ProxyAuth
│       └── native/             #[cfg(not(wasm32))]
│           ├── mod.rs          # re-exports all native client types
│           ├── connection.rs   # HttpClientConnection
│           ├── pool.rs         # ConnectionPool, HttpConnectionPool
│           ├── tls_task.rs     # TlsUpgradeTask
│           ├── client.rs       # SimpleHttpClient
│           ├── request.rs      # ClientRequestBuilder
│           ├── api.rs          # ClientRequest, FinalizedResponse
│           ├── proxy.rs        # HttpConnectionPool proxy impls
│           └── tasks/
│               ├── mod.rs
│               ├── request_intro.rs
│               ├── request_redirect.rs
│               ├── request_stream.rs
│               ├── send_request.rs
│               └── state.rs
├── event_source/
│   ├── mod.rs                  # shared + gated native
│   ├── core.rs                 # always compiled
│   ├── parser.rs               # always compiled
│   ├── error.rs                # always compiled
│   ├── response.rs             # always compiled
│   ├── writer.rs               # always compiled
│   ├── consumer.rs             #[cfg(not(wasm32))]
│   ├── task.rs                 #[cfg(not(wasm32))]
│   └── reconnecting_task.rs    #[cfg(not(wasm32))]
├── websocket/
│   ├── mod.rs                  # shared + gated native
│   ├── assembler.rs            # always compiled
│   ├── batch_writer.rs         # always compiled
│   ├── error.rs                # always compiled
│   ├── frame.rs                # always compiled
│   ├── handshake.rs            # always compiled
│   ├── message.rs              # always compiled
│   ├── connection.rs           #[cfg(not(wasm32))]
│   ├── task.rs                 #[cfg(not(wasm32))]
│   ├── reconnecting_task.rs    #[cfg(not(wasm32))]
│   └── server.rs               #[cfg(not(wasm32))]
└── http_stream/
    └── mod.rs                  #[cfg(not(wasm32))] — entirely native
```

### Split Details

**`client/request.rs` → `shared/request.rs` + `native/request.rs`:**
- Shared: `PreparedRequest` struct + `into_simple_incoming_request()`
- Native: `ClientRequestBuilder<R>` + impl blocks with `Arc<HttpConnectionPool<R>>`

**`client/client.rs` → `shared/config.rs` + `native/client.rs`:**
- Shared: `ClientConfig` struct (timeouts, headers, proxy, max body size)
- Native: `SimpleHttpClient<R>` + all impl blocks using `HttpConnectionPool`

**`client/proxy.rs` → `shared/proxy.rs` + `native/proxy.rs`:**
- Shared: `ProxyProtocol`, `ProxyAuth`, `ProxyConfig` + `parse()`, `from_env()`
- Native: `impl<R> HttpConnectionPool<R>` CONNECT tunnel methods

### `client/mod.rs` — conditional re-exports

```rust
pub mod shared;
pub use shared::*;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
```

### Step-by-Step Tasks

1. [ ] Create `client/shared/`, move 7 pure files (dns, cookie, control, compression, intro, middleware, redirects)
2. [ ] Extract `PreparedRequest` from `request.rs` → `client/shared/request.rs`
3. [ ] Extract `ClientConfig` from `client.rs` → `client/shared/config.rs`
4. [ ] Extract `ProxyConfig`/`ProxyProtocol`/`ProxyAuth` from `proxy.rs` → `client/shared/proxy.rs`
5. [ ] Create `client/native/`, move 3 pure native files (connection, pool, tls_task)
6. [ ] Move native parts: `ClientRequestBuilder` → `native/request.rs`, `SimpleHttpClient` → `native/client.rs`, `HttpConnectionPool` proxy impls → `native/proxy.rs`
7. [ ] Move `api.rs` → `native/api.rs`, move `tasks/` → `native/tasks/`
8. [ ] Set up `client/mod.rs` with conditional re-exports
9. [ ] Gate `event_source/consumer.rs`, `event_source/task.rs`, `event_source/reconnecting_task.rs`
10. [ ] Gate `websocket/connection.rs`, `task.rs`, `reconnecting_task.rs`, `server.rs`
11. [ ] Verify native: `cargo check -p foundation_core`
12. [ ] Verify wasm32: `cargo check -p foundation_core --target wasm32-unknown-unknown`

## What wasm32 gains access to

| Type | Module | Purpose |
|------|--------|---------|
| `DnsResolver` | client/shared | Custom DNS resolution trait |
| `MockDnsResolver` | client/shared | Testing |
| `Cookie` / `CookieJar` | client/shared | Cookie parsing/storage |
| `ClientConfig` | client/shared | HTTP client configuration |
| `PreparedRequest` | client/shared | HTTP request data |
| `ProxyConfig` / `ProxyProtocol` | client/shared | Proxy configuration |
| `ContentEncoding` / `DecompressingReader` | client/shared | Compression handling |
| `Middleware` / `MiddlewareChain` | client/shared | Middleware patterns |
| `RequestControlSignal` | client/shared | Request control |
| `ResponseIntro` | client/shared | Response parsing |
| `Extensions` | simple_http/shared | Type-safe extension storage |
| `Event` / `SseParser` | event_source | SSE types and parser |
| `WebSocketFrame` / `Opcode` | websocket | Frame encoding/decoding |
| `WebSocketMessage` | websocket | Message enum |
| `MessageAssembler` | websocket | Frame assembly |
| `generate_mask` / `compute_accept_key` | websocket | Handshake utilities |

---

_Created: 2026-05-16_
