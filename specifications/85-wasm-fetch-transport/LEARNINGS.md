# LEARNINGS — Wasm HTTP Streaming

## The owner's direction: use the type that exists (2026-11-15)

The first draft of this specification built six types: `Fetcher`, `GlobalFetcher`,
`FetchTransport`, `FetchServe`, `StubFetcher` and `DoStubTransport`. The owner read it and
said: "the core idea is `DynNetClient` is the platform agnostic way", and "connectrpc already
builds on it".

Both statements are true, and the audit proves them:

| Need | The type that already exists | Where |
|---|---|---|
| A client that works on native and on wasm | `HttpClient: Send + Sync` | `foundation_netio/src/shared/client/http_client.rs:65` |
| One handle to hold it | `DynNetClient = Arc<dyn NetClient>` | `foundation_netio/src/network_client.rs:41` |
| The choice of implementation per target | `HttpClientBuilder::build()` | `foundation_netio/src/network_client.rs:440` |
| The RPC transport over it | `H1Transport`, in `shared/`, no target gate | `foundation_connectrpc/src/shared/transport/h1.rs:61` |
| A streamed request body on wasm | `SendSafeBody::Stream` → `ReadableStream` | `foundation_netio/src/wasm/client/client.rs:395` |
| The wasm serving stack | `HttpApp`, `CfServe`, `CfConn`, the bridges | `foundation_http/src/wasm/` |
| The pull side of a served response | `ResponseSink` with `whole`, `open`, `chunk`, `finish` | `foundation_connectrpc/src/shared/router/dispatch.rs:375` |

Six new types would have added a second copy of each row. The lesson: before writing a plan,
search the workspace for the type that already does the job. A "platform seam" is often
already built, and a second seam on top of it hides the first.

## Why the request body is not collected (2026-11-15)

An earlier draft of this file said that fetch cannot stream a request body. That is wrong.
`send_safe_body_to_js` maps `SendSafeBody::Stream`, `ChunkedStream`, `LineFeedStream` and
`SseStream` to a JS `ReadableStream` through
`valtron::js_stream::iterator_to_readable_stream`
(`foundation_netio/src/wasm/client/client.rs:395`,
`foundation_core/src/valtron/executors/wasm/wasm_bindgen/js_stream.rs:225`).

So `H1Transport` streams its request on both targets with no change of its own, and
`WASM_FETCH_CAPABILITIES.request_streaming = true` is the correct value. The stale side is the
constant (`foundation_connectrpc/src/shared/transport/wasm.rs:17`), not the code.

The mistake stayed in the plan for a day because a claim about the platform went unmeasured.
Measure the platform call before you write a rule about it.

## The streaming response read already exists on the SSE path (2026-11-15)

`fetch_and_read` awaits `resp.text()` (`wasm/client/client.rs:334`), so `send_async` reads a
whole body on wasm. But `fetch_sse` (`client.rs:186`) already takes `resp.body()` and gives it
to `WasmSseIterator`, and `spawn_stream_reader` (`wasm/client/stream.rs:44`) reads the
`ReadableStream` through a `ReadableStreamDefaultReader` into a `ConcurrentQueue`, with a
`WakeRegistry` that wakes a parked valtron stream.

So the gap is not a missing capability. The gap is that one path does not use the capability
the other path has. T1 extracts the reader into `WasmByteStream` and uses it from both.

Two limits of the current SSE reader that T1 corrects:

- `WasmSseIterator::new` builds an **unbounded** queue. A slow consumer grows memory without a
  bound. T1 adds `ClientConfig::body_queue_chunks`, default 16.
- No code path cancels the JS reader. T1 calls `reader.cancel()` from `Drop`, so a dropped
  response stops the transfer.

## `iterator_to_readable_stream` drops a waiting item (2026-11-15)

The write direction calls the source iterator once per JS pull and closes the stream on
`None` (`js_stream.rs:225`). A valtron source that has nothing yet answers `Data::Retry`. The
call sites in `send_safe_body_to_js` `filter_map` it away and call `next()` again in the same
synchronous pull. On a one-thread wasm build that is a busy loop, and root rule 6 forbids it.

A request body is safe today because it is fed by a bounded pushable body that has a chunk
ready for each pull. A served response body is not: the handler may not have written one. So
the response builder must park instead of spin: register a waker on the queue and resolve the
pull promise when the next chunk arrives. This is a rule in section 5 of `requirements.md`
and a condition of T3 and T4.

## Why a valtron task is a `#[valtron::task]` (owner rule 2026-11-15)

The owner asked for the functional form: "if at all we write a valtron task i expect them to
follow the functional approach of `#[valtron::task]`". A task is an `async fn` state
transition:

```rust
#[task]
async fn pump(ctx: Ctx, state: Pump) -> Option<(Pump, TaskStatus<Item, ()>)> {
    match state {
        Pump::Done => None,          // terminal
        // …
    }
}
```

It is built with `pump::new(state)` and driven by `execute` or a stream adapter. The reasons
the owner gives are the ones that matter: less noise, easier to read, to test and to reason
about, and it fits an async-first system. A hand-written struct with `impl TaskIterator` and a
`Ctx` field is the old shape; do not add one.

`Ready` is not terminal. Only `None` ends a task. Use an enum with a `Done` arm, not a `bool`
flag.

## Why `SendWrapper` is sound here (2026-11-15)

`TransportStream` holds `Arc<dyn SendBody>`, so the exchange must be `Send`. JS values are not
`Send`. wasm has one thread, so a value that never leaves its thread may claim `Send`.

`foundation_compact::SendWrapper` has `unsafe impl Send` and `unsafe impl Sync` under
`cfg(all(target_family = "wasm", not(target_os = "emscripten"), not(target_feature = "atomics")))`
(`backends/foundation_compact/src/send_wrapper.rs`). On every other target it is
`#[repr(transparent)]` with no assertion, so the bounds come from the inner type. It has **no
run-time check**: it does not panic on a drop from another thread, and it cannot, because the
target it is for has one thread. Do not describe it as an enforcement mechanism. The protection
is compile-time: a build with threads (native, emscripten, wasm with `atomics`) has no impl, so
wrapping a `!Send` value there does not compile.

A better solution, where it applies: keep the JS handle inside the spawned task and let the
iterator hold only `Arc` state. `WasmSseIterator` does this, and so does `WasmByteStream`. Then
the type is genuinely `Send`, with no wrapper and no `unsafe impl`.

## Why the DO stub client takes headers from the caller (2026-11-15)

The first design put `ewe-forwarded-principal` inside `StubFetcher`. That makes
`foundation_connectrpc` know the agent RPC protocol. Spec 77 feature 07 needs a different
header (its token) on the same transport.

`StubHttpClient` is a `NetClient` and nothing more. Each host adds its own header at the place
that knows the protocol. The cost is one line in each host.

## Dead ends (2026-11-15)

| Attempt | Why it failed |
|---|---|
| `FetchTransport<F>` | `Transport` requires `Send + Sync + 'static` and `Rc<F>` is not, so it needed `SendWrapper`; and `H1Transport` already solves exactly this, in `shared/`, for both targets. |
| A `Fetcher` trait over `web_sys::Request` | `HttpClient` already takes a `PreparedRequest` and produces a `web_sys::Request` on wasm. The trait's only job was to copy fields. |
| `FetchServe` in `foundation_connectrpc` | `foundation_http` already owns the wasm serving stack, the router, the middleware, `CfConn` and the bridges. A second stack needs a public re-export of every internal type. |
| A second router for Connect paths | `foundation_http`'s router matches `/:method` and already runs the middleware chain. |
| Reading a body with `text()` or `array_buffer()` | Rule 4. It is also the current defect of `send_async` (#557). |

## What a consumer must not do

1. Read a whole response body. Read the stream, or set a bound and say why.
2. Add a client trait, a transport or a body pump. Use `HttpClient`, `DynNetClient`,
   `H1Transport` and `foundation_http`.
3. Build a client inside a task. Pass the client in.
4. Set a CORS header in a library. CORS is the policy of one host.
5. Write a task as a struct with `impl TaskIterator`. Write a `#[valtron::task]` `async fn`.
6. Depend on the `cloudflare` feature in a browser build.
