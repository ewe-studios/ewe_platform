---
description: "Implement a WASM-native HTTP client in foundation_wasm that provides reqwest-like functionality (Client, RequestBuilder, Response, Body) by leveraging wasm-bindgen + web_sys + js_sys as an optional feature layer. Bridges the browser's Fetch API into Rust, enabling foundation_wasm modules to make HTTP requests without needing a native TCP stack."
status: "pending"
priority: "high"
created: 2026-05-18
author: "Main Agent"
metadata:
  version: "1.0"
  last_updated: 2026-05-18
  estimated_effort: "large"
  tags:
    - wasm
    - http-client
    - fetch-api
    - wasm-bindgen
    - web-sys
    - foundation_wasm
  skills:
    - rust-clean-code
  tools:
    - Rust
    - cargo
    - wasm-pack
has_features: true
has_fundamentals: false
builds_on: "specifications/28-cloudflare-workers-readiness"
related_specs:
  - "specifications/02-build-http-client"
  - "specifications/28-cloudflare-workers-readiness"
  - "specifications/21-http-framework"
features:
  completed: 0
  uncompleted: 9
  total: 9
  completion_percentage: 0%
---

# WASM HTTP Client Specification

## Overview

This specification adds a WASM-native HTTP client to `foundation_wasm` at `backends/foundation_wasm/src/fetch/`. It provides a reqwest-like public API (`Client`, `ClientBuilder`, `RequestBuilder`, `Request`, `Response`, `Body`) by wrapping the browser's `fetch()` API through `wasm-bindgen` + `web_sys` + `js_sys`.

**Why this is needed:** foundation_wasm's existing custom FFI ABI (`#[link(wasm_import_module = "abi")]`) provides a binary protocol for encoding/decoding values, memory management, batch instructions, callback registries, and host function invocation. It has ZERO HTTP/fetch functionality. The ABI cannot handle streaming responses, `FormData` construction, or direct browser API access. A separate layer on top of wasm-bindgen is required to bridge the browser's Fetch API into idiomatic Rust.

**Scope:** WASM32-only implementation. All code is `#[cfg(target_arch = "wasm32")]` gated with non-wasm stub modules. Gated behind an `http` Cargo feature flag.

**Out of scope:** Native (non-wasm) HTTP (already in foundation_core), proxy/TLS/connection pooling (browser handles all of that), WebSocket, SSE (separate specs).

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Browser Environment                              │
│                                                                         │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │  foundation_wasm/src/fetch/ (#[cfg(target_arch = "wasm32")])      │  │
│  │                                                                   │  │
│  │  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────────┐  │  │
│  │  │   Client     │─▶│ClientBuilder │  │  Config (Arc)           │  │  │
│  │  │  (public)   │  │              │  │  default_headers        │  │  │
│  │  │             │  │              │  │  error (deferred)       │  │  │
│  │  └──────┬──────┘  └──────────────┘  └─────────────────────────┘  │  │
│  │         │                                                         │  │
│  │         ▼                                                         │  │
│  │  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────────┐  │  │
│  │  │RequestBuilder│─▶│   Request    │  │  method, url, headers   │  │  │
│  │  │ (deferred)  │  │              │  │  body, timeout, cors    │  │  │
│  │  └──────┬──────┘  └──────┬───────┘  │  credentials, cache     │  │  │
│  │         │                │           └─────────────────────────┘  │  │
│  │         │                ▼                                        │  │
│  │         │         ┌──────────────┐                                │  │
│  │         │         │  fetch()     │  Core bridge function          │  │
│  │         │         │  bridge      │  Request → web_sys::RequestInit│  │
│  │         │         │              │  → browser fetch() → Response  │  │
│  │         │         └──────┬───────┘                                │  │
│  │         │                │                                        │  │
│  │         ▼                ▼                                        │  │
│  │  ┌─────────────────────────────┐  ┌────────────────────────────┐  │  │
│  │  │        Response             │  │        AbortGuard           │  │  │
│  │  │  http::Response<web_sys>   │  │  AbortController + setTimeout│  │  │
│  │  │  text(), bytes(), json()   │  │  RAII: aborts on drop       │  │  │
│  │  │  error_for_status()        │  │  per-request timeout support │  │  │
│  │  └─────────────────────────────┘  └────────────────────────────┘  │  │
│  │                                                                   │  │
│  │  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────────┐  │  │
│  │  │    Body     │  │   multipart  │  │     (streaming)         │  │  │
│  │  │ Bytes/Text  │  │ Form + Part  │  │  wasm_streams           │  │  │
│  │  │ JsValue conv│  │ FormData     │  │  ReadableStream→Stream  │  │  │
│  │  └─────────────┘  └──────────────┘  └─────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                            │                                            │
│                            ▼                                            │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │  wasm-bindgen / web_sys / js_sys / wasm-bindgen-futures           │  │
│  │  (optional deps, gated behind `http` feature)                     │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                            │                                            │
│                            ▼                                            │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │  Browser Fetch API / AbortController / setTimeout / FormData      │  │
│  └───────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘

Non-wasm stubs (#[cfg(not(target_arch = "wasm32"))]):
  - Empty module re-exports with compile_error! or no-op types
  - Allows native code to compile without pulling in wasm deps
```

### Layer Breakdown

| Layer | Module | Purpose |
|-------|--------|---------|
| Public API | `mod.rs` | Re-exports Client, RequestBuilder, Response, Body |
| Client | `client.rs` | Client, ClientBuilder, Config, convenience methods |
| Request | `request.rs` | Request struct, RequestBuilder with deferred errors |
| Fetch Bridge | `client.rs` (internal) | fetch() function, RequestInit construction, AbortGuard |
| Response | `response.rs` | Response wrapping http::Response<web_sys::Response> |
| Body | `body.rs` | Body type with Bytes/Text, From impls, JS conversion |
| Multipart | `multipart.rs` | Form, Part, FormData conversion (feature-gated) |
| Streaming | `response.rs` | bytes_stream() via wasm_streams (feature-gated) |
| Errors | `error.rs` | wasm, builder, request, decode, TimedOut error variants |

### Key Design Decisions

1. **Module location:** `backends/foundation_wasm/src/fetch/` (not `http/` to avoid confusion with `http` crate and foundation_http). The public re-export path is `foundation_wasm::fetch`.

2. **No `std` dependency:** foundation_wasm is `#![no_std]`. The fetch module uses `alloc` crate only. `wasm-bindgen` and its ecosystem provide the necessary runtime support without `std`.

3. **Error integration:** New `FetchError` type in the existing error.rs (or separate error module) that adds to `WASMErrors` enum, keeping compatibility with the existing error hierarchy.

4. **`http` crate dependency:** Uses `http` crate for `Method`, `StatusCode`, `HeaderMap`, `HeaderName`, `HeaderValue`. foundation_core does not currently depend on `http`, so this is a new direct dependency for foundation_wasm (gated behind `http` feature).

5. **Promise bridging:** Replicates reqwest's `promise<T>` helper using `wasm_bindgen_futures::JsFuture` to convert JS Promises into Rust async futures.

6. **No async runtime needed:** `JsFuture` bridges JS Promises directly. Works in any async context that supports wasm-bindgen-futures (browser event loop).

## Known Issues/Limitations

1. **Browser-only:** This implementation only works when compiled for `wasm32-unknown-unknown` and running in a browser or ServiceWorker. Node.js WASM would need different bindings.

2. **No connection pooling:** The browser manages connections. No control over keep-alive, pooling, or connection reuse from Rust side.

3. **No custom TLS/proxy:** Browser handles all SSL/TLS. Proxy configuration is not available through fetch API.

4. **No streaming request body:** Fetch API does not support streaming uploads. Request body must be fully materialized (Bytes, Text, or FormData).

5. **Timeout via setTimeout:** Per-request timeout uses JS `setTimeout` calling `AbortController.abort()`. This is approximate (JS timer granularity) and fires on the event loop, not at exact wall-clock time.

6. **No redirect control:** Fetch API's redirect behavior is limited to "follow", "error", "manual". Cannot implement custom redirect logic or redirect counting.

7. **Response body consumed once:** Like reqwest, Response body can only be consumed once (text, bytes, json, or bytes_stream — pick one).

## Feature Index

### Pending Features (0/9 completed)

1. **[dependency-foundation](./features/01-dependency-foundation/feature.md)** — Cargo.toml setup, `http` feature flag, wasm-bindgen/js-sys/web-sys/wasm-bindgen-futures optional deps, non-wasm stub module, `http` crate dependency.

2. **[core-types](./features/02-core-types/feature.md)** — Body type (Single enum: Bytes/Text), JS value conversion (Uint8Array, JsString), From impls for common types, error types (FetchError with wasm/builder/request/decode/TimedOut variants).

3. **[request-layer](./features/03-request-layer/feature.md)** — Request struct (method, url, headers, body, timeout, cors, credentials, cache), RequestBuilder with deferred errors, header management, auth helpers (basic_auth, bearer_auth), fetch mode/credentials/cache configuration.

4. **[fetch-bridge](./features/04-fetch-bridge/feature.md)** — Core fetch() function: RequestInit construction, header conversion, AbortGuard (RAII AbortController + setTimeout), ServiceWorkerGlobalScope detection, promise resolution, Response wrapping, timeout detection, error mapping.

5. **[response-layer](./features/05-response-layer/feature.md)** — Response struct wrapping http::Response<web_sys::Response>, body consumption (text, bytes, json), error_for_status, content_length, url access, Debug impl.

6. **[streaming](./features/06-streaming/feature.md)** — Response body streaming via wasm_streams ReadableStream, bytes_stream() returning impl Stream<Item = Result<Bytes>>, AbortGuard kept alive via stream lifetime. (Feature-gated: `stream`)

7. **[multipart](./features/07-multipart/feature.md)** — Form, Part, FormParts, PartMetadata, conversion to web_sys::FormData, Blob creation, file name support, MIME type handling. (Feature-gated: `multipart`)

8. **[client-layer](./features/08-client-layer/feature.md)** — Client with Arc<Config>, ClientBuilder, default headers, user_agent, convenience methods (get/post/put/patch/delete/head), execute/send, header merge logic.

9. **[integration-testing](./features/09-integration-testing/feature.md)** — wasm32-unknown-unknown compilation verification, wasm-bindgen-test integration tests, examples demonstrating usage, integration with foundation_wasm's existing async callback system.

---

## Feature Dependencies

```
01-dependency-foundation (base)
    |
    v
02-core-types
    |
    +---------+---------+---------+
    |         |         |         |
    v         v         v         v
03-request  04-fetch   05-response 07-multipart
    |       (needs 02)  (needs 04) (needs 02)
    |         |         |         |
    +----+----+         |         |
         |              |         |
         v              v         v
      08-client ────────┼─────────┤
                        |         |
                        v         |
                     06-streaming │
                        |        |
                        +----+---+
                             |
                             v
                     09-integration-testing
```

Dependency chain: 01 → 02 → {03, 04, 05, 07} → 08 → {06, 09}
- 03 (request) and 04 (fetch-bridge) can be built in parallel after 02
- 05 (response) depends on 04 (fetch-bridge) for AbortGuard
- 08 (client) depends on 03 (request) and 04 (fetch-bridge)
- 06 (streaming) and 07 (multipart) are optional feature-gated additions
- 09 (integration) depends on everything being functional

---

## Success Criteria (Spec-Wide)

### Compilation
- [ ] `cargo build --target wasm32-unknown-unknown --features http` succeeds for foundation_wasm
- [ ] `cargo build --target wasm32-unknown-unknown --features http,streaming,multipart` succeeds
- [ ] `cargo build` (native target) succeeds without wasm deps
- [ ] `cargo clippy --target wasm32-unknown-unknown --features http -- -D warnings` passes

### Functionality
- [ ] Client can make GET/POST/PUT/DELETE/PATCH/HEAD requests
- [ ] Request headers, body, timeout, auth are correctly applied
- [ ] Response status, headers, body (text/bytes/json) are correctly retrieved
- [ ] Timeout triggers AbortController and returns TimedOut error
- [ ] error_for_status correctly identifies client/server errors
- [ ] Default headers from Client are merged with per-request headers
- [ ] [streaming] bytes_stream() yields body chunks correctly
- [ ] [multipart] Form with text/binary parts converts to FormData correctly

### Code Quality
- [ ] All public types have documentation comments
- [ ] Non-wasm stubs provide compile_error! with clear message
- [ ] Error types implement Display + Error traits
- [ ] No panics in normal operation (expect_throw only for invariant violations)
- [ ] Memory: AbortGuard correctly cleans up AbortController + setTimeout

---

## Module References

- **Implementation:** `backends/foundation_wasm/src/fetch/`
- **Cargo.toml:** `backends/foundation_wasm/Cargo.toml`
- **Error types:** `backends/foundation_wasm/src/error.rs` (extended)
- **Inspiration:** reqwest wasm implementation at `/home/darkvoid/Boxxed/@formulas/src.rust/src.wasm/reqwest/src/wasm/`
- **Builds on:** spec 28 (wasm-bindgen additions to foundation_wasm Cargo.toml)

---

_Created: 2026-05-18_
_Structure: Feature-based (has_features: true)_
