---
description: "Implement a WASM-native HTTP client in foundation_wasm that provides reqwest-like functionality (Client, RequestBuilder, Response, Body, streaming, multipart) entirely through foundation_wasm's existing binary ABI and megatron.js JS runtime, with no wasm-bindgen dependency. Also adds a native-only asset module for megatron.js access."
status: "pending"
priority: "high"
created: 2026-05-18
author: "Main Agent"
metadata:
  version: "2.0"
  estimated_effort: "large"
  tags:
    - wasm
    - http-client
    - fetch
    - foundation_wasm
    - megatron.js
    - binary-abi
    - external-reference
    - asset-module
  skills:
    - rust-clean-code
  tools:
    - Rust
    - cargo
has_features: true
has_fundamentals: false
builds_on: "specifications/28-cloudflare-workers-readiness"
related_specs:
  - "specifications/02-build-http-client"
  - "specifications/28-cloudflare-workers-readiness"
features:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---

# WASM HTTP Client Specification

## Overview

This specification defines the implementation of an HTTP client inside `foundation_wasm` that provides a reqwest-like API surface (`Client`, `ClientBuilder`, `RequestBuilder`, `Response`, `Body`, multipart, streaming) while running entirely through foundation_wasm's existing binary ABI — no wasm-bindgen, no web_sys, no js_sys.

The HTTP client bridges Rust → browser fetch() by:

1. **Megatron.js** executing JS fetch logic that reads request params from shared memory and writes responses back
2. **`ExternalPointer`** (type ID 16) holding live JS object references (`Response`, `AbortController`, `FormData`) — the same mechanism already used for `DOM_WINDOW`, `DOM_DOCUMENT`, `DOM_BODY`
3. **`MemoryId`** transfers for request/response bodies through shared memory allocations
4. **Async callbacks** (`host_invoke_async_function` → `invoke_callback`) for response delivery and streaming chunk delivery
5. **Enhanced FFI surface** — new parameter/return types in megatron.js and new Rust-side host function wrappers to support the data flows needed

Additionally, this spec adds a native-only `asset.rs` module (gated `#[cfg(not(target_arch = "wasm32"))]`) that exposes `megatron.js` via `include_str!` for build-time access, and updates `Cargo.toml` to package the JS runtime as a published asset.

## Architecture

### High-Level Layering

```
┌─────────────────────────────────────────────────────────────────┐
│                        Rust WASM Binary                         │
│                                                                 │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │   Client     │  │ ClientBuilder│  │     Config (defaults) │  │
│  │  execute()   │→ │  Request     │  │     User-Agent        │  │
│  │  send()      │  │  Builder     │  │     Default Headers   │  │
│  └──────┬───────┘  └──────┬───────┘  └───────────────────────┘  │
│         │                 │                                     │
│  ┌──────▼─────────────────▼───────────────────────────────┐     │
│  │                  Fetch Bridge (mod.rs)                   │     │
│  │  - AbortGuard (RAII: AbortController ExternalPointer)   │     │
│  │  - setTimeout/clearTimeout scheduling via ABI           │     │
│  │  - promise<T> helper for async callback resolution       │     │
│  │  - ServiceWorkerGlobalScope detection                    │     │
│  └──────────────────────┬──────────────────────────────────┘     │
│                         │                                        │
│  ┌──────────────────────▼──────────────────────────────────┐    │
│  │                   Request Layer                          │    │
│  │  Request: method, url, headers, body, timeout, cors,     │    │
│  │            credentials, cache                            │    │
│  │  RequestBuilder: deferred errors, query/form/json/builder │    │
│  └──────────────────────┬──────────────────────────────────┘    │
│                         │                                        │
│  ┌──────────────────────▼──────────────────────────────────┐    │
│  │                   Body Types                             │    │
│  │  Body → Inner::Single(Bytes | Text) | MultipartForm      │    │
│  │  JS value conversion: Uint8Array, JsString, FormData     │    │
│  └──────────────────────┬──────────────────────────────────┘    │
│                         │                                        │
├─────────────────────────┼────────────────────────────────────────┤
│              ABI BOUNDARY (shared memory + ExternalPointer)      │
├─────────────────────────┼────────────────────────────────────────┤
│                         │                                        │
│  ┌──────────────────────▼──────────────────────────────────┐    │
│  │              megatron.js (JS Runtime)                    │    │
│  │                                                          │    │
│  │  ┌────────────────────────────────────────────────────┐  │    │
│  │  │  Fetch Executor (NEW)                               │  │    │
│  │  │  1. Read params from shared memory (MemoryId)       │  │    │
│  │  │  2. Construct RequestInit (method, headers, body)   │  │    │
│  │  │  3. Create AbortController, wire timeout            │  │    │
│  │  │  4. Call browser fetch(url, init)                   │  │    │
│  │  │  5. On response: write body to shared memory        │  │    │
│  │  │  6. Store Response as ExternalPointer               │  │    │
│  │  │  7. Invoke Rust callback with result MemoryId       │  │    │
│  │  └────────────────────────────────────────────────────┘  │    │
│  │                                                          │    │
│  │  ┌────────────────────────────────────────────────────┐  │    │
│  │  │  Response Helpers (NEW)                             │  │    │
│  │  │  - readResponseBody(response_uid) → MemoryId        │  │    │
│  │  │  - getResponseHeaders(response_uid) → JSON string   │  │    │
│  │  │  - getResponseStatus(response_uid) → u16            │  │    │
│  │  │  - getResponseUrl(response_uid) → string            │  │    │
│  │  │  - abortResponse(response_uid)                      │  │    │
│  │  └────────────────────────────────────────────────────┘  │    │
│  │                                                          │    │
│  │  ┌────────────────────────────────────────────────────┐  │    │
│  │  │  Multipart Helpers (NEW)                            │  │    │
│  │  │  - createFormData() → ExternalPointer               │  │    │
│  │  │  - appendToFormData(fd_uid, name, body_memory)      │  │    │
│  │  │  │    optional: filename, mime                      │  │    │
│  │  │  - formDataToBody(fd_uid) → JS Body for fetch       │  │    │
│  │  └────────────────────────────────────────────────────┘  │    │
│  │                                                          │    │
│  │  ┌────────────────────────────────────────────────────┐  │    │
│  │  │  Streaming (NEW)                                    │  │    │
│  │  │  - streamResponseBody(response_uid, callback_ptr)   │  │    │
│  │  │    reads ReadableStream, writes chunks to mem,       │  │    │
│  │  │    invokes callback per chunk with MemoryId          │  │    │
│  │  └────────────────────────────────────────────────────┘  │    │
│  │                                                          │    │
│  │  Existing: function_heap, MemoryAllocations,             │    │
│  │            AsyncTaskCollector, ExternalReference mgmt    │    │
│  └──────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
```

### ABI Data Flow for a Single Request

```
Rust                                          JS (megatron.js)
 │                                                   │
 │ 1. Build Request                                  │
 │    (method, url, headers, body)                   │
 │                                                   │
 │ 2. Write body bytes to shared memory              │
 │    → get MemoryId                                 │
 │                                                   │
 │ 3. Encode params into ops buffer                  │
 │    [url(str), method(str), headers(json),         │
 │     body(MemoryId), timeout(u64), cors(bool)]     │
 │                                                   │
 │─── host_invoke_async_function ───────────────────►│
 │    (fetch_handler, callback_ptr, ops, ops_size)   │
 │                                       4. Parse params from ops
 │                                       5. Read body from MemoryId
 │                                       6. Create AbortController
 │                                       7. Set setTimeout if timeout > 0
 │                                       8. Call fetch(url, init)
 │                                                  │
 │                    9. await response              │
 │                    10. response.arrayBuffer()     │
 │                    11. Write body to shared mem   │
 │                    12. Store Response as ExtRef   │
 │                    13. Write result header:       │
 │                        [status, headers_json,    │
 │                         body_MemoryId, url,      │
 │                         response_uid(ExtRef)]     │
 │                                                   │
 │◄── invoke_callback(callback_ptr, result_mem) ────│
 │                                       14. Parse result
 │                                       15. Build Response struct
 │                                       16. Return to caller
 │
 │ 17. Response.text() / .bytes() / .json()
 │     → already consumed, data in MemoryId
 │     (or for streaming: repeated callbacks)
```

### ABI Data Flow for Streaming Response

```
Rust                                          JS (megatron.js)
 │                                                   │
 │ 1. Response.bytes_stream()                        │
 │    → register async chunk callback                │
 │    → get InternalPointer                          │
 │                                                   │
 │─── host_invoke_async_function ───────────────────►│
 │    (stream_reader, chunk_callback, response_uid)  │
 │                                       2. Get ReadableStream
 │                                          from response
 │                                       3. reader = stream.getReader()
 │                                       4. loop:                      │
 │                                          chunk = reader.read()     │
 │                                          if done: break             │
 │                                          write chunk to shared mem  │
 │                                          get MemoryId               │
 │─── invoke_callback(chunk_cb, [MemoryId, done]) ──│                 │
 │◄─────────────────────────────────────────────────│                 │
 │ 5. Process chunk, yield Bytes                    │                 │
 │ 6. Return to stream consumer                     │                 │
 │    (repeat until done=true)                      │                 │
 │                                       7. release() reader         │
 │─── invoke_callback(chunk_cb, [done=true]) ───────│                 │
 │                                                   │
```

### Why Not wasm-bindgen?

foundation_wasm is intentionally dependency-free. The existing ABI + megatron.js already provides:

- **Live JS object references** via `ExternalPointer` / `ExternalReference` (type ID 16) — already used for `DOM_WINDOW`, `DOM_DOCUMENT`, `DOM_BODY`
- **Binary data transfer** via `MemoryId` + shared memory (`TypedSlice`, `Uint8ArrayBuffer` — type IDs 18-27)
- **Async invocation** via `host_invoke_async_function` → `AsyncTaskCollector`
- **Typed returns** via `ReturnTypeId` (32 types including `Object`, `DOMObject`, `ExternalReference`)
- **Function registration** via `register_function` / `host_invoke_function`

Adding wasm-bindgen would duplicate the FFI surface and pull in 4+ dependencies (wasm-bindgen, js-sys, web-sys, wasm-bindgen-futures). The megatron.js runtime already has the plumbing — we just need to add the fetch executor logic and extend the ABI where the current type system needs enrichment.

### Existing ABI Capabilities Used

| Requirement | ABI Mechanism | Existing? |
|---|---|---|
| Hold live JS objects (AbortController, Response) | `ExternalPointer` / `ExternalReference` (type ID 16) | Yes |
| Transfer request body bytes | `MemoryId` + `TypedSlice` / `Uint8ArrayBuffer` | Yes |
| Transfer response body bytes | `MemoryId` + `Uint8ArrayBuffer` (type ID 18) | Yes |
| Async response delivery | `host_invoke_async_function` + `invoke_callback` | Yes |
| Per-request timeout | `schedule_timeout` / `unschedule_timeout` | Yes |
| Function registration | `host_register_function` + `host_invoke_function` | Yes |
| String transfer (URLs, headers) | `Text8` / `StrLocation` / `CachedText` | Yes |
| Error codes | `ErrorCode` (type ID 31) | Yes |
| JS object type detection | `ExternalReference` with heap inspection | Yes |

### Known Issues / Limitations

1. **No direct `web_sys` access** — Response methods (`text()`, `json()`, `arrayBuffer()`) must be called via megatron.js, not typed Rust bindings. Return data comes through shared memory.
2. **Streaming overhead** — Each chunk requires a memory write + callback invocation across the ABI boundary. Acceptable for typical HTTP responses but higher overhead than reqwest's direct `wasm_streams` integration.
3. **Multipart FormData** — Must be constructed in JS via megatron.js helper functions; cannot use `web_sys::FormData` directly.
4. **No HTTP/2 in browser fetch** — Browser `fetch()` handles protocol negotiation; we don't control ALPN.
5. **No connection pooling** — Browser manages connections; we cannot pool or reuse.
6. **No proxy/TLS config** — Browser handles all transport-level concerns.
7. **Binary ABI extension needed** — Current `ReturnTypeId` has no dedicated `JsObject` type beyond `ExternalReference`. We may want to add a typed `JsObject` return variant or extend `ExternalReference` with a type tag to distinguish function references from data objects.

## Feature Index

### Pending Features (0/10 completed)

1. **[abi-http-bridge](./features/01-abi-http-bridge/feature.md)** — Core fetch executor in megatron.js, Rust-side bridge module (`src/http/mod.rs`), AbortGuard via ExternalPointer, promise helper, ServiceWorkerGlobalScope detection
2. **[js-fetch-runtime](./features/02-js-fetch-runtime/feature.md)** — megatron.js fetch executor: request construction, header parsing, response delivery, abort handling, timeout, response helpers
3. **[error-types](./features/03-error-types/feature.md)** — HTTP-specific error types (HttpError, TimedOut, DecodeError, BuilderError) extending WASMErrors
4. **[body-types](./features/04-body-types/feature.md)** — Body type with Single (Bytes/Text) enum, JS value conversion via MemoryId, From impls for common types
5. **[request-layer](./features/05-request-layer/feature.md)** — Request struct, RequestBuilder with deferred errors, auth helpers, fetch mode/credentials/cache configuration
6. **[response-layer](./features/06-response-layer/feature.md)** — Response struct, body consumption (text/bytes/json) via MemoryId, error_for_status, header access, URL tracking
7. **[streaming](./features/07-streaming/feature.md)** — Response body streaming via repeated async callbacks with chunk MemoryIds, Stream iterator pattern
8. **[multipart](./features/08-multipart/feature.md)** — Form/Part types, FormData construction in JS, Blob creation, filename/mime support
9. **[client-layer](./features/09-client-layer/feature.md)** — Client with Arc<Config>, ClientBuilder, default headers, convenience methods (get/post/put/patch/delete/head), execute
10. **[asset-module](./features/10-asset-module/feature.md)** — Native-only `asset.rs` with `include_str!(megatron.js)`, Cargo.toml include for publishing, `#[cfg(not(target_arch = "wasm32"))]` gating

## Feature Dependencies

```
10-asset-module (independent)
    |
03-error-types (independent)
    |
04-body-types ────────────────────────┐
    |                                  │
02-js-fetch-runtime ◄── 01-abi-http-bridge
    |                                  │
    +──────────┬───────────┬───────────┤
               │           │           │
               v           v           v
         05-request   06-response   08-multipart
               │           │           │
               v           v           │
            07-streaming ◄─┘           │
               │                       │
               └───────────┬───────────┘
                           │
                           v
                    09-client-layer
```

## Success Criteria (Spec-Wide)

### Compilation
- [ ] `cargo build --target wasm32-unknown-unknown --features http` succeeds for foundation_wasm
- [ ] `cargo build --target wasm32-unknown-unknown --features http,http-streaming` succeeds
- [ ] `cargo build --target wasm32-unknown-unknown --features http,http-multipart` succeeds
- [ ] `cargo build --target wasm32-unknown-unknown --features http,http-streaming,http-multipart` succeeds
- [ ] `cargo build` succeeds for foundation_wasm on native target (no regressions)
- [ ] `asset.rs` is only compiled on non-wasm targets (`#[cfg(not(target_arch = "wasm32"))]`)

### Functionality
- [ ] Client can execute GET/POST/PUT/PATCH/DELETE/HEAD requests
- [ ] Request body (bytes, text, JSON, form-urlencoded) sent correctly
- [ ] Response body consumed as text, bytes, and JSON
- [ ] Default headers applied to all requests
- [ ] Per-request timeout triggers abort
- [ ] AbortGuard cancels in-flight requests on drop
- [ ] Streaming response yields chunks via async stream
- [ ] Multipart form with text and file parts sends correctly
- [ ] Response status code and headers accessible
- [ ] error_for_status returns error for 4xx/5xx

### Code Quality
- [ ] `cargo clippy --target wasm32-unknown-unknown --features http -- -D warnings` passes
- [ ] `cargo clippy -- -D warnings` passes on native target
- [ ] `cargo fmt -- --check` passes
- [ ] No unsafe code without documented justification
- [ ] No TODO/FIXME/stubs left in completed features

### Asset Module
- [ ] `megatron.js` accessible via `foundation_wasm::assets::MEGATRON_JS` on native
- [ ] `megatron.js` NOT included in wasm32 binary (verified via binary size)
- [ ] `Cargo.toml` `include` field lists `sdk/jsruntime/megatron.js` for publishing

---

## Prerequisites

- foundation_wasm binary ABI understanding (base.rs, ops.rs, jsapi.rs)
- megatron.js runtime understanding (sdk/jsruntime/megatron.js)
- `http` crate for Method, StatusCode, HeaderMap (add as optional dependency)

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | HTTP client implementation, ABI integration | `.agents/skills/rust-clean-code/skill.md` |
| JavaScript | megatron.js fetch executor, response helpers | No specific skill — follow existing megatron.js patterns |

---

_Created: 2026-05-18_
_Structure: Feature-based (has_features: true)_
