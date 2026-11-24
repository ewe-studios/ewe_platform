# What is currently available

This document maps the platform primitives that already exist in the `foundation_*`
crates. It answers three questions for each capability: does it work on native, on
`wasm32` in a browser, and on a Cloudflare Worker; what does it do; and where is the code.
Use it before you write a plan. It shows which gap needs a small change to an existing
type, and which gap has no type at all.

_Last updated: 2026-11-15. Status: all eleven crate groups are measured; a fresh agent is
checking every claim against the code before the plan review starts._

## How to read this document

Each of sections 1 to 11 has the same shape:

1. A summary table with three columns: `Platform`, `What's supported` and `What's not
   supported`.
2. Detail tables with the symbol and the file and line. A path is relative to the crate
   root.
3. A list of explicit absences. A missing capability is stated, not implied.

Section 12 turns the map into decisions: what to reuse, what not to build, and which claims
in the current documents the code contradicts.

A row with "none" means the crate has no code for that item. It does not mean the platform
cannot do it.

## Abbreviations

- `wasm32`: the 32-bit WebAssembly target, `wasm32-unknown-unknown` unless stated.
- Cloudflare Worker (Worker): a Cloudflare program that runs on the `workerd` runtime.
- Durable Object (DO): a Cloudflare object with one instance and one storage namespace.
- Server-sent events (SSE): a one-way event stream over HTTP.
- Virtual file system (VFS): a file interface that hides the host file system.
- Deterministic simulation testing (DST): a test that replays a run from a seed.

## Platforms in this document

| Platform | What it is |
|---|---|
| native | A normal operating system process: Linux, macOS or Windows. |
| wasm32 (browser) | A module in a browser page or a service worker. It has `fetch`, `WebSocket` and timers. |
| Cloudflare Worker | A `wasm32` module on `workerd`. It has `fetch`, D1, KV, R2 and DO stubs. It has no `window` and no `WebSocket` constructor. |

---

## 1. `foundation_netio`

`foundation_netio` owns the HTTP client, the server-sent events parser, the WebSocket
connector and the request and response body types.

### 1.1 Summary

| Platform | What's supported | What's not supported |
|---|---|---|
| native | Full HTTP/1.1 and HTTP/2 client with a connection pool, TLS, proxies, DNS, timeouts, retries, body limits and streaming in both directions. WebSocket client and server. SSE with reconnection. QUIC and HTTP/3 behind the `quic` feature. | Nothing in the list on the right; the native gaps are feature gaps, not platform gaps. |
| wasm32 (browser) | `fetch`-based HTTP client. Streamed request bodies. Streamed SSE responses. WebSocket through `web_sys::WebSocket`. A pull-based streaming response body for a served reply. | Whole response bodies only on `send_async`; most `ClientConfig` limits are ignored; no SSE reconnection; no WebSocket reconnect; no headers on the SSE path. |
| Cloudflare Worker | Same code as the browser, because the crate has no Cloudflare branch. `fetch` works. D1, KV and R2 live in `foundation_db`. | No Worker detection: with no `ServiceWorkerGlobalScope` and no `window`, the client panics. No `WebSocket` constructor. No DO stub client (`StubHttpClient` does not exist). |
| Anything else | — | No Cloudflare feature and no `worker` crate dependency anywhere in the crate. |

### 1.2 The HTTP client

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| The client trait | yes | yes | yes | `HttpClient: Send + Sync`, `src/shared/client/http_client.rs:65` | Five methods: `send_async`, `send_sse_async`, `send`, `send_sse`, `open_exchange` (lines 67, 73, 83, 90, 92). |
| One handle for HTTP and WebSocket | yes | yes | yes | `NetClient`, `src/network_client.rs:37`; `DynNetClient = Arc<dyn NetClient>`, `src/network_client.rs:41` | The handle is the platform-neutral client. |
| Choose the implementation per target | yes | yes | yes | `HttpClientBuilder::build()`, `src/network_client.rs:440` | Native at line 441; wasm at line 448. The wasm branch needs the `wasm-fetch` feature. |
| A small response | buffered or streamed | whole body only | whole body only | `send_async`, `src/wasm/client/client.rs:114`; body read at `:334` | The body is `Response.text()`. Binary data is decoded as text at `:340`. |
| A streamed response | yes | **no** | **no** | same lines | This is the main gap; see issue #557. |
| A streamed request body | yes | yes | yes | `send_safe_body_to_js`, `src/wasm/client/client.rs:387`; `Stream` at `:402` | `SendSafeBody::Stream`, `ChunkedStream`, `LineFeedStream` and `SseStream` become a JS `ReadableStream`. |
| Synchronous request | yes | error | error | `send`, `src/wasm/client/client.rs:150` | The error says a sync call would block the isolate. |
| SSE | yes, with reconnect | one-shot | one-shot | `send_sse_async`, `src/wasm/client/client.rs:125`; `fetch_sse` at `:180` | The wasm path needs a 2xx status. |
| A lazy SSE stream | yes | yes | yes | `send_sse`, `src/wasm/client/client.rs:159`; `LazySseIterator` at `:216` | The fetch starts on the first poll (spawn at `:247`). |
| An exchange task | yes | yes | yes | `open_exchange`, `src/wasm/client/client.rs:166`; `WasmHttpExchangeTask`, `src/wasm/client/tasks/http_exchange_task.rs:72` | The wasm task builds `FetchHttpClient::new()` and drops the caller's config at `:43`. It also buffers the whole body before the first chunk. |
| The fetch entry point | — | page or service worker | only the service-worker or page branch | `src/wasm/client/client.rs:503` | It checks `ServiceWorkerGlobalScope` at `:506`, then `window()`. It panics at `:513` when neither exists. |

### 1.3 Client configuration and limits

The config is `ClientConfig`, `src/shared/client/config.rs:42`, with defaults at `:339`.
The wasm client reads only `headers_to_add` (`src/wasm/client/client.rs:97`) and
`max_redirects` (`:121`, `:129`, `:160`). Every other field is native-only in practice.

| Limit | Native | wasm32 and Worker | Default | Where |
|---|---|---|---|---|
| `max_body_size` | enforced on the body iterators | not read; the whole body is buffered | none | `config.rs:50` |
| `full_body_threshold` | enforced | not read | 512 KiB | `config.rs:51` |
| `batch_size` | enforced | not read | 8192 | `config.rs:52` |
| `max_retries` | enforced | not read | 5 | `config.rs:53` |
| `frame_limits` | enforced on the native parser | not read | header 1 MiB, line 1 MiB, chunk line 4 KiB, SSE event 1 MiB | `config.rs:58`; `src/shared/http/framing.rs:56` |
| timeouts and `timeout_calculator` | enforced | not read; there is no `AbortController` | connect 10 s and others | `config.rs:45`; `src/shared/http/timeout.rs:169` |
| `max_redirects` | numeric | `0` is "manual", anything else is "follow" | 5 | `config.rs:46`; wasm map `src/wasm/client/client.rs:359` |
| `headers_to_add` | every request | `send_async` only | none | `config.rs:68`; merge at `src/wasm/client/client.rs:96` |
| `default_headers` | not consumed | not consumed | empty | `config.rs:47` |
| `proxy`, `proxy_from_env` | enforced | no-op | none | `config.rs:48`; wasm note `src/network_client.rs:388` |

### 1.4 The wasm response streaming primitives

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| A pull-based response body | native server | yes | intended | `src/wasm/response.rs:33` | One chunk per JS pull (pull closure `:43`). Nothing is buffered ahead of the reader. |
| Cancellation of that body | native server | yes | yes | `src/wasm/response.rs:66` | The `cancel` closure sets a flag and drops the source (checked also at `:54`). |
| Status and headers on that body | native server | **no** | **no** | `src/wasm/response.rs:76` | It sets only `Content-Type` and `Cache-Control: no-cache`. |
| An SSE response | native server | yes | yes | `src/wasm/response.rs:91` | The content type is `text/event-stream; charset=utf-8` (`:103`). |
| Read a JS stream into Rust | native sockets | yes | yes | `spawn_stream_reader`, `src/wasm/client/stream.rs:40`; reader at `:48`, `read()` at `:53` | `spawn_local` reads each chunk and pushes it to a queue (`:82`). |
| The queue bound | — | **unbounded** | unbounded | `src/wasm/client/stream.rs:116` | `ConcurrentQueue::unbounded`; a slow consumer grows memory. |
| Wake a parked reader | — | yes | yes | `src/wasm/client/stream.rs:140`; `src/shared/client/queue_reader.rs:224` | Register, then check (`:229`). |
| Read the queue as an iterator | — | yes | yes | `QueueSseEvents`, `src/shared/client/queue_reader.rs:90` | `WouldBlock` becomes `Stream::Wait` (`:204`); it never becomes end of stream. |
| Stop the JS reader when the Rust side drops it | — | **no** | **no** | `src/wasm/client/stream.rs:109` | No `Drop`, no `cancel()`; the reader loop continues to read. |
| The feature gate on this module | — | needs `wasm-fetch` | needs `wasm-fetch` | `src/wasm/mod.rs:8` | `response` and `client` are both `#[cfg(feature = "wasm-fetch")]`, so a wasm or Worker consumer must enable the feature. |

### 1.5 WebSocket and SSE

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| WebSocket client | yes | yes | **no** | `WebSocketConnector`, `src/websocket/shared/connector.rs:55`; wasm impl `src/websocket/wasm/http_client_connector.rs:23`; browser open `src/websocket/wasm/browser.rs:67` | The wasm path uses `web_sys::WebSocket`, which a Worker does not have. |
| Subprotocols | yes | yes | no | `src/websocket/wasm/browser.rs:71` | |
| Reconnect, custom handshake headers, Ping and Pong | yes | **no** | no | `src/websocket/wasm/browser.rs:27` | The browser owns the WebSocket life cycle. |
| Bounded inbound and outbound pipes | yes | yes, depth 64 | no | `src/websocket/wasm/browser.rs:57`, `:81`, `src/websocket/shared/client.rs:157` | Overflow drops the message. |
| SSE parser | yes | yes | yes | `SseParser`, `src/event_source/shared/sse.rs:90` | Shared code. |
| SSE writer | yes | yes | yes | `EventWriter`, `src/event_source/shared/writer.rs:35`; `SseResponse`, `src/event_source/shared/response.rs:12` | Shared code. |
| SSE reconnection and `Last-Event-ID` | yes | **no** | **no** | native config `src/event_source/native/task.rs:60`; wasm `fetch_sse` `src/wasm/client/client.rs:180` | `SseProgress::Reconnecting` is never produced on wasm (`src/shared/client/http_client.rs:34`). |

### 1.6 Absences, stated explicitly

- There is no `src/cloudflare/` directory, no `cloudflare` feature, no `worker` crate
  dependency and no `StubHttpClient` in this crate.
- There is no Cloudflare global-`fetch` branch. A Worker build reaches the `window()`
  branch and panics when `window` is absent (`src/wasm/client/client.rs:513`).
- There is no streaming HTTP response body. Only SSE is streamed on the read side.
- `JsReadableStreamIterator` is **not** a symbol in this crate. The code uses
  `web_sys::ReadableStreamDefaultReader` directly (`src/wasm/client/stream.rs:48`).
- The wasm netcap has no socket: `Stream` is an empty type (`src/wasm/netcap.rs:3`).

### 1.7 What this means for this specification

| Planned item | What already exists | Action |
|---|---|---|
| T1, a streamed response body | `WasmByteStream` is new, but its reader loop is `spawn_stream_reader` (`src/wasm/client/stream.rs:40`). | Extract the loop; add a bound and a cancel. Do not write a second loop. |
| T2, `open_exchange` over the same client | The exchange task exists and already reports `Head`, `BodyChunk` and `Failed`. | Pass `self`; the task then reads a stream. |
| T3, a streamed served response | `src/wasm/response.rs` already builds a pull-based `Response` with a cancel. | Extend it with a status and headers; do not add a second builder. |
| T5, a DO stub client | No `StubHttpClient`. | New code, and it is the only new client type. |

---

## 2. `foundation_http`

`foundation_http` owns the router, the middleware chain, the native server and the wasm
serving stack: the `CfServe` and `WebServe` traits, the connection types and the bridges.

### 2.1 Summary

| Platform | What's supported | What's not supported |
|---|---|---|
| native | `HttpServer` over TCP with HTTP/1.1, HTTP/2 and HTTP/3, TLS, keep-alive, graceful drain, `Serve`, `H2Serve`, `H3Serve`, `ServeWriter`, a streamed request body, a streamed response body, WebSocket upgrade, SSE, static files. | Path-parameter values are dropped before the handler runs. H2 and H3 never run middleware. `ServerConfig::max_body_bytes` is declared but never read. |
| wasm32 (browser) | `HttpApp<S>` and the router, the middleware chain, `WebServe`, `WebConn`, `WasmHttpApp`, the `web_sys::Request` bridge, CORS preflight. | The request body is read with `text()`; a response body is a `Vec<u8>`; no streaming in either direction. No `ClientIp`. No `waitUntil`. |
| Cloudflare Worker | `CfServe`, `CfConn`, `CfHttpApp` and `CfHttpAppSingleton`, the `web_sys::Request` bridge, the D1 binding at start-up. | Same as wasm, and only the `DB` binding is extracted on each request; `BUCKET` and `KV` are extracted only at start-up. |
| Automatic response streaming | — | `CfConn::body` is `Option<Vec<u8>>`. There is no `ReadableStream` anywhere in this crate's serving path. |

### 2.2 The serving types

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| App builder and router | yes | yes | yes | `HttpApp<S>`, `src/shared/app/mod.rs:23`; `middleware`, `:84` | Holds the `Ctx`, the `Router<S>` and the middleware list. |
| `Serve` | yes | no | no | `src/native/serve.rs:55` | `Send + Sync + 'static`; gets a raw connection stream. |
| `ServeWriter` | yes | yes | yes | `src/shared/serve/mod.rs:123` | Sync; writes to a `&mut dyn Write`. A wasm dispatcher for it exists (`src/wasm/serve_writer.rs:18`), but no `web_sys::Request` bridge calls it. |
| `CfServe` | no | yes | yes | `src/wasm/serve_cf.rs:16` | `#[async_trait(?Send)]`, `Sync + 'static`; `serve_cf(&self, ctx, req, conn: &mut CfConn)`. |
| `CfServeFactory` | no | yes | yes | `src/wasm/serve_cf.rs:28` | `create(&Ctx)`. |
| `WebServe` | no | yes | no | `src/shared/serve_web.rs:16` | Same shape as `CfServe`. Registration compiles everywhere; dispatch needs wasm. |
| `CfConn` | no | yes | yes | `src/wasm/cf_conn.rs:22` | `status: u16`, `headers: Vec<(String, String)>`, `body: Option<Vec<u8>>`; setters at `:39`, `:44`, `:53`, `:58`; `into_response` at `:64`. |
| `WebConn` | no | yes | no | `src/shared/web_conn.rs:22` | Same three fields and setters. |
| A JS `Response` builder | — | yes | yes | `WasmResponse`, `src/wasm/response.rs:5`; `build_response`, `:45`; `from_wasm`, `:68` | A second builder beside `CfConn::into_response` (`src/wasm/cf_conn.rs:64`). Both buffer a `Vec<u8>`. `build_response` and `from_wasm` need `wasm-bindgen-http` (`:33`). A plan picks one; it does not add a third. |
| A streamed response body | yes | **no** | **no** | `src/native/writer.rs:100`; wasm body built at `src/wasm/cf_conn.rs:77` | The wasm body is one `Uint8Array`. |

### 2.3 The bridges and the dispatch

| Capability | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|
| A `Request` becomes `SimpleIncomingRequest` | yes | yes | `request_from_web`, `src/wasm/bridge/web.rs:18`; `request_from_cf`, `src/wasm/bridge/cf.rs:20` | The body is `req.text()` awaited at `web.rs:33` and `cf.rs:35`. |
| The protocol field | `HTTP11` always | `HTTP11` always | `src/wasm/bridge/web.rs:49`; `cf.rs:51` | |
| Extensions | none | none | `src/wasm/bridge/web.rs:46`; `cf.rs:48` | No `ClientIp` and no `ConnectionContext`. |
| Bindings on each request | — | `DB` only | `extract_cf_bindings`, `src/wasm/bridge/cf.rs:61` | `BUCKET` and `KV` are read at start-up only (`:282`). |
| The singleton and its guard | `WasmHttpAppSingleton`, `WasmHttpAppGuard` | `CfHttpAppSingleton`, `CfHttpAppGuard` | `src/wasm/bridge/web.rs:144`, `:196`; `cf.rs:167`, `:223` | The guard holds an `Arc`; `unsafe impl Send` at `web.rs:140` and `cf.rs:160`. |
| The fetch entry | `fetch(&self, req)` | `fetch(&self, req, env)` | `src/wasm/bridge/web.rs:148`; `cf.rs:171` | The Worker entry gets `env`; it does not get the execution context. |
| `waitUntil` and the execution context | — | **no** | `src/wasm/bridge/cf.rs:93` | There is no `ExecutionCtx` parameter. |
| Dispatch | `dispatch_web` | `dispatch_cf` | `src/wasm/dispatch.rs:109`, `:26` | Middleware at `:48` and `:131`; router at `:77` and `:160`; handler at `:87` and `:170`. |
| Error to a JS value | `JsError` | `JsError` | `src/wasm/bridge/web.rs:87`; `cf.rs:97` | A routing failure becomes a thrown error, not a 404 response. |
| A response is buffered | yes | yes | `src/wasm/cf_conn.rs:75`; `src/shared/web_conn.rs:76` | The whole body is built before the call returns. |

### 2.4 The router

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| Static segment | yes | yes | yes | `SegmentType::Static`, `src/shared/router/segments.rs:94` | |
| `:param` segment | yes | yes | yes | `SegmentType::Param`, `src/shared/router/segments.rs:98` | The value is matched and then **dropped** (`Router::dispatch`, `src/shared/router/mod.rs:138`). A handler cannot read it. |
| Restricted segment, `:name::numbers` and others | yes | yes | yes | `SegmentType::Restricted`, `src/shared/router/segments.rs:95` | |
| Regex segment and `:name::(regex)` | yes | yes | yes | `SegmentType::ParamRegex`, `src/shared/router/segments.rs:96`; `SegmentType::Regex`, `:97` | |
| Catch-all `*` | yes | yes | yes | `AnyPath`, `src/shared/router/segments.rs:143` | |
| Method mismatch | 404 | 404 | 404 | `src/shared/router/method.rs:83` | No 405 response exists. |
| A route pattern that is not valid | panic | panic | panic | `src/shared/router/mod.rs:38` | `.expect("valid route pattern")`. |
| `route_any_*` method set | nine methods | nine | nine | `src/shared/router/segments.rs:515` | GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS, CONNECT, TRACE. |

### 2.5 Middleware and CORS

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| The middleware trait | yes | yes | yes | `RequestMiddleware`, `src/shared/middleware/mod.rs:22` | Sync; `Send + Sync + 'static`. |
| The middleware runs on | H1 | CF and Web dispatch | CF dispatch | `src/native/server/connection.rs:845`; `src/wasm/dispatch.rs:48`, `:131` | H2 and H3 skip it (`src/native/server/h2_connection.rs:472`; `h3_server.rs:253`). |
| `InterimResponse` | rendered | ignored | ignored | `src/native/server/connection.rs:857`; `src/wasm/dispatch.rs:72` | |
| CORS preflight | yes | yes | yes | `CorsMiddleware`, `src/shared/middleware/cors.rs:128` | An `OPTIONS` request is answered with 200 and the CORS headers (`:233`). |
| CORS headers on a normal response | **no** | **no** | **no** | `src/shared/middleware/cors.rs:263` | The middleware does not inject them; the handler must add them itself. |
| A `decorate` helper | **no** | no | no | none | The crate has no per-response CORS helper. |
| Body limit middleware | yes | yes | yes | `src/shared/middleware/body_limit.rs:16` | `Content-Length` only; a chunked body passes (`:76`). |
| Compression middleware | yes, but it compresses nothing | same | same | `src/shared/middleware/compression.rs:267` | It reads the preference and logs it (`:289`). |
| Request ID | yes | yes | yes | `src/shared/handlers/request_id.rs:16` | |
| Rate limit | yes | partial | partial | `src/shared/handlers/rate_limit.rs:19` | It needs a `ClientIp` extension that no wasm bridge sets. |

### 2.6 Limits and features

| Item | Native | wasm32 and Worker | Where |
|---|---|---|---|
| `ServerConfig` | yes | — | `src/native/server/mod.rs:171` |
| `max_body_bytes` | declared, default 10 MiB, **never read** | — | `src/native/server/mod.rs:177`; no read outside tests (`tests/integration_tests.rs:1106`) |
| Keep-alive idle | 75 s | — | `src/native/server/mod.rs:79` |
| Shutdown grace | 30 s | — | `src/native/server/mod.rs:182` |
| H2 limits | 256 streams, 16 KiB frame | — | `foundation_netio/src/http2/settings.rs:19` |
| wasm body limit | — | none | `src/wasm/cf_conn.rs:22`; `src/shared/web_conn.rs:22` |
| Native feature gate | `not(target_family = "wasm")` | — | `src/lib.rs:22` |
| wasm feature gate | `wasm-test` on native | `wasm-bindgen-http` | `src/lib.rs:26`; `Cargo.toml:106` |

### 2.7 Absences, stated explicitly

- No streamed request body on wasm: both bridges call `text()`.
- No streamed response body: `CfConn` and `WebConn` hold a `Vec<u8>`.
- No path-parameter value reaches a handler.
- No 405 response, no CORS decoration of a normal response, no `waitUntil`.
- No per-request R2 or KV binding.

### 2.8 What this means for this specification

| Planned item | What already exists | Action |
|---|---|---|
| T3, a streamed served response | `CfConn` is a `status`, a header list and a `Vec<u8>`; the JS `Response` is built at `src/wasm/cf_conn.rs:64`. | Add a body variant that takes the pull-based stream from `foundation_netio`'s `wasm/response.rs`; do not build a second response type. |
| T4, a streamed inbound body | The bridges call `req.text()`. | Replace the read with `resp.body()` and `WasmByteStream`; this closes issue #560. |
| A route that reads the RPC method | `/:param` values are dropped. | The handler must read the method from the request URI, or the router must expose the captures first. |
| A CORS header on an RPC reply | No helper exists. | The host adds the headers itself, or the plan adds a small helper. Do not assume `cors::decorate`.

---

## 3. `foundation_db`

`foundation_db` owns the storage traits, the Cloudflare bindings for D1, KV and R2, the
memory backends and the migration runner.

### 3.1 Summary

| Platform | What's supported | What's not supported |
|---|---|---|
| native | Turso and libsql SQL, the D1 and R2 REST clients, JSON-file storage, memory stores. One shared connection per store, serialised by a `ConnGate` with a 30 s default lock timeout. The migration runner. | No connection pool type. No Durable Object storage. No VFS: the crate uses `std::fs` directly. |
| wasm32 (no host) | The memory stores and the storage traits compile. | Every persistent backend is absent; the memory `QueryStore` methods return errors. `FileStateStore` compiles but its `std::fs` calls are unsupported. |
| Cloudflare Worker | `D1WasmStorage`, `KVWasmStorage` and `R2WasmStorage` behind `wasm-bindgen-storage`; `CfD1R2DocumentStore`; the migration runner over D1. | No Durable Object SQL storage; no `StateStore`; no atomic multi-statement batch on D1; no atomic `take` on KV; KV listing stops at the first page. |
| Feature name | `wasm-bindgen-storage` | There is no `cloudflare` feature. |

### 3.2 The storage traits

| Trait | Object-safe | Async | wasm32 | Worker | Symbol and file:line |
|---|---|---|---|---|---|
| `QueryStore` | yes | yes | memory only | D1 | `src/core/storage_provider.rs:290` (`query:304`, `execute:307`, `execute_batch:310`, `execute_batch_statements:317`) |
| `KeyValueStore` | no (generic `get`, `set`, `take`) | yes | memory | D1, KV | `src/core/storage_provider.rs:331` |
| `BlobStore` | yes | yes | memory | D1, KV, R2 | `src/core/storage_provider.rs:428` |
| `RateLimiterStore` | yes | yes | memory | D1 exact, KV best-effort | `src/core/storage_provider.rs:546` |
| `DocumentStore` | no (generic `append`, `scan`) | yes | memory only | D1 and R2 | `src/core/storage_provider.rs:637` |
| `StateStore` | — | — | **no implementation** | **no implementation** | `src/core/state/traits.rs:28` |
| `StorageProvider` | yes | yes | — | D1, KV, R2 | A struct (`src/storage_provider.rs:108`); it implements `KeyValueStore` (`:278`), `QueryStore` (`:473`), `RateLimiterStore` (`:581`) and `BlobStore` (`:724`); not `DocumentStore` or `StateStore`. |

The traits are `async_trait` with `Send` futures. On wasm the JS promise is awaited through
`SendWrapper::new(JsFuture::from(promise)).await` (`src/wasm/wasm_storage/d1_wasm.rs:22`).

### 3.3 The Cloudflare bindings and stores

| Capability | Worker | Symbol and file:line | Notes |
|---|---|---|---|
| Read a binding from `env` | yes | `get_binding::<T>(env, name)`, `src/wasm/bindgen/cf/mod.rs:14` | `Reflect::get` then `unchecked_into`. |
| D1 binding | yes | `D1Database`, `src/wasm/bindgen/cf/d1.rs:12` | `prepare`, `exec`, `bind`, `first`, `run`, `all`. No `batch()` and no Sessions. |
| D1 stores | yes | `D1WasmStorage`, `src/wasm/wasm_storage/d1_wasm.rs:36` | Implements `KeyValueStore` (`:314`), `QueryStore` (`:436`), `RateLimiterStore` (`:464`), `BlobStore` (`:618`). |
| KV binding | yes | `KVNamespace`, `src/wasm/bindgen/cf/kv.rs:12` | `get`, `put`, `delete`, `list`. Values are JSON strings. |
| KV stores | yes | `KVWasmStorage`, `src/wasm/wasm_storage/kv_wasm.rs:28` | Implements KV (`:166`), `BlobStore` (`:420`), `RateLimiterStore` (`:310`); `QueryStore` returns errors (`:207`). |
| R2 binding | yes | `R2Bucket`, `src/wasm/bindgen/cf/r2.rs:12` | `get`, `put`, `delete`, `head`, `list`. |
| R2 stores | yes | `R2WasmStorage`, `src/wasm/wasm_storage/r2_wasm.rs:23` | `BlobStore` only (`:218`); etag, range and list are overridden (`:236`, `:240`, `:253`, `:282`); streams still buffer. |
| A document store over D1 and R2 | yes | `CfD1R2DocumentStore`, `src/wasm/wasm_storage/d1r2_document_store.rs:13` | Offloads a value over 4 KiB to R2 (`src/core/backends/d1r2_document_store.rs:36`). |
| `workers-rs` interop | D1 only | `From<worker::D1Database>`, `src/wasm/workers_rs/d1.rs:12` | Needs `wasm-bindgen-bindings` and `wasm-bindgen-storage`. |
| Durable Object SQL storage | **no** | none | This is an explicit absence. |

### 3.4 The migration engine

The names are exact. There is no `MigrationSet` and no `MigrationGroup` in this crate or in
the workspace.

| Capability | Symbol and file:line | Notes |
|---|---|---|
| One migration | `Migration { id, name, sql }`, `src/core/schema/migrations.rs:12` | `'static` strings; no checksum, no down migration. |
| The ordered set | `MIGRATIONS: &[Migration]`, `src/core/schema/migrations.rs:19` | 27 entries in slice order, ids 001 to 022 and 024 to 028. |
| SQL source | `include_str!`, `src/core/schema/migrations.rs:23` | Files under `src/core/schema/sql/`. No runtime directory scan. |
| The runner | `MigrationRunner::new(&[Migration])`, `src/core/schema/migrations.rs:158` | |
| Run | `run_async(&self, store: &dyn QueryStore)`, `src/core/schema/migrations.rs:174` | Works over `D1WasmStorage`. No sync runner. |
| Applied table | `_migrations (id, name, applied_at)`, `src/core/schema/migrations.rs:176` | `SELECT 1` skip at `:184`; insert at `:208`. |
| A crate's own group | pass its own `&[Migration]` to `MigrationRunner::new` | This is the only extension point. |
| Auto-migrate | Turso and libsql only | `src/native/turso_backend.rs:140`; the wasm path creates only `{prefix}_kv` (`src/wasm/wasm_storage/d1_wasm.rs:61`). |

### 3.5 Limits, features and file access

| Item | Value or behaviour | Where |
|---|---|---|
| `ConnGate` lock timeout | 30 s default, settable; native only | `src/native/conn_gate.rs:47`, `:62` |
| D1 REST response cap | 16 MiB, settable; native only | `src/native/d1_kvstore.rs:39` |
| D1 REST error-body cap | 8 KiB, settable; native only | `src/native/d1_kvstore.rs:43` |
| Document scan limit | `limit`, `0` means no limit | `src/core/backends/sql_document_store.rs:34` |
| Blob offload threshold | 4096 bytes | `src/core/backends/d1r2_document_store.rs:36` |
| KV list | one page of 1000 keys, no cursor | `src/wasm/wasm_storage/kv_wasm.rs:120` |
| R2 list | cursor loop to the end, then filter | `src/wasm/wasm_storage/r2_wasm.rs:167` |
| Features | `wasm`, `wasm-bindgen-bindings`, `wasm-bindgen-storage`, `workers-rs`, `turso`, `libsql`, `d1`, `r2`, `multi` | `Cargo.toml:84` |
| The `d1` and `r2` features | native REST clients only; they do not enable the Worker stores | `src/native/mod.rs:13` |
| File access | `std::fs` in `src/core/state/file.rs` (15 references), `src/native/json_file.rs` and `src/native/libsql_store.rs` | `src/core/state/file.rs:10` |
| VFS use | none; `foundation_nativeapis` is not imported | repo grep returns nothing |

### 3.6 Absences, stated explicitly

- There is no Durable Object storage binding: no `storage.sql`, no DO transaction, no
  DO-alarm storage. A DO that needs SQL needs a new binding module.
- There is no `StateStore` implementation for any wasm backend. `NamespacedStore` cannot
  wrap `D1WasmStorage`.
- `PasskeyStore`, `TosStore` and `AuthStore` are declared with no implementation
  (`src/core/auth_store.rs:51`, `:72`, `:87`).
- `D1WasmStorage` does not override `execute_batch_statements`, so a parameterised batch is
  sequential and not atomic.
- `KVWasmStorage` does not override `take_is_atomic`, so `take` is read-then-delete.
- `FileStateStore` compiles on wasm and calls `std::fs`, which does not exist in a Worker.

### 3.7 What this means for this specification

Nothing in spec 85 depends on a new storage type. The specification's Worker and Durable
Object plans must use `D1WasmStorage` and the existing traits, and must state that a DO has
no SQL storage today.

---

## 4. `foundation_connectrpc`

`foundation_connectrpc` owns the RPC transport seam, the protocol codecs, the RPC router and
the client. It sends and receives RPC messages over a byte transport.

### 4.1 Summary

| Platform | What's supported | What's not supported |
|---|---|---|
| native | The `Transport` trait with `H1Transport`, `H2Transport` and `H3Transport`; `WsTransport`; the RPC router with unary, server-stream, client-stream and bidi procedures; the native server adapters `ConnectRpcServe`, `ConnectRpcServeH2` and `ConnectRpcServeH3`; every codec and compressor; the auth interceptor. | `ResponseSink` has no trailer method. The H2 and H3 servers write frames directly and do not use `ResponseSink`. |
| wasm32 (browser) | The `Transport` trait, `H1Transport` (it holds a `DynNetClient` and has no target gate), `WsTransport`, the RPC router, the client, every codec, gzip, the auth module. | No server adapter: there is no connection owner that implements `ResponseSink`. No H2 or H3 transport. A streamed response is buffered by the fetch client first. |
| Cloudflare Worker | The same library code as the browser, through `H1Transport` over a `DynNetClient`. | No Worker entry, no `worker` dependency, no `cdylib`, no `#[event(fetch)]`. A Worker needs `CfServe` from `foundation_http` to call `dispatch_to`. |
| Default features | `default = ["rpc_multi"]` | A wasm build must set `default-features = false` and choose `rpc`, or it pulls `foundation_core/multi` and its `Send` bounds. |

### 4.2 The transport seam

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| The trait | yes | yes | yes | `Transport: Send + Sync + 'static`, `src/shared/transport/base.rs:161` | Two methods: `capabilities()` at `:163` and `open(RequestDescriptor)` at `:173`. No associated types. |
| The stream a transport returns | yes | yes | yes | `TransportStream`, `src/shared/transport/base.rs:177` | `send_body: Arc<dyn SendBody>`, `head: HeadStream`, `recv_body: BodyStream`, `trailers`. |
| The request body seam | yes | yes | yes | `SendBody`, `src/shared/transport/base.rs:44` | `try_send`, `send_async`, `close`, `is_closed`. |
| The error type | yes | yes | yes | `TransportError`, `src/shared/transport/base.rs:201` | `Connect`, `Timeout`, `Reset`, `Protocol`, `Canceled`, `Io`. |
| `H1Transport` | yes | yes (not build-tested) | yes via `DynNetClient` | `src/shared/transport/h1.rs:61` | It holds only a `DynNetClient`; `Default` builds the platform client at `:52`. No target gate. |
| The H1 request body | streamed | streamed | streamed | `src/shared/transport/h1.rs:91` | `pushable_request_body_with_readiness(4)`; the depth constant is 4 (`foundation_netio/src/shared/http/pushable_body.rs:35`). |
| The H1 response | streamed | buffered by the fetch client | buffered | `src/shared/transport/h1.rs:108` | The head observer and the body observer split the exchange; the body is only as streamed as the client below it. |
| `H2Transport` | yes | **no** | **no** | `src/native/transport/h2.rs:355` | h2c only, no TLS; needs `socket2` and the reactor. |
| `H3Transport` | yes, feature `h3` | **no** | **no** | `src/native/transport/h3.rs:57` | QUIC; a TLS `ClientConfig` is mandatory. |
| `WsTransport` | yes | yes | unverified | `src/shared/transport/ws.rs:40` | It holds a `DynNetClient`; it needs a WebSocket connector, which a Worker does not have. |
| A wasm-only transport | — | **none** | **none** | `src/shared/transport/wasm.rs` | The file holds only the capability constant. |

### 4.3 The capability constant

The constant is `WASM_FETCH_CAPABILITIES`, `src/shared/transport/wasm.rs:17`. No crate code
reads it. Only `tests/transport_tests.rs:151` reads it.

| Field | Constant value | `H1Transport` value | Problem |
|---|---|---|---|
| `request_streaming` | `false` | `true` (`src/shared/transport/h1.rs:64`) | The two disagree. The code streams, so the constant is stale. |
| `full_duplex` | `false` | `false` | They agree. |
| `h2_trailers` | `false` | `false` | They agree. |
| `http_versions` | `&[]` | `&[Proto::HTTP11]` (`:67`) | An empty list fails every version check (`src/shared/transport/capabilities.rs:107`). |
| `multiplexed` | `false` | `false` | No transport reports `true`. |

### 4.4 The response sink and dispatch

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| `ResponseSink` | yes | yes | yes | `src/shared/router/dispatch.rs:375` | Bound is `Send` only. Four methods: `whole` (`:381`), `open` (`:388`), `chunk` (`:394`), `finish` (`:400`). |
| A trailer on the sink | **no** | **no** | **no** | `src/shared/router/dispatch.rs:375` | The H2 and H3 servers send trailers outside the sink. |
| A streamed RPC response | yes | library only | library only | `run_streaming`, `src/shared/router/dispatch.rs:717` | It calls `open`, then `chunk` per frame, then `finish`. |
| The sink that owns a native connection | yes | **no** | **no** | `H1Sink`, `src/native/server.rs:388` | It is the implementation a wasm or Worker host must supply. |
| A buffered sink | yes | yes | yes | `CollectingSink`, `src/shared/router/dispatch.rs:406` | `ConnectRpcHandler::dispatch` uses it (`:106`). |
| `dispatch_to` | yes | yes | yes | `src/shared/router/dispatch.rs:145` | Takes `Box<dyn ResponseSink>`. This is the call a `CfServe` adapter makes. |

### 4.5 The RPC router and the service side

The names in this table are the names that exist. There is no `ConnectService`, no
`route` and no `route_any` in this crate.

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| Register a unary procedure | yes | yes | yes | `Router::unary`, `src/shared/router/mod.rs:226` | Full path `/package.Service/Method`. |
| Server stream | yes | yes | yes | `Router::server_stream`, `src/shared/router/mod.rs:252` | |
| Client stream | yes | yes | yes | `Router::client_stream`, `src/shared/router/mod.rs:279` | |
| Bidi stream | yes | yes | yes | `Router::bidi_stream`, `src/shared/router/mod.rs:305` | |
| Freeze the router | yes | yes | yes | `Router::into_handler`, `src/shared/router/mod.rs:334` | Gives a `ConnectRpcHandler`. |
| The handler type | yes | yes | yes | `ConnectRpcHandler`, `src/shared/router/dispatch.rs:67` | `dispatch` at `:97`; `dispatch_to` at `:145`. |
| How a path is matched | exact map | exact map | exact map | `src/shared/router/mod.rs:185` | An exact `HashMap<String, HandlerEntry>` keyed by the full path. There is no `/:method` pattern. |
| A method that is not allowed | 405 and `Allow` | 405 | 405 | `src/shared/router/dispatch.rs:470` | |
| A protocol that is not supported | 415 | 415 | 415 | `src/shared/router/dispatch.rs:476` | |
| A capability that does not match | 505 | 505 | 505 | `src/shared/router/dispatch.rs:494` | |
| The native H1 adapter | yes | no | no | `ConnectRpcServe`, `src/native/server.rs:115` | Implements `foundation_http::native::serve::Serve`. |
| The native H2 adapter | yes | no | no | `ConnectRpcServeH2`, `src/native/h2_serve.rs:24` | |
| The native H3 adapter | yes, feature `h3` | no | no | `ConnectRpcServeH3`, `src/native/h3_serve.rs:65` | |
| Interceptors | yes | yes | yes | `Interceptor`, `src/shared/interceptor/mod.rs:77` | Global and per-procedure. |
| The service macro | yes | yes | yes | `register_<service>`, `foundation_macros/src/connectrpc_service.rs:939` | It calls the four typed registration methods. |

### 4.6 The client side

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| A typed client | yes | yes | yes | `Client<Req, Res>`, `src/shared/client.rs:462` | Holds `Arc<dyn Transport>`. |
| Unary | yes | yes | yes | `Client::unary`, `src/shared/client.rs:547` | |
| Server stream | yes | yes | yes | `Client::server_stream`, `src/shared/client.rs:575` | |
| Client stream | yes | yes, if the transport streams | yes | `Client::client_stream`, `src/shared/client.rs:673` | It needs `request_streaming`. |
| Bidi | yes | **no** over H1 | no | `Client::bidi_stream`, `src/shared/client.rs:772` | It needs `full_duplex`, and `H1Transport` reports `false`. |
| Codecs | Protobuf, JSON, Arrow, gzip, zstd, brotli | the same | the same | `src/shared/codec/`; `src/shared/compression/mod.rs:282` | No target gate. |
| Generated clients | yes | yes | yes | `foundation_connectrpc_codegen/src/generator.rs:397` | The constructor takes `Arc<dyn Transport>`. |

### 4.7 Absences, stated explicitly

- There is no `ConnectService` type. The service-side type is `ConnectRpcHandler`.
- There are no `route` or `route_any` methods on the RPC `Router`.
- There is no wasm server adapter and no wasm `ResponseSink` owner.
- There is no wasm-specific `Transport`. `H1Transport` is the one that serves both targets.
- There is no Worker entry point and no `worker` dependency.

### 4.8 What this means for this specification

| Planned item | What already exists | Action |
|---|---|---|
| A Worker RPC host | `ConnectRpcHandler::dispatch_to` and `ResponseSink` | T4 supplies one `CfServe` adapter that owns a `ResponseSink` over `CfConn`. |
| A mount path such as `/:method` | The RPC router matches an exact path | Mount the whole prefix as a `CfServe` and let `ConnectRpcHandler` match the procedure; do not plan a `/:method` capture. |
| T2, the capability constant | `H1Transport` already reports `request_streaming: true` | Correct the constant to `true` and give it `&[Proto::HTTP11]`. |
| A streamed RPC reply over a Worker | `run_streaming` calls `open`, `chunk` and `finish` | The sink must park on a chunk instead of returning an empty body. |

---

## 5. `foundation_core`, `foundation_compact`, `foundation_context`

These three crates are the async runtime (valtron), the target polyfills and the context.
Read this section before you write a task, a stream or a clock.

### 5.1 Summary

| Platform | What's supported | What's not supported |
|---|---|---|
| native | The single-threaded executor and the `multi` thread-pool executor; `#[task]` state tasks; stream tasks; futures streams; `block_on_future`; a `Ctx` with a clock, a config, a seeded random generator and typed entries. | — |
| wasm32 (browser) | The single-threaded executor with a JavaScript yielder; `#[task]` state tasks; stream tasks; `spawn_local`; `SendWrapper`; the clock and the entropy from `crypto.getRandomValues`. | No `multi`; no `block_on_future`; no bare `--no-default-features`; `Stream::Wait` on a build without a JS backend busy-spins. |
| Cloudflare Worker | The same as the browser with the `js-wasmbindgen` feature. `ci/packages.toml:296` names `js-wasmbindgen` as the one Worker run. | No `multi`; no `block_on_future`; the `js-foundation-wasm` and `host-abi` backends need a JS host module that a Worker does not supply. |
| Build gate | — | `multi` is a hard `compile_error!` on `wasm32` (`src/lib.rs:18`). |

### 5.2 The task forms

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| The task attribute | yes | yes | yes | `#[task]`, `foundation_macros/src/lib.rs:687`; re-export `src/valtron/mod.rs:31` | It takes no arguments. The function must be `fn name(ctx: Ctx, state: T) -> Option<(T, TaskStatus<R, P>)>`. |
| The generated type | yes | yes | yes | `Name::new(state)`, `foundation_macros/src/state_task.rs:238` | The generated struct implements `TaskIterator`. |
| A synchronous state task | yes | yes | yes | `StateTask`, `src/valtron/executors/state_task.rs:37` | |
| An async state task | yes | yes | yes | `AsyncStateTask`, `src/valtron/executors/state_task.rs:109` | The transition is `Send` only under `multi` (`:88`). |
| The transition output | yes | yes | yes | `TransitionOutput<T, R, P>`, `src/valtron/executors/state_task.rs:26` | `Option<(T, TaskStatus<R, P>)>`; `None` is the only end. |
| The status enum | yes | yes | yes | `TaskStatus`, `src/valtron/task.rs:899` | `Delayed`, `Spawn`, `Pending`, `Init`, `Ready`, `Ignore`, `Wait`, `Spread`, `Depends`. |
| The task trait | yes | yes | yes | `TaskIterator`, `src/valtron/task.rs:1086` | `next_status(&mut self, ctx) -> Option<TaskStatus<Ready, Pending>>`. |
| Run a task | yes | yes | yes | `execute`, `src/valtron/executors/non_sendables.rs:817` | No `Send` bounds outside `multi`. |
| A stream as a task | yes | yes | yes | `from_stream`, `src/valtron/executors/non_sendables.rs:556`; `StreamTask`, `src/valtron/executors/future_task.rs:366` | |
| A task as a future | yes | yes | yes | `into_ready_future`, `src/valtron/extensions/streams/non_sendable.rs:763` | Returns `Option<(D, SI)>`. |
| Collect a stream | yes | yes | yes | `into_collect_future`, `src/valtron/extensions/streams/non_sendable.rs:713` | Needs `WakeableStream` and `D: Clone`. |
| Block on a future | yes | **no** | **no** | `block_on_future`, `src/valtron/executors/non_sendables.rs:469` | Compile-excluded on a JS-hosted wasm build (`:465`). |
| A `Task` type | — | — | — | none | There is no bare `Task`; the aliases are `GlobalTask` (`src/valtron/task.rs:1353`). |

### 5.3 The stream forms

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| The stream enum | yes | yes | yes | `Stream<D, P>`, `src/valtron/streams.rs:78` | `Wait` (`:95`) means yield; with a JS backend it is about a 4 ms timer. |
| The stream trait | yes | yes | yes | `StreamIterator`, `src/valtron/streams.rs:150` | No `Send` outside `multi`. |
| A parking future stream | yes | yes | yes | `StreamParkingFutureStream`, `src/valtron/stream_future.rs:352` | Parks on `Init`, `Pending`, `Delayed` and `Wait` through `WakeableStream`. |
| A raw future stream | yes | yes | yes | `StreamAsFutureStream`, `src/valtron/stream_future.rs:310` | It never parks. |
| `into_future_stream` | yes | yes | yes | `src/valtron/extensions/streams/non_sendable.rs:779` | The parking form. |
| `into_next_stream` | yes | yes | yes | `StreamNextStream`, `src/valtron/stream_future.rs:451` | |
| A JS readable stream in Rust | no | yes | yes | `JsReadableStreamIterator`, `src/valtron/executors/wasm/wasm_bindgen/js_stream.rs:74` | Default capacity 512 (`:94`); `unsafe impl Send` at `:186`; gated to `js-wasmbindgen` (`src/valtron/executors/mod.rs:56`). |
| A Rust iterator as a JS stream | no | yes | yes | `iterator_to_readable_stream`, `src/valtron/executors/wasm/wasm_bindgen/js_stream.rs:225` | The same gate. It closes on `None`. |
| The reader data type | yes | yes | yes | `Data`, `foundation_core/src/io/readers/mod.rs:38` | Only `Bytes(Vec<u8>)` and `Retry`. |
| A retry cap | yes | yes | yes | `max_consecutive_retries`, `src/io/readers/mod.rs:96` | Default 100; over the cap the read fails (`:167`). |
| `TaskStream` and `FutureStream` | — | — | — | none | These names do not exist. |
| `IsWouldBlock` | — | — | — | none | This trait does not exist. |

### 5.4 The executor and the wake path

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| The executor | single or `multi` | single only | single only | `src/valtron/executors/mod.rs:39` | `multi` is a native thread pool. |
| The wake registry | yes | yes | yes | `WakeRegistry`, `src/valtron/wake.rs:100` | `register` at `:123`, `fire` at `:157`. |
| A wakeable stream | yes | yes | yes | `WakeableStream`, `src/valtron/wake.rs:53` | `wake`, `wake_send`, `announce`. |
| An event queue | yes | yes | yes | `EventQueue<T>`, `src/valtron/readiness_wake.rs:590` | Readiness and vacancy notifiers. |
| The JavaScript yielder | — | yes | yes | `JSThreadYielder`, `src/valtron/executors/wasm/wasm_bindgen/mod.rs:112` | It uses `CooperativeSpinWaiter`, which returns at once (`foundation_nostd/src/primitives/cooperative_spin_waiter.rs:40`). |
| The spin fallback | yes | yes | yes | `SpinWaiter`, `foundation_nostd/src/primitives/spin_waiter.rs:32` | Busy-spins, 100 000 iterations per millisecond, capped at 10 s (`:56`, `:64`). It is used only when no JS backend is selected. |
| Blocking at the boundary | yes | removed | removed | `blocking_boundary!`, `src/valtron/mod.rs:118` | It removes `block_on_future`, `run_stream_iter`, `SyncFs` and `exec_async` on a JS-hosted build. |
| An ungated blocking helper | — | present | present | `__private::block_on_future`, `src/valtron/mod.rs:174` | **Anti-pattern.** It is macro-internal, and it must not be depended on or called. A wait is a `StreamIterator` transition, not a blocking call. |

### 5.5 The test harness, on both targets

A valtron test owns the engine through an attribute macro. It never calls
`initialize_pool` by hand. The skill `.agents/skills/rust-valtron-usage/skill.md` still shows
the old `let _guard = init_pool();` shape and the `#[ntest::timeout]` and
`#[serial_test::serial]` stack. Read the macros instead; they are the current contract.

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| A test with a live engine | yes | — | — | `#[valtron_test]`, `backends/foundation_macros/src/lib.rs:675`; re-export `foundation_core::valtron::valtron_test` | It replaces `#[test]`. Do not stack `#[test]` with it. It initializes the pool and holds the guard for the body. |
| A non-test entry with a live engine | yes | — | — | `#[valtron]`, `src/lib.rs:652` | The `#[tokio::main]` analogue. |
| A wasm test with a live engine | — | yes | yes | `#[valtron_bindgen]`, `backends/foundation_macros/src/lib.rs:635`, `bindgen_test.rs:7` | It expands to `#[wasm_bindgen_test]`, initializes the pool, and runs an `async` body on the JavaScript event loop. The path is `foundation_macros::valtron_bindgen`; `foundation_core::valtron` does not re-export it. An async body must not block; `block_on_future` would starve the loop and is absent on a JS-hosted build. |
| One body, both targets | yes | yes | yes | the `dual!` macro, `backends/foundation_core/tests/valtron/async_first_api.rs:36` | The same `async fn` registers as `#[valtron_test]` on native and `#[valtron_bindgen]` on wasm. |
| Arguments | `seed`, `threads`, `timeout`, `slow_poll`, `ctx`, `shutdown_grace`, `tracing*` | — | — | `foundation_macros/src/valtron_entry.rs:131` | `ctx = expr` sets the engine root, so a test injects a simulated clock: `#[valtron_test(ctx = ctx_with(&CLOCK))]` (`tests/clock_advance_wake.rs:95`). |
| A blocking poll | fails the `slow_poll` warning | not possible | not possible | `foundation_macros/src/valtron_entry.rs:91` | A poll must not block. A wait is a readiness signal, a timer or an async equivalent. |
| A bounded test | yes | — | — | `timeout = ms` and `shutdown_grace = ms` | A hung body fails in bounded time and releases the process-wide pool gate. |

### 5.6 `foundation_compact` and `foundation_context`

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| `SendWrapper` | no `Send` impls | `Send` and `Sync` impls | yes | `src/send_wrapper.rs:16` | Gate: `wasm` and not `emscripten` and not `atomics` (`:18`). Compile-time only; no run-time check. |
| The clock | `std::time` | `performance.now()` and `Date.now()` | yes | `src/wasm/js.rs:13`, `:52`, `:63` | |
| The host-ABI clock | — | needs `foundation-wasm.js` | **no** | `src/wasm/host_abi.rs:7` | |
| The entropy | OS | `crypto.getRandomValues` | yes | `src/entropy/backends/wasm_js.rs:62` | At most 65 536 bytes per call (`:14`). |
| `Ctx` | yes | yes | yes | `foundation_context/src/lib.rs:152` | One `Arc`; a clone shares it (`:555`). |
| A typed entry read | yes | yes | yes | `ctx.get::<T>()`, `src/lib.rs:366` | Returns `Option<Arc<T>>`. |
| A typed entry write | yes | yes | yes | `ctx.store(value)`, `src/lib.rs:345` | |
| An error for a missing entry | **no** | **no** | **no** | — | `get` returns `None`; there is no typed error. |
| `Env` | **absent** | **absent** | **absent** | — | There is no `Env` type in any of the three crates today. |
| `Ctx` is `Send + Sync` | yes | yes | yes | `tests/ctx.rs:16` | It is a compile-time assertion. |

### 5.7 Hazards, stated explicitly

- `Stream::Wait` on a build with no JS backend busy-spins for up to 10 s per wait.
- `__private::block_on_future` compiles on wasm and blocks the isolate thread. It is an
  anti-pattern. Do not depend on it and do not call it. Every wait already has a friendly
  primitive: `StreamIterator`, `TaskStatus`, `into_ready_future`, `into_future_stream` and
  the parking wake path.
- `Ctx::get` has no typed error, so rule 11's "a missing bundle is a typed error" is not
  met by the current type. Spec 84 owns the `Env` entry that closes this.
- `multi` does not compile on wasm, so a crate that enables `foundation_core/multi` cannot
  be built for a Worker.

### 5.8 What this means for this specification

| Planned item | What already exists | Action |
|---|---|---|
| A task in the plan | `#[task]` with an enum state | Write it as the macro form. `None` is the only end; `Ready` is not. |
| A queue with a wake | `WakeRegistry`, `EventQueue` and `WakeableStream` | Reuse them. Do not poll in a loop, and do not spin. |
| A wait inside a task | `StreamIterator`, `TaskStatus` and the valtron stream adapters | Never block. Never call `__private::block_on_future`. Return `Wait` or `Pending` and let the wake path resume the task. |
| A JS stream reader | `JsReadableStreamIterator` in `foundation_core` | Reuse it where the `js-wasmbindgen` feature is on; `foundation_netio` has its own reader because it needs no bindgen in the loop. |
| A clock or randomness in core logic | the `Ctx` clock and random generator | Read them from the `Ctx`; do not call the globals. |
| A test of a streaming path | `#[valtron_test]` on native, `#[valtron_bindgen]` on wasm | Write one `async fn` body and register it on both targets with `dual!`. Never initialize the pool by hand. Inject the clock with `ctx = expr`. |

---

## 6. `foundation_wasm`, `foundation_iogate`, `foundation_nativeapis`

These three crates are the host layer, the readiness layer and the file layer. Read this
section before you plan any timer, queue, file or readiness code.

### 6.1 Summary

| Platform | What's supported | What's not supported |
|---|---|---|
| native | `foundation_wasm` compiles to a stub. `foundation_iogate` has the file-descriptor readiness gate, the shutdown signal, the completion socket and the accept and connect helpers. `foundation_nativeapis` has the virtual file system, `NativeFs`, the native delta stores, file watching, signals and the IPC bus. | — |
| wasm32 (browser) | `foundation_wasm` supplies the JavaScript host ABI: timers, microtasks, animation frames, a lock-free stream queue, a memory arena and host functions. `foundation_nativeapis` supplies the VFS traits and the in-memory file systems. | No networking, no file system, no threads, no offload pool. `SyncFs` and `exec_async` are removed from the API on a JavaScript-hosted build. |
| Cloudflare Worker | The same as the browser. The timers and the queue work. | `WasmLoader.fromScripts` needs the DOM. There is no `requestAnimationFrame`, so the driver falls back to a 16 ms timer. |
| `foundation_iogate` on wasm | — | **It does not build.** `src/lib.rs:39` re-exports `foundation_reactor::ShutdownSignal` on every target, but the dependency exists only for a non-wasm target (`Cargo.toml:20`). |

### 6.2 `foundation_wasm`: the host layer

`foundation_wasm` gives a wasm module a way to call its JavaScript host. It has no network
stack, no file system and no thread support.

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| The `abi` import namespace | stub | yes | yes | `src/host_runtime.rs:711`; stubs at `:1027` | Needs the `web` feature. Native gets no-op re-exports (`:1188`). |
| One transport for every call | stub | yes | yes | `host_apply`, `src/host_runtime.rs:775`; JS `runtime/foundation-wasm.js:1967` | The single bulk data path. |
| Batch apply | stub | yes | yes | `src/host_runtime.rs:751`; JS `:2114` | |
| A checked memory arena | yes | yes | yes | `src/host_runtime.rs:539`; JS `:1392` | A generation-checked `MemoryId`; use it instead of raw pointers. |
| Host functions | registry | yes | yes | `HostFunction`, `src/host_runtime.rs:1877`; register `:1549`; invoke `:1810` | |
| Timeouts, intervals, microtasks | stub | yes | yes | imports `src/host_runtime.rs:725`; JS `:1437` | `setTimeout`, `setInterval`, `queueMicrotask`. |
| Animation frames | stub | yes | timing-degraded | `src/host_runtime.rs:721`; JS `:1613` | A 16 ms timer fallback. |
| A lock-free chunk queue | yes | yes | yes | `StreamQueue`, `src/stream.rs:43` | `ConcurrentQueue<StreamChunk>`; unbounded, no `std`. |
| The call envelope | yes | yes | yes | `src/protocol.rs:96` | A 14-byte header (`:109`); protocols: binary, Arrow, JSON, capability, IPC. |
| A timer value | yes | yes | yes | `schedule.rs:76`; `intervals.rs:166` | Reuse these; do not write another timer. |
| The JavaScript runtime file | — | yes | yes | `runtime/foundation-wasm.js`; `FoundationWasm` at `:1852` | Supplies `web_abi` (`:1962`) and the loader. |
| Embed the runtime | — | yes | yes | `src/embedded.rs:14` | Feature `embedded-js`. |
| A browser page host | — | yes | no | `init`, JS `:2218`; `WasmLoader` `:2255` | The module must export `memory` and the arena and timer exports. |
| Script-tag discovery | — | yes | **no** | JS `:2313` | It needs `document.querySelectorAll`. A Worker uses `loadURL` (`:2284`). |
| Networking | **absent** | **absent** | **absent** | crate docs, `src/lib.rs:1-13` | A wasm HTTP call goes through `foundation_netio`. |
| Files, sockets, threads | **absent** | **absent** | **absent** | crate docs, `src/lib.rs:1-13` | |

### 6.3 `foundation_iogate`: the readiness layer

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| `ServerIo` mode enum | yes | inert | inert | `src/shared/mod.rs:33` | The only target-neutral item. |
| `FdReadiness` | yes | **no** | **no** | `foundation_reactor/src/fd_readiness.rs:32`; re-export `src/native/mod.rs:25` | The `native` module is `cfg(all(not(wasm), unix))` (`src/lib.rs:27`). |
| `ShutdownSignal` | yes | **build error** | **build error** | `foundation_reactor/src/shutdown_signal.rs:30`; re-export `src/lib.rs:39` | See 6.1. |
| `CompletionSocket` | yes | no | no | `src/native/completion_socket.rs:95` | It wraps `foundation_nativeapis::RegisteredFd`. |
| Accept and connect helpers | yes | no | no | `src/native/accept.rs` | |
| A wasm module | — | empty | empty | `src/wasm/mod.rs:1` | Doc comments only. |

### 6.4 `foundation_nativeapis`: the file layer

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| The VFS traits, sync and async | yes | yes | yes | `src/shared/vfs/traits.rs:6`, `:33`, `:49`, `:195`, `:374`; `async_traits.rs:15`, `:28`, `:39`, `:150`, `:302` | Feature `vfs`. |
| `MemoryFs` | yes | yes | yes | `src/shared/vfs/memory_fs.rs:132` | The only file system a wasm build can use. |
| `MemoryDelta` | yes | yes | yes | `src/shared/vfs/memory_delta.rs:13` | |
| `OverlayFileSystem` | yes | yes | yes | `src/shared/vfs/overlay_fs.rs:33` | |
| `ObservableFs` | yes | yes | yes | `src/shared/vfs/observable_fs.rs:152` | |
| `DynFs` | yes | yes | yes | `src/shared/vfs/dynfs.rs:537` | |
| `SyncFs` and `exec_async` | yes | **removed from the API** | removed | `src/shared/vfs/sync_bridge.rs:460`; `exec_async.rs:31`; gate `foundation_core/src/valtron/mod.rs:126` | `blocking_boundary!` removes them on a JavaScript-hosted build. |
| `NativeFs` | yes | **no** | **no** | `src/native/vfs/native_fs.rs:184` | The `native` module is `cfg(not(target_family = "wasm"))` (`src/lib.rs:11`). |
| Delta stores: directory, libsql, turso, D1, R2, fjall, NFS, blob | yes | **no** | **no** | `src/native/vfs/mod.rs:9`, `:43`, `:46`, `:50`, `:53` | The D1 and R2 adapters are named for Cloudflare but compile only on native. |
| FUSE, ptrace, preload | yes | **no** | **no** | `src/lib.rs:11` | |
| File watching | yes | compiles, useless | compiles, useless | `src/shared/watcher/poll_watcher.rs:115` | It calls `std::fs`, which has no host file system on `wasm32-unknown-unknown`. |
| Signals, poll, fd, IPC | yes | **no** | **no** | `src/lib.rs:28`, `:37`, `:53` | |
| `StopSignal`, `CompositeReadiness`, `VfsEventReadiness` | yes | yes | yes | `src/valtron/mod.rs:13`, `:17`, `:23` | The file-descriptor monitor is native only (`:30`). |
| A thread spawn or offload pool | **absent** | **absent** | **absent** | — | `NativeFs` has an `OffloadPool` (`src/native/vfs/native_fs.rs:184`), and it is native only. |

### 6.5 Absences, stated explicitly

- `foundation_wasm` has no network, no file system and no thread abstraction.
- `foundation_iogate` does not compile for `wasm32` as written.
- `foundation_nativeapis` has no path to a real file on `wasm32-unknown-unknown`. Only the
  in-memory file systems exist.
- There is no WASI preview2 backend; `wasip2` is a feature alias only (`Cargo.toml:49`).

### 6.6 What this means for this specification

| Planned item | What already exists | Action |
|---|---|---|
| A queue between the JS reader and the Rust consumer | `StreamQueue` in `foundation_wasm` (`src/stream.rs:43`) and the queue in `foundation_netio` (`src/wasm/client/stream.rs:116`) | Reuse one; do not add a third. |
| A timer for a retry or a deadline | `foundation_wasm::schedule` and `intervals` | Reuse these. |
| A file store on wasm | `MemoryFs` and the VFS traits | Use the VFS traits. Do not call `std::fs`. |
| A readiness gate on wasm | none; `foundation_iogate` does not build | A wasm plan must not depend on `foundation_iogate`. |

---

## 7. `foundation_auth`

`foundation_auth` owns the OAuth and OIDC clients, JWT, password hashing, sessions and the
credential store.

### 7.1 Summary

| Platform | What's supported | What's not supported |
|---|---|---|
| native | OAuth2 and OIDC: the authorization URL, PKCE, the token grants, discovery, userinfo, introspection and the JWKS cache; JWT with EdDSA, RS256 and ES256; Argon2id and PBKDF2 password hashing; sessions with a key ring; the credential store over Turso or over a `StorageProvider`; the IdP server; WebAuthn with `server-native`; TOTP. | — |
| wasm32 (browser) | The shared OIDC clients over `foundation_netio`; PKCE; the session manager; PBKDF2 through `SubtleCrypto` with `wasm-pbkdf2`; `WasmOAuth` with `wasm-bindgen-oauth`; the credential store over D1, KV, R2 or memory. | No Argon2id; no WebAuthn; no IdP server; no native password login. |
| Cloudflare Worker | The same as the browser, with D1 as the session store. | `WasmOAuth` has no Cloudflare-specific path: `js_fetch` takes the `ServiceWorkerGlobalScope` branch when that global exists, and otherwise calls `window()`, which panics when both are absent (`src/wasm_bindgen/oauth.rs:181-194`). Whether `workerd` exposes that global is not measured. `D1CredentialStore` needs `foundation_db/wasm-bindgen-storage` (`Cargo.toml:141-145`). |
| A `Secret` type | **absent** | Spec 86 owns it. The crate has `ConfidentialText` (`src/shared/types.rs:17`). |
| One constant-time compare | **absent** | Four copies exist; spec 86 owns the single one. |

### 7.2 The OAuth and OIDC surface

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| Authorization URL, PKCE, state and nonce | yes | yes | yes | `OAuthManager::get_authorization_url`, `src/shared/oauth.rs:263`; `PkceChallenge::generate`, `:199` | Pure Rust. |
| State validation | yes | yes | yes | `OAuthManager::validate_state`, `src/shared/oauth.rs:309` | A plain `==` compare, not constant-time. |
| Upstream token exchange | yes | yes | yes | `UpstreamOidcClient::exchange_code`, `src/shared/upstream_client.rs:295` | Uses `foundation_netio`, so it works on wasm. |
| Discovery, userinfo, introspection | yes | yes | yes | `src/shared/discovery.rs:202`; `userinfo.rs:135`; `introspection.rs:116` | All over `foundation_netio`. |
| The JWKS cache | yes | yes | yes | `JwksManager`, `src/shared/jwks.rs:276` | |
| A dedicated wasm OAuth client | — | yes | yes; it panics without either global | `WasmOAuth`, `src/wasm_bindgen/oauth.rs:40` | Its own `web_sys::fetch` at `:150`; it reads `Response::text()` at `:253`; `window().expect(...)` at `:191` panics when there is no window and no service worker. |
| The native OAuth client | yes | — | — | `NativeOAuth`, `src/native/oauth.rs:14` | Over `DynNetClient`. |

### 7.3 Password hashing, sessions and JWT

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| Argon2id | yes | **no** | **no** | `src/shared/password_hash.rs:144` | The dependency is `cfg(not(target_family = "wasm"))`. |
| PBKDF2 | yes, sync | yes, async | yes | native `src/shared/password_hash.rs:35`; wasm `:61` | `SubtleCrypto`; a maximum of 100 000 iterations (`:31`). |
| `verify_password` | yes | PBKDF2 only | PBKDF2 only | `:279`; wasm `:305` | wasm rejects Argon2id (`:315`). |
| The session store trait | yes | yes | yes | `CredentialStore`, `src/shared/credential_store.rs:32` | `get`, `set`, `delete`, `exists`, `list_keys`. |
| `SessionManager` | yes | yes | yes | `src/shared/session.rs:102` | HMAC tokens, a key ring, sliding expiry. |
| The D1 session store | — | yes | yes | `D1CredentialStore`, `src/wasm_bindgen/d1_credential_store.rs:18` | Needs `wasm-bindgen-session`; no in-repo consumer. |
| `WasmSessionManager` | — | yes | yes | `src/wasm_bindgen/session.rs:28` | One key; the HMAC is compared with `!=` at `:141`, not constant-time. |
| JWT signing | yes | yes | yes | `JwtSigningKey`, `src/shared/jwt.rs:239`; `sign_claims` `:260` | Rust `jwt-simple`, not `SubtleCrypto`. |
| JWT verification | yes | yes | yes | `JwtVerifier::verify`, `src/shared/jwt.rs:193` | EdDSA, RS256 and ES256. There is no HS256. |
| TOTP and backup codes | yes | yes | yes | `TOTPSecret`, `src/shared/two_factor.rs:44`; `BackupCodeSet`, `:288` | Pure Rust. |
| WebAuthn | yes, `server-native` | **no** | **no** | `src/server/services/webauthn_service.rs:107` | `webauthn-rs` and `openssl`. |

### 7.4 Absences, stated explicitly

- There is no `Secret`, no single `constant_time_eq` and no `bearer_header`; spec 86 owns
  all three. The crate has `extract_bearer_token` (`src/shared/middleware.rs:123`).
- There is no HS256 JWT, although the crate's overview document claims one.
- The IdP server module does not compile for wasm: it imports `foundation_http::native`
  (`src/server/handlers/serve_adapter.rs:7`).
- There is no Workers fetch adapter. The only Worker example is
  `examples/cf-login-app/src/lib.rs`, and it uses `CredentialStorage` and `SessionManager`.

### 7.5 What this means for this specification

| Planned item | What already exists | Action |
|---|---|---|
| A bearer credential on an RPC call | `extract_bearer_token` and spec 86's future `Secret` | Do not add a parser here. Spec 86 owns the type. |
| A wasm HTTP call in the auth crate | `foundation_netio`'s `DynNetClient` | `WasmOAuth` should use it. It repeats the fetch and the whole-body read that spec 85 removes. |
| A session in a Worker | `SessionManager` over `CredentialStorage` on D1 | Reuse it; do not write a new session type. |

---

## 8. `foundation_deployment_cloudflare`

This crate has two halves. The `workers` module is a Worker-side runtime helper that
compiles for wasm. The REST client, the DNS code and the generated API are native-only.

### 8.1 Summary

| Platform | What's supported | What's not supported |
|---|---|---|
| native | The REST API client with a token, the generated API groups for Workers, D1, KV, R2, versions and deployments, and the DNS operations. Tests use serde and a simulated clock only. | No deployer, no plan type, no `wrangler.toml` writer, no D1 migration runner, no Worker rollback, no Durable Object namespace creation. |
| wasm32 (browser) | The `workers` feature gives the runtime helper module, which uses the `worker` crate. | Without the `workers` feature the crate is empty on wasm. |
| Cloudflare Worker | `get_d1`, `get_kv`, `get_r2`, `get_secret`, `get_do_namespace`, `do_connect`, `WebSocketHub`, `with_d1`, `accept_websocket` and the SignalR handshake. | No deployer and no plan. The REST half is not compiled. |
| Default features | `default = ["cloudflare"]` pulls the native REST stack | A wasm consumer uses `default-features = false, features = ["workers"]`, as `foundation_keychain` does (`foundation_keychain/Cargo.toml:66`). |

### 8.2 The Worker runtime helpers

These are the types a Worker host should use instead of writing its own `env` plumbing.

| Capability | Worker | Symbol and file:line | Notes |
|---|---|---|---|
| Read a D1 binding | yes | `get_d1`, `src/workers/env.rs:20` | |
| Read a KV binding | yes | `get_kv`, `src/workers/env.rs:29` | |
| Read an R2 binding | yes | `get_r2`, `src/workers/env.rs:38` | |
| Read a secret | yes | `get_secret`, `src/workers/env.rs:49` | |
| Read a Durable Object namespace | yes | `get_do_namespace`, `src/workers/env.rs:59` | |
| Connect to a Durable Object | yes | `do_connect`, `src/workers/durable_object.rs:35` | Broadcast helpers at `:49`, `:58`, `:65`; `connected_count` at `:73`. |
| A WebSocket hub | yes | `WebSocketHub`, `src/workers/durable_object.rs:87`; `WebSocketRegistry` at `:118` | |
| A D1 handle inside a route | yes | `with_d1`, `src/workers/context.rs:21`; async form at `:31` | |
| Accept a WebSocket in a Worker | yes | `accept_websocket`, `src/workers/websocket.rs:33` | Send, close and ping at `:51`, `:60`, `:65`. |
| A SignalR handshake | yes | `SignalRHandshake`, `src/workers/websocket.rs:76` | |
| The gate | — | `src/lib.rs:22`; `Cargo.toml:117` | wasm and the `workers` feature; the `worker` crate is 0.8.3. |

### 8.3 The REST and deployment side

| Capability | native | wasm | Symbol and file:line | Notes |
|---|---|---|---|---|
| The API client | yes | **no** | `CloudflareClient`, `src/client.rs:16` | Token, zone, domain, base URL and a `Ctx`. There is no account field. |
| The base URL | yes | no | `DEFAULT_BASE_URL`, `src/client.rs:13` | `https://api.cloudflare.com/client/v4`. |
| The HTTP client | yes | no | `DynNetClient`, `src/client.rs:5` | Wasm fetch is deliberately not enabled (`Cargo.toml:38`). |
| Generated API groups | yes | no | `src/generated/mod.rs:1` | 43 groups, each behind a feature (`src/lib.rs:17`). |
| Worker script list and download | yes | no | `foundation_cloudflare_workers/src/workers/mod.rs:13321`, `:13422` | |
| Worker script upload | **partial** | no | `worker_script_upload_worker_module_request`, `.../workers/mod.rs:13469` | The generated args have no body. `builder_mod` can only call `set_body_json` (`foundation_netio/src/shared/client/request_builder.rs:167`); `body_bytes` consumes the builder (`:203`). A module byte upload is not reachable today. |
| Worker versions and deployments | yes | no | `.../workers/mod.rs:15566`, `:13720`, `:13767` | A gradual rollout is an untyped string and a list of maps (`:4123`). |
| D1, KV and R2 management | yes | no | `foundation_cloudflare_workers/src/workers/mod.rs:8591`; `foundation_cloudflare_storage/src/storage/mod.rs:3419`, `:2453` | |
| A deployer or a plan | **absent** | absent | none | No `Deployable` implementation and no plan type. |
| A `wrangler.toml` writer | **absent** | absent | none | `foundation_deployment/src/config.rs:31` only detects the file. |
| D1 migrations | **absent** | absent | none | |
| Durable Object namespace creation | **absent** | absent | `.../workers/mod.rs:12575` only lists namespaces. |
| A Worker rollback | **absent** | absent | none | |
| `wrangler dev` or Miniflare | **absent** | absent | none | |

### 8.4 What this means for this specification

| Planned item | What already exists | Action |
|---|---|---|
| A Worker host that reads bindings | `get_d1`, `get_kv`, `get_r2` and the rest in `src/workers/env.rs` | Reuse these helpers; do not write a second `env` reader. |
| A Durable Object client | `do_connect` and the hub types | Reuse them. There is no `StubHttpClient`; spec 85 T5 adds it. |
| A deployment plan | nothing | Do not plan one here. A plan is spec 77's work and needs a new type. |
| A Worker script upload | the generated request cannot carry module bytes | The generator must gain a byte-body path before a deploy can work. |

---

## 9. `foundation_agent_rpc`

This crate holds the agent RPC service, its client and its mutual-TLS auth. It is the
consumer that spec 83 feature 05 wants to run in a Worker.

### 9.1 Summary

| Platform | What's supported | What's not supported |
|---|---|---|
| native | `AgentService` and `BootstrapService`, the generated client, `AgentRpcService`, `agent_router` over `foundation_connectrpc::Router`, the native server with HTTP/1.1 and h2c, the mutual-TLS enrollment and trust flow, the instance registry and the model handles. | No middleware on its routes, no interceptor. It uses no D1, KV or R2. |
| wasm32 (browser) | **Nothing. The crate does not compile.** | Every module is unconditional, and `src/server.rs:27` imports `foundation_http::native`, which does not exist on wasm. |
| Cloudflare Worker | **Nothing. The crate does not compile, and it has no Worker code.** | No Worker entry, no `CfServe` registration, no Durable Object code, no fetch transport wiring. |
| Features | `candle`, `gguf` | There is no `wasm` and no `cloudflare` feature. |

### 9.2 The surface and its target gates

| Capability | native | wasm | Symbol and file:line | Notes |
|---|---|---|---|---|
| The service trait | yes | blocked by the crate gate | `AgentService`, `proto/ewe/agent_rpc/v1/agent.proto:11`; `src/proto.rs:26` | Generated by `build.rs:60`; needs `protoc`. |
| The client | yes | blocked | `AgentServiceClient`, a generated type re-exported at `src/client.rs:36` (`src/proto.rs:27`) | Uses `foundation_connectrpc::Client` over an `H1Transport`. |
| The service implementation | yes | blocked | `AgentRpcService`, `src/service.rs:49` | The implementation is at `:83`. |
| The RPC router | yes | blocked | `agent_router`, `src/service.rs:77` | Registers through the generated `register_agent_service`. |
| A streamed reply | yes | blocked | `TurnEventStream`, `src/service.rs:46`; `ChatStream` at `:205` | `Stream<Item = ConnectResult<TurnEvent>> + Send`. |
| The native server | yes | **no** | `AgentRpcServer`, `src/server.rs:179` | `std::net::TcpListener` and `std::thread` (`:467`, `:516`). |
| Route registration | yes | **no** | `src/server.rs:527` | `add_route_any` for H1 and `route_any_h2` for H2. |
| The instance registry | yes | blocked | `InstanceRegistry`, `src/instance.rs:216` | |
| Model handles | yes | blocked | `ModelHandle`, `src/model.rs:98` | |
| Enrollment and trust | yes | **no** | `src/auth.rs:489`, `:615`; `src/trust.rs:153` | rustls, files and SQLite. |
| The bootstrap payload | yes | blocked | `src/bootstrap.rs:27` | Pure Rust, but the crate gate blocks it. |
| Tests | 14 native files | **none** | `tests/` | No `wasm-bindgen-test`; not in the wasm test list (`ci/packages.toml:319`). |

### 9.3 What blocks a wasm or Worker build

| Blocker | Where | What it means |
|---|---|---|
| `foundation_http::native` import | `src/server.rs:27`; `foundation_http/src/lib.rs:22` | The server module must be native-only. |
| `foundation_netio` `ssl` and netcap | `Cargo.toml:28`; `src/client.rs:26`; `src/auth.rs:35` | The mutual-TLS client and the TLS listeners must be native-only. |
| `rcgen`, `rustls`, `ring` | `Cargo.toml:33` | Keys and certificates cannot be generated in a Worker. |
| The file-based CA and key state | `src/secret.rs:82`; `src/auth.rs:392` | It uses `std::fs` and Unix permissions. |
| The SQLite trust store | `src/trust.rs:153`; `Cargo.toml:20` | A Worker would need D1, and no such store exists. |
| `std::net` and `std::thread` | `src/server.rs:467`, `:516` | A Worker has neither. |

### 9.4 What this means for this specification

| Planned item | What already exists | Action |
|---|---|---|
| A Worker host for the agent RPC | `AgentRpcService`, `agent_router` and `ConnectRpcHandler` | The crate needs a target split first: `proto`, `service` and the registry stay shared; `server`, `client` TLS and `trust` become native-only. Spec 85 cannot host a crate that does not compile. |
| A DO store for the registry or trust | nothing | This is new work in `foundation_db`, and it does not exist. |
| A streamed `ChatStream` reply in a Worker | `TurnEventStream` and `ResponseSink` | T4's sink must carry it; the current sink path buffers. |
| Unary `Chat` | `src/service.rs:168` gathers every record into a `Vec` | It buffers the request; it needs a stream or a limit. |

---

## 10. The Cloudflare management crates

`foundation_cloudflare_workers`, `foundation_cloudflare_storage`, `foundation_cloudflare_apps`
and `foundation_cloudflare_types` hold the generated Cloudflare management API. They are not
in a request path. Read this section before you plan a deploy step, a generated type or a
Durable Object.

### 10.1 Summary

| Platform | What's supported | What's not supported |
|---|---|---|
| native | The generated management wrappers for scripts, versions, deployments, secrets, routes, subdomains, settings, dispatch namespaces, D1, Durable Object listing, R2, KV, cron, tails, usage and Alexandria apps; the code generator; an offline generated-type check. | A request body on the script and version upload wrappers; a multipart body on any wrapper; a live-account test. |
| wasm32 (browser) | `foundation_cloudflare_types` only: it has no target-gated dependency. | The three wrapped crates: each has an unconditional `foundation_deployment` dependency and `foundation_netio` with `ssl` and `multi`. |
| Cloudflare Worker | The same as the browser. | Everything the browser cannot do; a Worker is a wasm32 target. The only Worker code in the umbrella is its runtime `workers` module. |
| The umbrella gate | native for `types` and the REST client; wasm for the runtime `workers` module | The umbrella gates `types` native, although the `types` crate itself compiles on wasm. |

### 10.2 The crates and their gates

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| The worker management wrappers | yes | **no** | **no** | `pub mod workers`, `foundation_cloudflare_workers/src/lib.rs:30` | `Cargo.toml:13` and `:15` are unconditional. |
| The storage wrappers | yes | **no** | **no** | `foundation_cloudflare_storage/src/lib.rs:30` | `Cargo.toml:13` and `:15`. |
| The apps wrappers | yes | **no** | **no** | `foundation_cloudflare_apps/src/apps/mod.rs:300` | `Cargo.toml:13` and `:15`. |
| The generated wire types | yes | yes | yes | `pub mod generated`, `foundation_cloudflare_types/src/lib.rs:32` | Serde, base85rs, sha2 and the macros only (`Cargo.toml:12-20`). No target gate. |
| The umbrella re-export | native | **no** | **no** | `foundation_deployment_cloudflare/src/lib.rs:12-21` | It gates the `types` group native, so a wasm consumer cannot reach the target-neutral crate through the umbrella. |
| The runtime `workers` helpers | — | yes | yes | `src/lib.rs:22-23` | This is the `get_d1`, `do_connect` and hub layer of section 8, not the management client. |
| The CI wasm gate | n/a | umbrella only | umbrella only | `ci/packages.toml:397` | The group crates are declared native-only (`:395`). |

### 10.3 The worker management API

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| Script list and search | yes | **no** | **no** | `worker_script_list_workers_request`, `workers/mod.rs:13321`; search `:13367` | |
| Script download and content | yes | **no** | **no** | `:13422`, `:13673` | A `ByteStream`. |
| Script module upload | yes | **no** | **no** | `:13469` | No request-body argument; only the `bindings_inherit` query (`:13489`). |
| Script put content | yes | **no** | **no** | `:13621` | No request-body argument. |
| Script delete | yes | **no** | **no** | `:13518` | |
| The asset upload session | yes | **no** | **no** | `:13570` | A JSON body. |
| Deployments | yes | **no** | **no** | `:13720`, `:13767`, `:13823`, `:13870` | Create takes a JSON `WorkersDeployment` (`:13767`). |
| Legacy versions | yes | **no** | **no** | `:14740`, `:14794`, `:14843` | The upload has no body. |
| New worker family | yes | **no** | **no** | `:15223` to `:15671` | `create_worker_version_request` (`:15566`) takes a JSON body. |
| Secrets and settings | yes | **no** | **no** | `:14113` to `:14356` | `WorkersSecret` is untyped JSON (`generated.rs:4683`). |
| Routes and subdomains | yes | **no** | **no** | `:16384` to `:16568`; `:14403` to `:14501`; `:15087` to `:15179` | Zone-scoped. |
| Dispatch namespaces | yes | **no** | **no** | `:11173` to `:12336` | The only script-binding read is the namespace one, `:11759`. |
| D1 management | yes | **no** | **no** | `:8591` to `:9136` | D1 lives in the workers group, not the storage group. |
| Durable Object management | list only | **no** | **no** | `:12575`, `:12625` | Two GETs. The spec has no create, delete or migration operation. Migrations appear only as opaque fields (`generated.rs:4769`). |
| R2 and KV management | yes | **no** | **no** | `foundation_cloudflare_storage/src/storage/mod.rs:2453` to `:4020` | KV value read is a `ByteStream` (`:3919`); write uses `with_raw_body` (`:3967`). |
| Alexandria apps | yes | **no** | **no** | `foundation_cloudflare_apps/src/apps/mod.rs:300` to `:501` | Five endpoints. |

### 10.4 The transport under the wrappers

| Capability | native | wasm32 | Worker | Symbol and file:line | Notes |
|---|---|---|---|---|---|
| The client | yes | **no** | **no** | `ApiClient`, `foundation_deployment/src/providers/common/api_types.rs:97` | Over `DynNetClient` (`:98`). |
| JSON response | yes | **no** | **no** | `send_json`, `:167` | Buffers the whole body up to `max_json_body`, 1 MiB by default (`body.rs:89`). Acceptable for a small management reply. |
| Byte download | yes | **no** | **no** | `send_bytes`, `:254`; `ByteStream`, `body.rs:194` | Streamed. |
| JSON lines | yes | **no** | **no** | `send_json_lines`, `:232` | Exported; no call site in these crates. |
| A raw or multipart request body | **absent** | **absent** | **absent** | — | `request_builder.rs` has a JSON setter only (`:167`, `:187`, `:203`, `:219`). |

### 10.5 Absences, stated explicitly

- There is no `set_body_bytes` and no multipart setter. A module upload cannot carry its bytes,
  so the generated script-upload and put-content wrappers are not usable for a real deploy.
- There is no Durable Object create, delete or migration operation in the vendored OpenAPI
  document. The migration fields on `WorkersVersion` are opaque JSON.
- There is no test in the three wrapped crates. The checks live in
  `foundation_codegentools/tests/cloudflare_types.rs:45` (offline generation check) and
  `foundation_deployment_cloudflare/tests/types_tests.rs:64`.
- No live-account test exists.
- The generated `types` crate is target-neutral, but the umbrella hides that fact.

### 10.6 What this means for this specification

| Planned item | What already exists | Action |
|---|---|---|
| Anything in the request path | nothing in these crates | Do not plan a change here. A fetch transport never calls the management API. |
| A Worker module upload for a deploy | the generated upload wrapper, without a body | Out of scope. Record it against the deployment work; the fix is a raw body setter in the request builder. |
| Generated Cloudflare types in a Worker | `foundation_cloudflare_types`, which compiles on wasm | Reuse them through a direct dependency. Do not route through the umbrella, which gates them native. |
| A Durable Object migration | opaque fields only | No migration engine exists on any target. A plan must not assume one. |

---

## 11. `foundation_testbed` and the test matrix

`foundation_testbed` decides which test runs where. Read this section before you plan a test
for the wasm path. A test that does not run is not a test.

### 11.1 Summary

| Platform | What's supported | What's not supported |
|---|---|---|
| native | `cargo test` with `#[test]`, `#[valtron_test]` and `#[valtron]`; the multi executor; a real TCP listener in a test; `MockDnsResolver`. | No fake `fetch`. |
| wasm32 (browser) | The `wasm-testbed` binary: the owned `#[wasm_test]` ABI in Deno or a Chromium page, and `#[wasm_bindgen_test]` or `#[valtron_bindgen]` in Deno or a Chromium page. | `#[valtron_test]`, because it expands to `#[test]`. No fake `fetch`; Deno uses the real `deno_fetch`. The browser suite has no CI caller. |
| Cloudflare Worker | `#[wasm_bindgen_test]` and `#[valtron_bindgen]` under `bindgen-wrangler`, which runs `wrangler dev --local` with real D1, R2 and KV bindings. | `#[wasm_test]` and `#[valtron_wasm_test]`, which need the owned host ABI. No Durable Object test. No generic "run any crate in a Worker" target. |
| The compile gate | `cargo check --tests --target wasm32-unknown-unknown` | It compiles the test. It does not run it. |

### 11.2 The runners

| Capability | native | wasm32 | Worker | Symbol, command and file:line | Notes |
|---|---|---|---|---|---|
| The test binary | yes | yes | yes | `wasm-testbed`, `foundation_testbed/src/bin/wasm-testbed.rs` | Feature `cli`; default features are `wasm,cli` (`Cargo.toml:11`). |
| The owned test ABI | — | Deno and browser | **no** | `__fwt_` exports and the `__fwt_manifest` section, `src/wasm/fwt.rs:24`; macro `foundation_macros/src/lib.rs:591` (impl `wasm_test.rs:218`) | One fresh instance per case. |
| `#[wasm_test]` | — | yes | **no** | `foundation_macros/src/lib.rs:591` | |
| `#[valtron_wasm_test]` | — | yes | **no** | `foundation_macros/src/lib.rs:609` | `#[wasm_test]` plus pool setup. |
| `#[wasm_bindgen_test]` | — | Deno and browser | yes | discovery scans the `__wbgt_` bytes, `src/wasm/wasm_test.rs:26`; runner `bindgen_runner.rs:94` | There is no `wasm-bindgen-test-runner`. |
| `#[valtron_bindgen]` | — | Deno and browser | yes | `foundation_macros/src/bindgen_test.rs:7` | The pool is live for the body. |
| `#[valtron_test]` / `#[valtron]` | yes | **no** | **no** | `foundation_macros/src/valtron_entry.rs:505` | It emits `#[test]`. |
| The Deno runtime | — | in-process V8 | **no** | `src/wasm/embedded_js.rs:246`; feature `wasm-embedded-js` (`Cargo.toml:56-69`) | Real `deno_fetch` (`:246`). |
| The browser runtime | — | Chromium over CDP | **no** | `src/wasm/browser.rs:1` | Pure Rust driver. |
| The Worker runtime | — | — | `wrangler dev --local` | `src/wasm/wrangler.rs:30`, `:175` | Polls `/health`; the template gives the test real D1, R2 and KV (`src/wasm/templates/bindgen-wrangler/`). |
| A fake `fetch` | **absent** | **absent** | **absent** | — | No kind of `fetch` stub exists. |
| The clock in a test | `SimulatedClock` through `Ctx` | injectable by a library caller | injectable by a library caller | `foundation_core/src/valtron/ctx.rs:31`; `src/bin/wasm-testbed.rs:37` | The CLI passes `Ctx::host()`; the test macro has no clock argument. |
| A seeded random generator | `Ctx::with_seed` | yes | yes | `foundation_context/src/lib.rs:220` | The wasm test macros take no seed; they hash `RandomState` (`bindgen_test.rs:59`). |

### 11.3 The commands, and what runs them

| Command | Platform | What it runs | In CI? |
|---|---|---|---|
| `mise run test:wasm-testbed:deno` | wasm (Deno) | the sample crate, then `foundation_core` `wasm_tests` | yes, `scripts/ci/ci.py:772` |
| `mise run test:wasm-bindgen:deno` | wasm (Deno) | `bindgen-deno`, filter `wasm_js_yield_integration` | yes, `ci.py:773` |
| `mise run test:wasm-testbed:wrangler` | Worker | `wasm-testbed test bindgen-wrangler backends/foundation_db/integration/wasm_cf` | yes, `ci.py:777` |
| `mise run test:wasm-testbed:web` | browser | the Chromium suite | **no** workflow calls it (`mise.toml:1080`) |
| `mise run test:cf`, `test:cf:r2`, `test:cf:d1`, `test:vfs:cf:*` | native against the local explorer | D1 and R2 through the REST API | **no** workflow calls them (`mise.toml:599-724`) |
| `cargo check --tests --target wasm32-unknown-unknown` | wasm | compiles the `foundation_core`, `foundation_netio`, `foundation_wireguard`, `wasm_tests` and `wasm_cf` test targets | yes, `ci.py:749-768` |
| `cargo check --target wasm32-unknown-unknown -p <crate>` | wasm | the per-package wasm gate | yes, `ci.py:835-868` |

### 11.4 Absences, stated explicitly

- No fake `fetch` exists on any target. A test of the wasm HTTP path uses a real server: a
  Deno `data:` URL, a local TCP listener, or `wrangler dev --local`.
- `foundation_netio` and `foundation_wireguard` test targets are compile-checked only
  (`ci.py:751-756`). Their wasm tests do not execute in CI.
- No Durable Object test target exists. The one unit test in
  `foundation_deployment_cloudflare/src/workers/durable_object.rs:176` sits in a module gated
  `all(target_family = "wasm", feature = "workers")`, and no test target includes it.
- The Worker runner runs one integration crate, `foundation_db/integration/wasm_cf`. There is
  no general "run crate X in a Worker" step.
- The browser suites, the native explorer suites and the `examples/cf-*` builds have no CI
  caller. The examples are built, not run (`ci.py:729`).
- The `wasm-testbed` CLI passes `Ctx::host()`, so a testbed test cannot inject a simulated
  clock unless it builds its own engine.

### 11.5 What this means for this specification

| Planned item | What already exists | Action |
|---|---|---|
| A test for a streamed response on wasm | `#[valtron_bindgen]` in `bindgen-deno` or `bindgen-wrangler` | Write the test there. Do not write a `#[valtron_test]` for it; that macro is native only. |
| A test for the DO stub client | the Worker runner with real bindings | Add an integration crate next to `foundation_db/integration/wasm_cf`; the runner has no generic form. |
| A test that needs a fake server | nothing; only a real server | Run a real local server in the test. State that the test uses the real service, per the repository rule. |
| A test of the `foundation_netio` wasm path | a compile-checked test target | The plan must add an execution step for that crate, or the new code lands untested on wasm. |
| A browser test | `test:wasm-testbed:web` | No CI caller exists. Ask the owner before you depend on a browser-only result. |
| A test with an injected clock on wasm | `Ctx` injection from library code | The macro has no clock argument. Build the engine in the test body. |

---

## 12. What the plan should reuse

This section turns the map into decisions. For each need, it names the type that exists, the
gap that remains, and what the plan does.

### 12.1 The reuse table

| Need | The type that exists | Where | The gap | What the plan does |
|---|---|---|---|---|
| One HTTP client on both targets | `HttpClient: Send + Sync` | `foundation_netio/src/shared/client/http_client.rs:65` | none | Reuse. Write no client. |
| One handle to hold it | `DynNetClient = Arc<dyn NetClient>` | `foundation_netio/src/network_client.rs:41` | none | Reuse. Write no seam. |
| The per-target choice | `HttpClientBuilder::build()` | `foundation_netio/src/network_client.rs:440` | none | Reuse. Write no `cfg` select. |
| An RPC transport | `H1Transport` in `shared/`, no target gate | `foundation_connectrpc/src/shared/transport/h1.rs:61` | none | Reuse. Write no transport. |
| A streamed request body on wasm | `SendSafeBody::Stream` to a `ReadableStream` | `foundation_netio/src/wasm/client/client.rs:395` | none | Reuse. It already streams. |
| A streamed response body on wasm | `spawn_stream_reader` and the pull reader | `foundation_netio/src/wasm/client/stream.rs:44`; `src/wasm/response.rs` | `send_async` reads `text()`; the SSE queue is unbounded and has no cancel | Small enhancement (T1). Extract the reader; add a bound and a cancel. |
| A served response body | the pull-based `web_sys::Response` with cancel | `foundation_netio/src/wasm/response.rs` | the bridges call `req.text()` | Small enhancement (T3), not a new stack. |
| A wasm serving stack | `HttpApp`, `CfServe`, `CfConn`, `WebServe`, the bridges | `foundation_http/src/wasm/` | the H1 body limit is not enforced | Reuse, plus one limit fix. |
| A router and middleware | the `foundation_http` router and chain | `foundation_http/src/shared/router/` | `:param` captures are dropped | Reuse. Do not add a second router. |
| An RPC procedure match | `ConnectRpcHandler` over the exact path | `foundation_connectrpc/src/shared/router/mod.rs:185` | none | Reuse. Mount the whole prefix. |
| The server side of a streamed reply | `ResponseSink` (`whole`, `open`, `chunk`, `finish`) | `foundation_connectrpc/src/shared/router/dispatch.rs:375` | no trailers | Reuse. |
| A DO stub client | `do_connect`, `get_do_namespace`, the hub | `foundation_deployment_cloudflare/src/workers/` | no `NetClient` over a stub | New code (T5), the only new client type. |
| D1, KV and R2 access | `D1WasmStorage`, `KVWasmStorage`, `R2WasmStorage` | `foundation_db/src/wasm/wasm_storage/` | no Durable Object SQL store | Reuse the three; do not add a fourth. |
| A schema change on wasm | `Migration`, `MIGRATIONS`, `MigrationRunner::run_async` | `foundation_db/src/core/schema/` | no `MigrationSet` type | Reuse. Name the real types. |
| Auth primitives | `extract_bearer_token`; spec 86's `Secret` | `foundation_auth/src/shared/middleware.rs:123` | `Secret` does not exist yet | Wait for spec 86. Write no second type. |
| A wasm test with a live engine | `#[valtron_bindgen]` | `foundation_macros/src/bindgen_test.rs:7` | `foundation_netio` tests do not execute on wasm | Reuse the macro; add the execution step. |
| A task or a stream | `#[task]`, `TaskStatus`, `WakeRegistry`, `EventQueue` | `foundation_core/src/valtron/` | none | Reuse. Write no pump and no block. |
| A Worker host for a request | `HttpApp<Arc<dyn CfServe>>`, `CfHttpAppSingleton` | `foundation_http/src/wasm/` | none | Reuse. |

### 12.2 What the plan must not build

1. A client trait, a transport, a fetcher or a body pump. Six such types already exist.
2. A second wasm serving stack, a second router, a second middleware chain or a third
   response builder. `CfConn::into_response` and `WasmResponse` already exist.
3. A `/:method` capture. The RPC router matches the exact path.
4. A CORS decorator. No such helper exists, and CORS is the policy of one host.
5. A migration set type. The engine has `Migration` and `MigrationRunner`.
6. A Durable Object SQL store, a deployer or a plan writer in this specification.
7. A blocking wait. No `__private::block_on_future`, no spin loop, no sync consumption of a
   valtron stream.

### 12.3 Contradictions this map found in the current documents

These are claims in the plan and in the related specifications that the code does not
support. Each one is corrected in the review that follows this map.

| Document | The claim | The code | The correction |
|---|---|---|---|
| Spec 83 feature 05 | a `ConnectService` type | `ConnectRpcHandler` (`foundation_connectrpc/src/shared/router/dispatch.rs`) | Use `ConnectRpcHandler`. |
| Spec 83 feature 05 | a route `/:method` | an exact `HashMap<String, HandlerEntry>` on the full path (`foundation_connectrpc/src/shared/router/mod.rs:185`) | Mount the prefix; let the handler match. |
| Spec 83 feature 05 | `cors::decorate` | no such helper; the chain never decorates a response | The host sets the headers; or a small helper is added and named. |
| Spec 83 feature 05 | a Worker host for agent RPC | `foundation_agent_rpc` does not compile for wasm | The crate needs a target split first. |

Spec 85 itself already matches the code on the points an earlier draft got wrong: T3
extends `streaming_response` in `foundation_netio/src/wasm/response.rs`
(`requirements.md:127`), and T2 corrects `WASM_FETCH_CAPABILITIES` (`requirements.md:285`).
No `MigrationSet` appears in this specification; that name belongs to spec 80 and is a
planned type there.
