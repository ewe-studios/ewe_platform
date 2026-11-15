# Wasm HTTP Streaming: the design

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** WebAssembly (wasm); Hypertext Transfer Protocol (HTTP);
Hypertext Transfer Protocol version 2 (HTTP/2); remote procedure call (RPC); Cloudflare
Durable Object (DO); JavaScript (JS).

This page gives the detail of section 4 of [requirements.md](./requirements.md). The first
draft of this specification added a `Fetcher`, a `FetchTransport`, a `FetchServe` and a body
pump. The owner rejected that on 2026-11-15: "the core idea is `DynNetClient` is the platform
agnostic way, and connectrpc already builds on it." This page records the design that follows
that direction.

## 1. One client abstraction

| Layer | Type | File |
|---|---|---|
| The trait | `HttpClient: Send + Sync` | `foundation_netio/src/shared/client/http_client.rs:65` |
| The handle | `DynNetClient = Arc<dyn NetClient>` | `foundation_netio/src/network_client.rs:41` |
| The native implementation | `NativeHttpClient` | `foundation_netio/src/http/http_client_impl.rs:50` |
| The wasm implementation | `FetchHttpClient`, which also implements `WebSocketConnector` | `foundation_netio/src/wasm/client/client.rs:56` |
| The choice | `HttpClientBuilder::build()` | `foundation_netio/src/network_client.rs:440` |
| The RPC transport | `H1Transport`, which holds a `DynNetClient` and has no target gate | `foundation_connectrpc/src/shared/transport/h1.rs:61` |

`H1Transport::open` already writes a streaming request body:
`pushable_request_body_with_readiness` gives a `SendSafeBody::Stream`, and on wasm
`send_safe_body_to_js` turns that stream into a JS `ReadableStream` through
`valtron::js_stream::iterator_to_readable_stream`
(`foundation_netio/src/wasm/client/client.rs:395`). So a streamed request works today on both
targets.

`foundation_http` owns the serving side: `HttpApp<Arc<dyn CfServe>>`,
`HttpApp<Arc<dyn WebServe>>`, `dispatch_cf`, `dispatch_web`, `CfConn`, `WebConn`, and the two
bridges (`foundation_http/src/wasm/`). This specification does not add a router or a second
serving stack.

## 2. Gap 1: the wasm client reads the whole response

Today `fetch_and_read` awaits `resp.text()`
(`foundation_netio/src/wasm/client/client.rs:334`). Every response is in memory before
`send_async` returns, with no bound (`max_body_size` defaults to `None`).

The streaming read is not missing from the workspace. The SSE path already does it:
`fetch_sse` takes `resp.body()` and the reader task in
`foundation_netio/src/wasm/client/stream.rs:44` pushes each `Uint8Array` into a
`ConcurrentQueue` and fires a `WakeRegistry`. T1 moves that reader into one shared type and
uses it from both paths.

```rust
// foundation_netio/src/wasm/client/stream.rs (extend the module that exists)

/// The bytes of one wasm response, read one chunk at a time.
///
/// The JS reader lives inside the spawned task; this type holds `Arc` state
/// only, so it is `Send` with no `unsafe impl` and no `SendWrapper`.
pub struct WasmByteStream { /* queue, done, error, wake, cancelled */ }

impl WasmByteStream {
    /// At most `chunks` queued ahead of the consumer; stop after `max` bytes.
    pub fn new(stream: web_sys::ReadableStream, chunks: usize, max: Option<usize>) -> Self;
    /// The same bytes as `std::io::Read`, for the SSE parser.
    pub fn reader(&self) -> QueueReader;
}

impl Iterator for WasmByteStream {
    /// `Bytes` a chunk, `Retry` nothing yet (`Stream::Wait` for the caller),
    /// `Err` a read error or a body over `max`, `None` at the end.
    type Item = Result<Data, BoxedError>;
}

impl Drop for WasmByteStream {
    /// Set `cancelled`; the task calls `reader.cancel()` and closes the queue.
    fn drop(&mut self);
}
```

`fetch_and_read` then reads the head first and builds the body from `resp.body()`:

```rust
let status = Status::from(resp.status().to_string());
let headers = web_sys_headers_to_simple(&resp.headers());
let body = match resp.body() {
    Some(stream) => SendSafeBody::Stream(Some(Box::new(WasmByteStream::new(
        stream,
        config.body_queue_chunks,
        config.max_body_size,
    )))),
    None => SendSafeBody::None, // 204, 304, HEAD
};
Ok(SimpleResponse::new(status, headers, body))
```

`body_queue_chunks` is a new `ClientConfig` field, default 16. A browser chunk is 16 KiB to
64 KiB, so the bound is 256 KiB to 1 MiB. `max_body_size` becomes a real limit on wasm: the
reader counts the bytes it hands out and stops with an `HttpClientError` that names the
limit. Its default stays as it is today.

Two details of the web platform decide the shape:

- `Response::body()` is `None` when there is no body. That is not an error, and it must not
  become an empty `Text` body.
- `cancel()` is the only way to release a connection early. The reader calls it in `Drop`, so
  a caller that drops the stream stops the transfer.

The `byob` reader (`ReadableStreamBYOBReader`) would save one copy per chunk. `web-sys` 0.3
does not expose it everywhere, and the copy is not the cost that matters here. It is not in
this specification.

After this change `H1Transport` streams on wasm with no change of its own, and the wasm
`open_exchange` drains a streaming body instead of a `SendSafeBody::Text`.

## 3. Gap 2: the exchange task ignores its client

`FetchHttpClient::open_exchange` builds `WasmHttpExchangeTask::new(req)`
(`foundation_netio/src/wasm/client/client.rs:166`), and that task calls
`fetch_via_client`, which builds a **new** `FetchHttpClient`
(`foundation_netio/src/wasm/client/tasks/http_exchange_task.rs`). So `H1Transport::new(client)`
on wasm never calls `client`.

The design: `open_exchange` passes `self`, and the task calls `HttpClient::send_async` on that
client. A DO stub client, a proxy client or a recording test client then works as it does on
native.

The capability constants follow the code: fetch streams a request body, so
`WASM_FETCH_CAPABILITIES.request_streaming` is `true`. `full_duplex`, `h2_trailers` and
`multiplexed` stay `false`.

## 4. Gap 3: the response builder has no status

`streaming_response` always answers 200 (`foundation_netio/src/wasm/response.rs:33`), and
`sse_response` does too (`:91`). A Connect error needs its own status.

```rust
pub fn streaming_response_with<S>(status: u16, headers: &SimpleHeaders, body: S)
    -> Result<web_sys::Response, JsValue>
where
    S: Stream<Item = Vec<u8>> + 'static;
```

The body is pulled one chunk at a time. A cancel drops the source. `streaming_response` and
`sse_response` keep their signatures and call it.

`foundation_http` already has two served-side builders, and both build a `Vec<u8>`:
`CfConn::into_response` (`foundation_http/src/wasm/cf_conn.rs:64`) and `WasmResponse` with
`build_response` and `from_wasm` (`foundation_http/src/wasm/response.rs:5`). This
specification adds no third one. The sink in section 5 uses `streaming_response_with`, because
it is the only builder that takes a stream.

## 5. Gap 4: the inbound body and the sink

`foundation_http/src/wasm/bridge/web.rs` builds a `SimpleIncomingRequest` with `req.text()`.
The bridge reads `req.body()` as a `ReadableStream` instead and gives a
`SendSafeBody::Stream`, bounded by a config field. `bridge/cf.rs` does the same for the
Cloudflare connection.

The serving side of connectrpc needs one adapter: a `ResponseSink`
(`foundation_connectrpc/src/shared/router/dispatch.rs:375`) over a streaming response.

| Method | What it does |
|---|---|
| `whole` | `streaming_response_with` over a one-chunk stream |
| `open` | `streaming_response_with` from the head, and the stream waits for `chunk` |
| `chunk` | one write into the stream |
| `finish` | close the stream |

A handler is polled only when the runtime asks for the next chunk, so a slow client slows the
handler and memory stays flat. A dropped response drops the sink and stops the handler.

Two rules make that true:

- **The pull parks.** `iterator_to_readable_stream` (`js_stream.rs:225`) takes one item per
  pull. A source with no chunk yet answers `Data::Retry`. A loop over `Retry` freezes the one
  wasm thread (root rule 6). The pull registers a waker on the queue's `WakeRegistry` and
  returns a promise that resolves on the next push.
- **A task, if one is needed, is a `#[valtron::task]`.** For example, a task that feeds the
  queue from the handler is an `async fn` transition over an enum state with a terminal
  `Done` arm: `async fn feed(ctx: Ctx, state: Feed) -> Option<(Feed, TaskStatus<R, P>)>`. Use
  `feed::new(state)`. Do not write a struct with `impl TaskIterator`; the functional form has
  less noise and is easier to test and to reason about.

## 6. The Durable Object client

The Worker side of a call to a DO is a `NetClient`, not a new transport:

```rust
// foundation_netio/src/cloudflare/stub_client.rs   (wasm + feature "cloudflare")
pub struct StubHttpClient { /* SendWrapper<Rc<worker::Stub>> */ }

impl HttpClient for StubHttpClient { /* stub.fetch_with_request */ }
impl WebSocketConnector for StubHttpClient { /* Unsupported */ }
```

A Worker builds `H1Transport::new(Arc::new(StubHttpClient::new(env, binding, name)?))`. The
transport, the streams and the error mapping are the ones of every other call. The forwarded
identity headers stay with the caller: it adds them to the request that it sends.

## 7. Errors

| Case | Result |
|---|---|
| A response body larger than `max_body_size` | `HttpClientError` that names the limit, after the head |
| A fetch that fails | `HttpClientError::Reason` with the JS error |
| A request that needs full duplex or HTTP/2 trailers | `TransportError` from `check_compatible`, before any fetch |
| gRPC (`application/grpc`) at a fetch server | `UNIMPLEMENTED`, "gRPC needs HTTP/2 trailers" |
| A client that goes away | the source is dropped; the handler sees the call end |

## 8. What a consumer must not do

- Do not add a client trait, a transport or a body pump. Use `HttpClient`, `DynNetClient` and
  `H1Transport`.
- Do not read a body with `text()` or `array_buffer()`.
- Do not build a client inside a task. Pass it.
- Do not write a second `SendWrapper`, and do not use `unsafe impl Send`.
- Do not set CORS in a library. The host sets it.
