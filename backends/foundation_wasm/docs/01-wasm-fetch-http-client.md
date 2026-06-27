# Fundamentals: wasm `fetch` HTTP client (F00f)

Zero-to-expert on the cross-platform `HttpClient` trait, the wasm `FetchHttpClient`,
and the valtron `js_stream` bridge. Read this if you've never worked with `web_sys`,
`wasm-bindgen-futures`, or streaming bodies across the native/wasm boundary.

---

## 1. The problem: native-only HTTP in a cross-platform stack

`foundation_netio`'s HTTP client — `SimpleHttpClient<R: DnsResolver>` — is built on
TCP sockets, rustls/native-tls, and OS-level DNS. None of that exists on wasm32.
Browsers and Cloudflare Workers provide `fetch()` instead — a completely different API.

Before F00f, every `foundation_ai` provider carried a `R: DnsResolver` type parameter
that leaked through model structs, stream structs, and every impl block. The providers
were structurally native-only.

The goal: one trait that both native and wasm implement, so providers don't know or
care which platform they're on.

---

## 2. The `HttpClient` trait: async, object-safe, platform-agnostic

```rust
pub type BoxedSseIterator = Box<dyn Iterator<Item = Stream<ParseResult, SseProgress>> + Send>;
pub type BoxedSseFutureStream = Pin<Box<dyn futures_core::Stream<Item = Stream<ParseResult, SseProgress>> + Send>>;

#[async_trait::async_trait]
pub trait HttpClient: Send + Sync {
    async fn send_async(&self, req: PreparedRequest) -> Result<SimpleResponse<SendSafeBody>, HttpClientError>;
    async fn send_sse_async(&self, req: PreparedRequest) -> Result<BoxedSseFutureStream, HttpClientError>;
    fn send(&self, req: PreparedRequest) -> Result<SimpleResponse<SendSafeBody>, HttpClientError>;
    fn send_sse(&self, req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError>;
}
```

The trait has four methods — async and sync surfaces for both regular and SSE requests.
Async methods return async types (`Future`, `futures_core::Stream`); sync methods return
sync types (`Result`, `Iterator`). Each platform implements both properly — no lazy
delegation. On native, `send()` calls `ClientRequest::send()` (sync) and `send_async()`
calls `ClientRequest::send_async().await` (truly async). On wasm, async methods are
canonical (using `fetch()`); sync methods wrap via `valtron::run_future()`.

**Why `PreparedRequest` and `SimpleResponse<SendSafeBody>`?** These already exist as
cross-platform types. `PreparedRequest` carries a proper `Uri` (RFC 3986 parsing from
`foundation_core::url`), `SimpleMethod`, `SimpleHeaders`, `SendSafeBody`, and
`Extensions`. `SimpleResponse<SendSafeBody>` carries `Status`, `SimpleHeaders`, and
the full body. No new request/response types were invented.

**Why `Send + Sync`?** Providers store `Arc<dyn HttpClient>`. On native with
multi-threaded executors, the client must be shareable. On wasm32 (single-threaded),
`Send`/`Sync` are satisfied via `unsafe impl` since there's only one thread.

**Why object-safe?** So providers use `Arc<dyn HttpClient>` instead of being generic
over the client type. This eliminates the `R: DnsResolver` type parameter cascade.

---

## 3. `NativeHttpClient` — wrapping the existing stack

```rust
pub struct NativeHttpClient<R: DnsResolver + Clone + Send + 'static = SystemDnsResolver> {
    client: SimpleHttpClient<R>,
    resolver: R,
}
```

`NativeHttpClient` is a thin adapter with four methods:
- `send()` / `send_async()` convert `PreparedRequest` → `ClientRequestBuilder` → send
  via `SimpleHttpClient` → convert `FinalizedResponse.into_parts()` back to
  `SimpleResponse<SendSafeBody>`, dropping the connection pool handle.
- `send_sse()` / `send_sse_async()` build a `ReconnectingEventSourceTask`, execute it
  to get a `DrivenStreamIterator`, then use valtron's `map_pending(map_progress)` to
  convert `ReconnectingProgress` → `SseProgress`. The async variant additionally calls
  `into_future_stream()` to wrap the iterator as a `futures_core::Stream`.

All `SendSafeBody` variants pass through via `ClientRequestBuilder::body(SendSafeBody)`.
No body variants are rejected.

---

## 4. The browser `fetch()` API — what you need to know

`fetch()` is the standard HTTP primitive in browsers and CF Workers. It takes a
`Request` object and returns a `Promise<Response>`:

```js
const resp = await fetch(new Request(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
}));
```

Key differences from a native socket client:
- **No connection pooling** — the browser manages connections internally.
- **No DNS resolution** — the browser handles it.
- **No TLS configuration** — the browser enforces its own certificate store.
- **CORS restrictions** — cross-origin requests require server cooperation (the
  `Access-Control-*` response headers). CF Workers are exempt from CORS.
- **Streaming bodies** — `fetch()` accepts `ReadableStream` as a request body.
- **Streaming responses** — `response.body` is a `ReadableStream`.

In Rust, `web_sys` provides typed bindings:
- `web_sys::Request` / `web_sys::RequestInit` — the request
- `web_sys::Response` — the response
- `web_sys::Headers` — header map
- `web_sys::ReadableStream` / `ReadableStreamDefaultReader` — streaming body

`wasm-bindgen-futures::JsFuture` converts a `js_sys::Promise` into a Rust `Future`
so you can `.await` it.

---

## 5. Browser vs CF Workers: one client, runtime detection

Both environments implement `fetch()`, but they differ in how you access it:
- **Browser:** `window.fetch(req)`
- **CF Workers:** No `window`. The global scope is a `ServiceWorkerGlobalScope`.

The `js_fetch()` function detects the environment at runtime:

```rust
fn js_fetch(req: &Request) -> js_sys::Promise {
    let global = js_sys::global();
    if let Ok(true) = js_sys::Reflect::has(
        &global, &JsValue::from_str("ServiceWorkerGlobalScope"),
    ) {
        global.unchecked_into::<web_sys::ServiceWorkerGlobalScope>()
            .fetch_with_request(req)
    } else {
        web_sys::window().expect("no window")
            .fetch_with_request(req)
    }
}
```

This pattern was proven in `foundation_auth/src/wasm_bindgen/oauth.rs`. One
`FetchHttpClient` struct covers both environments — no separate CF Workers client.

---

## 6. `SendWrapper` — making `!Send` JS futures `Send`

`web_sys` types are `!Send` because they're backed by JS GC objects that can't move
between threads. But the `HttpClient` trait requires `Send` (for `Arc<dyn HttpClient>`).

On wasm32 there's only one thread, so the `Send` requirement is vacuously satisfied.
`foundation_compact::SendWrapper` (from F00e) wraps a `!Send` future and implements
`Send` on wasm32. The `FetchHttpClient` methods wrap their entire async body in
`SendWrapper::new(async move { ... }).await`.

---

## 7. Streaming request bodies: `SendSafeBody` → JS

`SendSafeBody` has seven variants. `FetchHttpClient` handles all of them:

| Variant | JS representation |
|---------|-------------------|
| `None` | no body (`JsValue::UNDEFINED`) |
| `Text(String)` | JS string (`JsValue::from_str`) |
| `Bytes(Vec<u8>)` | `Uint8Array` |
| `Stream(Iterator<Data>)` | JS `ReadableStream` via `iterator_to_readable_stream` |
| `ChunkedStream(Iterator<ChunkedData>)` | JS `ReadableStream` via `iterator_to_readable_stream` |
| `LineFeedStream(Iterator<LineFeed>)` | JS `ReadableStream` — lines formatted with `\n` |
| `SseStream(Iterator<ParseResult>)` | JS `ReadableStream` — proper SSE text serialization |

The streaming variants all use the same bridge: extract bytes from each item, pass a
`Box<dyn Iterator<Item = Vec<u8>>>` to `valtron::js_stream::iterator_to_readable_stream()`.

---

## 8. Valtron `js_stream` — the async↔sync bridge for wasm

Native valtron has `ThreadedIterFuture` which spawns async work on a background thread
and exposes results through a sync `Iterator` via channels. On wasm32, there are no
background threads. `js_stream` provides the equivalent using `spawn_local` +
`ConcurrentQueue`.

### 8a. JS ReadableStream → sync Iterator (`JsReadableStreamIterator`)

```
JS ReadableStream
    → spawn_local async task reads chunks via reader.read()
    → pushes JsStreamValue::Chunk(Vec<u8>) into ConcurrentQueue
    → on error, pushes JsStreamValue::Error(String)
    → sets done flag on completion

Iterator::next()
    → pops from queue: Chunk / Error / None (done) / Waiting (empty, not done)
```

Three constructors control backpressure:
- `new(stream)` — bounded queue (default 64 slots), prevents OOM
- `with_capacity(stream, n)` — bounded with custom size
- `unbounded(stream)` — no backpressure, for short-lived streams

When the bounded queue is full, the reader task holds the rejected item locally and
retries draining it before reading the next JS chunk. When the queue is closed
(consumer dropped), the reader stops immediately.

Errors flow through the same queue as data — `JsStreamValue::Error(msg)` — so the
consumer sees all buffered chunks before the error. No separate error storage needed.

### 8b. Sync Iterator → JS ReadableStream (`iterator_to_readable_stream`)

```
Box<dyn Iterator<Item = Vec<u8>>>
    → wrapped in Rc<RefCell<..>> (wasm is single-threaded)
    → pull(controller) JS closure pops from iterator
    → controller.enqueue(Uint8Array) per chunk
    → controller.close() when exhausted
    → ReadableStream with pull-based underlying source
```

This is used by `FetchHttpClient` to bridge streaming `SendSafeBody` variants into
`fetch()` request bodies.

---

## 9. SSE on wasm: `WasmSseIterator`

AI providers consume SSE as `Iterator<Item = Stream<ParseResult, SseProgress>>`.
On native, `ReconnectingEventSourceTask` drives the SSE connection with auto-reconnect.
On wasm, `WasmSseIterator` provides basic SSE (no auto-reconnect — providers handle
retries at their level).

The pipeline:

```
JS ReadableStream (response.body)
    → spawn_local reader task pushes byte chunks into ConcurrentQueue
    → QueueReader: std::io::Read adapter over ConcurrentQueue
    → SharedByteBufferStream wraps QueueReader
    → SseParser<QueueReader> parses SSE events from the byte stream
    → WasmSseIterator yields Stream<ParseResult, SseProgress>
```

`QueueReader` bridges the queue into `std::io::Read` — it returns `Ok(0)` when the
queue is temporarily empty (parser retries on next poll). `WasmSseIterator` checks
both the `done` flag and queue emptiness: if both are true, the stream is finished
(`None`); otherwise it returns `Stream::Wait` (valtron yields to JS event loop).

The `SseParser` is the **same parser** used on native — shared code in
`event_source::shared::sse`. No SSE parsing was duplicated for wasm.

---

## 10. Auth header injection

Headers are set on `PreparedRequest` before calling `HttpClient::send()`. The provider
builds the request with auth headers:

```rust
let mut req = PreparedRequest::new(SimpleMethod::POST, uri);
req.headers.add(SimpleHeader::new("Authorization", format!("Bearer {key}")));
req.headers.add(SimpleHeader::new("Content-Type", "application/json"));
```

`FetchHttpClient` converts `SimpleHeaders` → `web_sys::Headers` via
`simple_headers_to_web_sys()`, and response headers back via
`web_sys_headers_to_simple()`. The conversion is lossless for standard ASCII headers.

---

## 11. Platform-gated module wiring

The wasm client module is behind two gates:

```rust
// foundation_netio/src/simple_http/client/mod.rs
#[cfg(all(target_arch = "wasm32", feature = "wasm-fetch"))]
pub mod wasm;
```

The `wasm-fetch` Cargo feature enables:
- `js-event-loop` (wasm-bindgen + web-sys + js-sys)
- `wasm-bindgen-futures` (JsFuture, spawn_local)
- `foundation_core/js-wasmbindgen` (valtron's js_stream module)

A convenience function selects the right client at compile time:

```rust
#[cfg(all(feature = "multi", not(target_arch = "wasm32")))]
pub fn default_http_client() -> Arc<dyn HttpClient> { Arc::new(NativeHttpClient::default()) }

#[cfg(all(target_arch = "wasm32", feature = "wasm-fetch"))]
pub fn default_http_client() -> Arc<dyn HttpClient> { Arc::new(FetchHttpClient) }
```

Providers call `default_http_client()` and get whichever backend matches the target.

---

## 12. Verification commands

```bash
# Native build + tests
cargo check -p foundation_netio
cargo test -p foundation_netio --lib

# Wasm build (no tests — needs browser/deno runtime)
cargo check -p foundation_netio --target wasm32-unknown-unknown \
    --no-default-features --features "std,wasm-fetch"

# Clippy
cargo clippy -p foundation_netio --all-targets -- -D warnings
```
