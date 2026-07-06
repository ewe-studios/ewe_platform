---
feature: "Connection-owner walking skeleton — end-to-end spine over a real socket (D11 §Connection ownership)"
description: "Server per-connection pump (Serve adapter) + client open() pump + one unary and one streaming RPC over loopback TCP; lands PushableRequestBody::into_sender"
status: "complete"
priority: "critical"
phase: 1
depends_on: ["17-transport-seam", "22-router-dispatch", "07-pushable-request-body", "10-reactor-parking"]
estimated_effort: "large"
created: 2026-07-05
updated: 2026-07-06
---
# Feature 44-connection-owner-walking-skeleton: end-to-end spine over a real socket

## Completion status (2026-07-06)

**F44 is complete.** All three tracked items delivered:

1. ✅ **WASM `HttpExchangeTask`** (`wasm/tasks/http_exchange_task.rs`) — wraps `FetchHttpClient::send_async()` in a `FutureTask`, polls cooperatively via `from_future()`, yields `HttpExchange::Head` then `BodyChunk` chunks. Compiles clean on `wasm32-unknown-unknown --features wasm-fetch`. Compile-time smoke test included.
2. ✅ **Direct `SendSafeBodyBytesIterator` tests** — 15 unit tests in `body_reader.rs` covering all 7 `SendSafeBody` variants (`Bytes`, `Text`, `None`, `Stream`, `ChunkedStream`, `LineFeedStream`, `SseStream`) + error propagation + exhaust idempotency.
3. ✅ **`HttpClientConnection` pool-return** — **Bug fix**: `HttpExchangeTask` was dropping connections on all 4 exit paths instead of returning them to the pool. Added `connection_count()` introspection to `ConnectionPool`/`HttpConnectionPool`. Integration test verifies pool size 0→1 after exchange completes.
4. 🔄 **Incremental streaming `WriteBody`** — *deferred to F23 (h1-transport-client)*. Today's unary path doesn't hit it; streaming uploads need `Depends(pipe-readiness)` composing cancel.

The walking-skeleton spine remains proven over a real loopback socket.

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

- **Client connection owner — TransportPump:** a single `TaskIterator` that creates the platform
  `HttpExchangeTask`, spawns it via `inlined_task` + `TaskStatus::Spawn`, then forwards
  `HttpExchange::Head` → `head_tx` and `HttpExchange::BodyChunk` → `recv_tx`.
  `open()` creates the pump and the three output pipes, spawns via `valtron::send()`, and
  returns `TransportStream` **synchronously**.

## H1Transport pump design (2026-07-05)

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

- `Transport::open()` returns `Result<TransportStream, TransportError>` synchronously — no
  `BoxFuture`. `TransportStream.head: PipeReceiver<SimpleResponse<()>>` replaces the old
  `response: BoxFuture<…>` field. Uses `SimpleResponse<()>` (`no_body`) which is already a
  shared type in `foundation_netio::simple_http::shared`.

- Server: `ConnectRpcServe` per-connection task owns fd, dispatches, streams response. ✅ DONE

- Client: `TransportPump` spawned via `valtron::send()`; three caller-facing pipe halves
  (`send_body`, `head`, `recv_body`). The caller polls them directly — no `futures_lite::block_on`.

- End-to-end over real loopback TCP: unary + server-stream + H1Transport client RPC.
  `#[valtron_test]`, `--profile uat`, `--features multi`.
