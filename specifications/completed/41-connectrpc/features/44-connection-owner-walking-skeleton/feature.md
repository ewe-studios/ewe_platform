---
feature: "Connection-owner walking skeleton — end-to-end spine over a real socket (D11 §Connection ownership)"
description: "Server per-connection pump (Serve adapter) + client open() pump + one unary and one streaming RPC over loopback TCP; lands PushableRequestBody::into_sender"
status: "complete"
priority: "critical"
phase: 1
depends_on: ["17-transport-seam", "22-router-dispatch", "07-pushable-request-body", "10-reactor-parking"]
estimated_effort: "large"
created: 2026-07-05
updated: 2026-07-11
---
# Feature 44-connection-owner-walking-skeleton: end-to-end spine over a real socket

## Completion status (2026-07-11)

**F44 is complete.** The walking-skeleton spine is proven end-to-end over a real
loopback socket, on both the server and the H1 **client** owner, for **unary and
server-streaming** RPCs.

> **Design note — this feature shipped its client seam through
> [Feature 45](../45-delivery-and-split-backpressure/feature.md) Part D, not the
> `TransportPump` shape sketched in the *H1Transport pump design* section below.**
> That section is kept for historical context; the *authoritative* shape is:
> `Transport::open()` builds an `HttpExchangeTask` and fans its `HttpExchange`
> output out with valtron's native splits (`split_collect_until_map` for the head,
> `split_collector_map` for the body), spawns **one** drive task via
> `valtron::send()`, and returns a **four-half** `TransportStream`
> (`send_body: Arc<dyn SendBody>`, `head: HeadStream`, `recv_body: BodyStream`,
> `trailers`). There is no `TransportPump` type, and `head`/`recv_body` are erased
> `futures::Stream`s carrying `Result<_, TransportError>`, not
> `PipeReceiver<SimpleResponse<()>>`. See the *Acceptance criteria* below, which
> have been updated to the shipped shape.

Delivered and verified:

1. ✅ **`SendSafeBodyBytesIterator`** in
   `simple_http/client/shared/body_reader.rs`, with an extensive `#[test]` suite
   covering every `SendSafeBody` variant + error propagation + exhaust idempotency.
2. ✅ **`HttpExchange` / `HttpExchangePending`** shared types in
   `client/shared/request_task.rs`.
3. ✅ **Native `HttpExchangeTask`** (`client/native/tasks/http_exchange_task.rs`)
   with tests in `tests/simple_http/http_exchange_task_tests.rs` (Head→BodyChunk
   success, pre-head `Failed`, and `HttpClientConnection` pool-return via `Drop`).
4. ✅ **WASM `WasmHttpExchangeTask`** (`client/wasm/tasks/http_exchange_task.rs`) —
   wraps `FetchHttpClient::send_async()` in a `FutureTask` via `from_future`,
   yields `HttpExchange::Head` then `BodyChunk`. Compiles clean on
   `wasm32-unknown-unknown --features wasm-fetch`; compile-time smoke test included.
5. ✅ **Server owner** `ConnectRpcServe` implements foundation_http `Serve`.
6. ✅ **Client owner** `H1Transport::open()` returns `TransportStream`
   synchronously (four halves, no `BoxFuture`).
7. ✅ **End-to-end over real loopback TCP** (`tests/server_socket_tests.rs`,
   `#[valtron_test]`, `--profile uat`): `unary_round_trip_over_real_socket` and
   `server_stream_over_real_socket` pin the server via raw TCP;
   `h1_client_transport_over_real_socket` (unary) and
   `h1_client_server_stream_over_real_socket` (3-frame server stream + EndStream)
   prove the H1 **client** owner. All pass.

Deferred (unchanged): **incremental streaming `WriteBody`** →
[Feature 23](../23-h1-transport/feature.md). Streaming *uploads* need
`Depends(pipe-readiness)` composing cancel; the unary and server-stream paths
proven here do not exercise it.

## Why this exists (sequencing correction)

Features 17-22 built the RPC middle (seam, protocols, router) as isolated units tested against
**stubbed** connection owners (F22's `feeder`/`collector`, in-memory `block_on`). The **connection
owner** — the component holding the raw fd that spawns the byte pump owning the socket-facing pipe
halves — was never laid down as a first, load-bearing spine, so the ownership contract kept
resurfacing as ambiguity at each layer boundary (most sharply in F23's `open`). This feature builds
the thin vertical slice that makes the Decision 11 §Connection ownership contract concrete, so
F23/F24 and every later transport slot into a **proven** path.

## Normative sources (read before writing code)

- decisions/11-transport-seam.md — §Connection ownership (both sides), §Duplex table, §who-owns-enveloping
- decisions/08-router-and-dispatch.md — §Integration with foundation_http, Decided Detail 0 (server connection owner)
- decisions/07-client-architecture.md — Decided Details (connection ownership / reuse)

## Scope

- **netio addition 1 — `PushableRequestBody::into_sender(self) -> PipeSender<Bytes>`** so the pushable
  body's pipe *is* the seam `send_body` (no bridge task, no copy). ✅ DONE

- **netio addition 2 — `SendSafeBodyBytesIterator` in `body_reader.rs`** — an
  `Iterator<Item = Stream<Bytes, BoxedError>>` that wraps a `SendSafeBody` and yields
  `Stream::Next(Bytes)` chunks for each body variant (`Bytes` → one `Next`, `Text` → one `Next`,
  `Stream` → each `Data::Bytes`, `ChunkedStream` → each `ChunkedData::Data`, `SseStream` →
  each `Event::Message`, `LineFeedStream` → each `LineFeed::Line`). `Data::Retry` /
  `ChunkedData::Trailers` / `Event::Comment` / `LineFeed::SKIP` → `Stream::Ignore`. Errors →
  `Stream::Next(Err(…))`. Shared by both native and WASM paths.

- **netio addition 3 — `HttpExchange` + `HttpExchangePending` in `client/shared/request_task.rs`** —
  the `Ready` and `Pending` types that both platform `TaskIterator` impls yield. These live in
  `shared/` so the transport pump (in connectrpc) only depends on shared types.

  | Platform | Impl | Location | What it wraps |
  |---|---|---|---|
  | Native | `HttpExchangeTask` | `client/native/tasks/http_exchange_task.rs` | `SendRequestTask<R>` via `inlined_task`, polls child → `RequestIntro::Success` → takes `HttpClientConnection` and `HttpResponseReader` into task state → `HttpExchange::Head` + body via `SendSafeBodyBytesIterator`. **Cleanup:** `HttpClientConnection` is held in task state; on `Done`/`Failed`/`drop`, the connection goes back to `HttpConnectionPool` automatically via `HttpClientConnection`'s `Drop` impl. `HttpResponseReader`'s drop closes the underlying stream. Exports `new_http_exchange_task(request, pool, config)`. |
  | WASM | `HttpExchangeTask` | `client/wasm/http_exchange_task.rs` | `build_web_request()` + `do_fetch()` (existing in `wasm/client.rs`) via `from_future` + `FutureTask`. On `Ready`: `web_sys_headers_to_simple()` → `HttpExchange::Head`. Body via `resp.text()` → `SendSafeBody::Text` → `SendSafeBodyBytesIterator` → `HttpExchange::BodyChunk`. Exports `new_http_exchange_task(request)`. |

- **Server connection owner:** `ConnectRpcServe` implements foundation_http `Serve`. ✅ DONE

- **Client connection owner — `H1Transport::open()`** (shipped shape; F45 Part D).
  `open()` builds the platform `HttpExchangeTask`, then fans its `HttpExchange`
  output out with valtron's native splits rather than a bespoke pump task:
  `split_collect_until_map` peels the head (or a pre-head `Failed`) into a
  `HeadStream`, `split_collector_map` peels body chunks (or a mid-body `Failed`)
  into a `BodyStream`, and the remaining continuation — terminated with
  `map_ready(|_| ())` so body chunks are not re-buffered — is the **one** task
  spawned via `valtron::send()`. `open()` returns `TransportStream`
  **synchronously**. (The earlier `TransportPump` design in *H1Transport pump
  design* below was superseded by this split-based shape.)

## H1Transport pump design (2026-07-05) — SUPERSEDED, historical

> **This section describes the original `TransportPump` sketch. It was superseded
> by the split-based `open()` shape (F45 Part D) documented in *Completion status*
> and *Scope* above. `TransportPump` does not exist in the code; `open()` uses
> `HttpExchangeTask` + `split_collect_until_map` / `split_collector_map`, and
> `TransportStream` has four halves whose `head`/`recv_body` are erased
> `futures::Stream`s. Read the rest of this section only for the original intent.**

### Shared types in `client/shared/request_task.rs`

The `Ready` and `Pending` types live in `shared/`. Each platform provides its own
`TaskIterator` impl in its own directory.

```rust
// backends/foundation_netio/src/simple_http/client/shared/request_task.rs

/// Ready type shared by both platform impls.
pub enum HttpExchange {
    /// Response head. Exactly once per request. Uses the existing shared type.
    Head(SimpleResponse<()>),
    /// One chunk of response body bytes. Zero or more.
    BodyChunk(Bytes),
    /// The request failed before or during the response — no Head was produced.
    Failed(BoxedError),
}

pub enum HttpExchangePending { Waiting }
```

### TransportPump (TaskIterator wrapping HttpExchangeTask)

```text
TransportPump state:
  Init                    → create HttpExchangeTask, spawn via inlined_task
  AwaitingChild(receiver) → poll receiver
    → Stream::Next(HttpExchange::Head(head))
      → head_tx.try_send(head)   // SimpleResponse<()> = status + headers
    → Stream::Next(HttpExchange::BodyChunk(bytes))
      → recv_tx.try_send(bytes)
    → Stream::Next(HttpExchange::Failed(_))
      → close both pipes, Done
    → Stream::Pending(_) → Depends(QueueReadiness)
    → None → close recv_tx, Done
```

The pump never touches `RequestIntro`, `HttpResponseReader`, `HttpClientConnection`,
or any native-specific type. It only sees `HttpExchange`.

### Output pipes returned to caller

| pipe | type | direction |
|---|---|---|
| `send_body` | `PipeSender<Bytes>` | caller pushes request bytes → pushable body pipe drains to HTTP request |
| `head` | `PipeReceiver<SimpleResponse<()>>` | pump pushes exactly one response head; `SimpleResponse<()>` is status + headers, no body |
| `recv_body` | `PipeReceiver<Bytes>` | pump pushes body chunks via `SendSafeBodyBytesIterator` |

### Complete data flow

```
caller                            TransportPump               HttpExchangeTask (child)
──                                ───────────────────         ─────────────────────────
send_body.try_send(envelope)      Spawn → HttpExchangeTask    Native: wraps SendRequestTask
                                  ↓                           WASM: wraps fetch()
                                  poll child receiver
                                    HttpExchange::Head
                                      → head_tx.try_send()
                                    HttpExchange::BodyChunk
                                      → recv_tx.try_send()
                                    HttpExchangePending
                                      → Depends
                                    None → close recv_tx
```

**No `start()`, no `send_async()`, no `from_future` at the pump level, no `block_on`,
no `BoxFuture`, no `std::thread::spawn`, no `try_collect_bytes` buffering.**

## Out of scope

- Full client core ergonomics (F24), auth (F25), codegen (F26/27), HTTP/2+ (F29+).
- Pool-reuse assertion detail and Fetch capabilities (stay in F23).

## Acceptance criteria

- `SendSafeBodyBytesIterator` lands in `body_reader.rs` with tests in
  `backends/foundation_netio/tests/simple_http/` covering all 6 body variants
  (`Bytes`, `Text`, `Stream`, `ChunkedStream`, `SseStream`, `LineFeedStream`,
  `None`) + error propagation.

- `HttpExchange` + `HttpExchangePending` types land in `client/shared/request_task.rs`.

- Native `HttpExchangeTask` in `client/native/tasks/http_exchange_task.rs` with tests in
  `backends/foundation_netio/tests/simple_http/` covering:
  - `Head` → `BodyChunk*` → exhaust (success path)
  - `Failed` → no head, pipes closed (connect error, DNS failure)
  - `HttpClientConnection` returned to pool on task completion/drop

- WASM `HttpExchangeTask` in `client/wasm/http_exchange_task.rs` with a compile-time
  smoke test (full browser integration stays in F23).

- `Transport::open()` returns `Result<TransportStream, TransportError>`
  synchronously — no `BoxFuture`. **Shipped shape (F45 Part D):** `TransportStream`
  has four halves — `send_body: Arc<dyn SendBody>`, `head: HeadStream`,
  `recv_body: BodyStream`, `trailers: PipeReceiver<SimpleHeaders>`. `head` and
  `recv_body` are erased `futures::Stream`s yielding
  `Result<(Status, SimpleHeaders), TransportError>` and `Result<Bytes, TransportError>`
  respectively (a pre-head/mid-body failure rides the payload as `Err` — no silent
  drop), replacing both the old `response: BoxFuture<…>` field and the
  `PipeReceiver<SimpleResponse<()>>` head sketched earlier. ✅ DONE

- Server: `ConnectRpcServe` per-connection task owns fd, dispatches, streams response. ✅ DONE

- Client: `H1Transport::open()` builds an `HttpExchangeTask`, fans it out with
  `split_collect_until_map` (head) + `split_collector_map` (body), and spawns the
  single drive task via `valtron::send()`. The caller drains the four halves
  directly — no `futures_lite::block_on`, no `TransportPump`. ✅ DONE

- End-to-end over real loopback TCP: unary + server-stream + H1Transport client RPC.
  `#[valtron_test]`, `--profile uat` (default `rpc_multi` feature). Proven by
  `unary_round_trip_over_real_socket`, `server_stream_over_real_socket` (server via
  raw TCP), `h1_client_transport_over_real_socket` (unary via H1 client), and
  `h1_client_server_stream_over_real_socket` (3-frame server stream via H1 client). ✅ DONE
