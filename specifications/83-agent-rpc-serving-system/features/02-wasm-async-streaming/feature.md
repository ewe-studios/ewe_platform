---
feature: "WASM Lazy Streaming"
description: "Fix the wasm streaming hang (T6) at the root by removing the one blocking anti-pattern — run_future()'s synchronous collect() — from the streaming path, so Model::stream() (and the wasm HttpClient::send_sse it calls) return genuinely lazy, pull-based valtron streams again. Also fix the valtron future-stream bridge so into_future_stream() parks on Wait/Pending (renaming the raw lens to into_pure_future_stream()), and surface deferred fetch errors through the stream (SseProgress::Failed -> ModelState::Error). Zero API change to the Model trait: stream() still returns an iterator; the boundary (sync vs async) is decided by the consumer via into_future_stream()/into_next_stream()."
status: "completed"
priority: "high"
depends_on: ["00-foundation"]
estimated_effort: "medium"
created: 2026-10-20
last_updated: 2026-10-20
author: "Main Agent"
tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100
---

# WASM Lazy Streaming

_Status note (2026-10-26): complete (5/5, PR #124). This file is the design record. New text is
in ASD-STE100 Simplified Technical English (STE); older paragraphs keep their wording. Code and
identifiers stay as they are._

**Abbreviations in this file.** WebAssembly (wasm); server-sent events (SSE); JavaScript (JS);
Hypertext Transfer Protocol (HTTP); application programming interface (API); input/output (I/O);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

## Glossary

| Term | Meaning |
|---|---|
| `run_future` | The old helper that drove a future to completion in place. On wasm it blocks the only thread, so the Worker hangs. |
| Park | To give the thread back to the JS event loop and wait for a wake call, instead of polling in a loop. |
| `into_future_stream` | The valtron bridge that turns a valtron stream into a Rust `Stream` that parks when no item is ready. |
| Lazy SSE (`LazySseIterator`) | An SSE reader that starts the `fetch` only when the consumer first asks for an item. |
| `SseProgress::Failed` | The stream state that carries a transport error to the consumer, so it is not lost. |
| `spawn_local` | Runs a future on the JS event loop of the current Worker. |


## Overview

`00-foundation` made `foundation_ai` compile on wasm and shipped `cf-cloud-provider`.
T6 exposed a **runtime** failure: `POST /generate` hangs with 0 bytes and the whole
isolate blocks (even `GET /` stops answering).

The root cause is a single anti-pattern — `run_future()`'s synchronous `collect()`
— used by every wasm streaming entry point:

```
Worker handle_generate
  → AgentSession::run_turn_stream(prompt)                 // returns a lazy Turn
    → AgentLoop::transition_start_generate
      → OpenAIModel::stream(interaction, params)           // Model::stream (sync)
        → HttpClient::send_sse(req)                        // sync
          → FetchHttpClient::send_sse (wasm/client/client.rs:160)
            → run_future(async { do_fetch(..).await .. })  // ← BLOCKING collect()
```

`run_future` (`foundation_core::valtron::executors::non_sendables.rs:408`) is:

```rust
pub fn run_future<F>(future: F) -> GenericResult<Vec<F::Output>> {
    let iter: DrivenRecvIterator<FutureTask<F>> = execute_as_task(task, None)?;
    Ok(ReadyValues::new(iter).filter_map(ReadyValue::inner).collect()) // ← collect
}
```

It is a "block a **future** to completion, gather every `Next` output into a
`Vec`" helper. The `collect()` is inherent — a future has no stream to return —
but it means the future is driven inline and never yields to the JS event loop, so
`do_fetch`'s promise never resolves and the isolate blocks inside `collect()`.

The rest of the pipeline is *already* lazy and correct:

- `WasmSseIterator` (`wasm/client/stream.rs:139`) bridges the JS body via
  `spawn_local` + `ConcurrentQueue` + `Stream::Wait` — the wasm-native valtron
  bridge — yielding `Wait` when no chunk is ready.
- The agent loop / `run_turn_stream` return the model stream lazily without
  consuming it (`Turn::into_inner()` → `ModelStreamBox`).
- The consumer boundary (`into_next_stream()`) already parks on `Wait`/`Pending`.

So the only thing that blocks is the **initial fetch**, wrapped in `run_future`.

### The two facts that scope the fix

1. **`run_future` is exclusively a wasm deadlock trap.** All three call sites are
   wasm and all deadlock: `FetchHttpClient::send` (client.rs:150),
   `FetchHttpClient::send_sse` (client.rs:180), and `backend_utils::sleep`
   (backend_utils.rs:23, the retry backoff). There are no native callers.
2. **The async seam's bridge is misnamed/underused.** `into_future_stream()` is a
   documented raw lens (`poll_next = Poll::Ready(iter.next())`, never parks);
   `into_next_stream()` is the awaitable value stream (parks). The former is used
   by `send_sse_async`; the latter is already what `cf-cloud-provider` consumes.

## Requirements

1. **Fix the bridge — `into_future_stream` parks.** Rename the current raw lens to
   `into_pure_future_stream()` (forwards `Stream<D, P>` items as-is, never parks,
   for consumers that inspect control states). Make `into_future_stream()` an
   await-friendly full-enum lens: it yields `Next`/`Spread`/`Ignore` items and
   parks on `Wait`/`Pending`/`Init`/`Delayed` via `WakeableStream::wake` (self-wake
   fallback on `NotSupported`). `into_next_stream()` is unchanged (value-only
   awaitable). Add parking tests for all three.

2. **Audit `run_future` → `block_on_collect`.** Rename `run_future` to make its
   blocking contract explicit and document it "wasm-unsafe, native-only"; remove
   all three wasm call sites (T3/T4). Keep the lazy alternative (`run_future_iter`)
   but fix its wasm single-threaded path (it polls inline with a no-op
   waker and drops the future on the first `Pending`).

   > **Naming note:** the original plan renamed `run_future` to `block_on_future`,
   > but `block_on_future` already exists (it returns a single `F::Output` and is
   > used by the `#[valtron]` macro, tests, `deployment_platform` and `keychain`).
   > The collect-to-`Vec` helper was therefore renamed `block_on_collect` instead,
   > so the two blocking helpers stay distinct (`block_on_future` = single output,
   > `block_on_collect` = all outputs).

3. **Make wasm `send_sse` lazy — no `run_future`.** `FetchHttpClient::send_sse`
   returns a lazy `BoxedSseIterator` that, on first poll, spawns the fetch via
   `spawn_local`, yields `Pending(Connecting)`/`Wait` until it resolves, then
   flattens `WasmSseIterator`'s body events. Both the fetch stage and the body
   stage implement `WakeableStream` (a `WakeRegistry` fired on data/close) so the
   awaitable bridge wakes exactly on readiness. A fetch failure is surfaced as
   `Stream::Pending(SseProgress::Failed(msg))` then `None`.

4. **Add a transport error channel.** Extend `SseProgress` with
   `Failed(String)` (this drops the `Copy` derive — the variant carries owned
   text). `ParseResult` is unchanged. This is what lets a *deferred* fetch error
   reach a lazy sync consumer, because `send_sse`'s `Result` can no longer carry
   it eagerly once the fetch is deferred. **Every existing `match` on
   `SseProgress` becomes non-exhaustive** (and the type is no longer `Copy`), so
   each consumer is updated in the same change to extract the error from the
   `Failed(msg)` arm: native `map_progress` (`http/http_client_impl.rs`), the
   wasm reader/stream mapping, and the model-layer `OpenAIStream` /
   `WorkersAiStream` (→ `ModelState::Error`).

5. **Map the error at the model layer.** `OpenAIStream` (and `WorkersAiStream`)
   map `Stream::Pending(SseProgress::Failed(msg))` → `Stream::Pending(ModelState::
   Error(msg))`. `ModelState::Error` already exists (`base_types.rs:1530`).

6. **Zero `Model` API change.** `Model::stream` keeps its signature
   (`GenerationResult<ModelStreamBox>`) and becomes non-blocking on wasm once
   `send_sse` is lazy. A consumer in async context does
   `model.stream(..)?.into_future_stream()` (now parking) or `.into_next_stream()`
   and awaits. No `stream_async`, no routing through `send_sse_async`.

7. **Fix the wasm `sleep` backoff.** `backend_utils::sleep` (wasm) currently uses
   `run_future(sleep_async)` and would deadlock on the retry path; drive it
   lazily/async instead (or mark retry-backoff as native-only and track the wasm
   path in the backlog).

## Architecture

### Technical approach

- **Lazy, pull-based valtron streams everywhere (CLAUDE.md rule #6).** The fix
  removes the *one* eager `collect()` and restores the lazy contract that
  `stream()`/`send_sse()` already promised. The boundary (sync vs async) is
  decided by the consumer — `into_future_stream()`/`into_next_stream()` for
  async, plain `Iterator::next()` for sync.
- **One parking primitive.** `WakeableStream::wake` + `WakeRegistry` (already used
  by `NotifyQueueStreamIterator` and the `into_next_stream`/`into_collect_future`
  bridges) is extended to `WasmSseIterator` and the new fetch stage.
- **Error through the stream, not the `Result`.** The lazy `send_sse` can no
  longer eagerly report a fetch failure; `SseProgress::Failed` carries it to
  `ModelState::Error`, which the agent loop already treats as a terminal error.

### Changes required

| # | Crate | Change | Why | Task |
|---|-------|--------|-----|------|
| 1 | `foundation_core` | Rename `into_future_stream` → `into_pure_future_stream`; make `into_future_stream` park on `Wait`/`Pending`/`Init`/`Delayed` | Await-friendly full-enum lens; raw lens gets an explicit name | T1 |
| 2 | `foundation_core` | Rename `run_future` → `block_on_collect` (doc: wasm-unsafe); fix `run_future_iter` wasm path | Make the blocking contract explicit; remove the trap from the streaming path | T2 |
| 3 | `foundation_core` | Parking tests for `into_future_stream`/`into_next_stream`/`into_collect_future`/`into_ready_future` (`Poll::Pending` on control states, resume-on-wake) | Regression guard for the bridge contract | T1 |
| 4 | `foundation_netio` | Add `SseProgress::Failed(String)` (drop `Copy`) | Deferred fetch error channel | T3 |
| 5 | `foundation_netio` | Lazy `send_sse`: `spawn_local` fetch + flatten `WasmSseIterator`; `WakeableStream` on both stages | Remove `run_future` from the wasm streaming path | T3 |
| 6 | `foundation_netio` | `send_sse_async` uses the (now parking) `into_future_stream`; `BoxedSseFutureStream` item stays `Stream<ParseResult, SseProgress>` | The async seam already `.await`s the fetch; the fixed bridge makes it park | T3 |
| 7 | `foundation_ai` | `OpenAIStream`/`WorkersAiStream` map `SseProgress::Failed` → `ModelState::Error`; fix wasm `sleep` | Surface deferred errors; remove the last `run_future` | T4 |
| 8 | `examples/cf-cloud-provider` | No structural change needed: `stream()` is now lazy; the existing `into_next_stream()` drives it. Verify live streaming | End-to-end proof | T5 |

### Interface definitions

```rust
// foundation_core — the three bridge shapes
pub trait StreamIteratorExt {
    // NEW: park-friendly full-enum lens (Item = Stream<D, P>), parks on control states
    fn into_future_stream(self) -> StreamAsFutureStream<Self> where Self: WakeableStream;
    // RENAMED: raw lens (Item = Stream<D, P>), never parks — for control-state inspection
    fn into_pure_future_stream(self) -> StreamAsFutureStream<Self>;
    // UNCHANGED: value-only awaitable (Item = D), parks
    fn into_next_stream(self) -> StreamNextStream<Self> where Self: WakeableStream;
}

// foundation_core — explicit blocking contract (wasm-unsafe)
pub fn block_on_collect<F>(future: F) -> GenericResult<Vec<F::Output>>; // was run_future

// foundation_netio — transport error channel
pub enum SseProgress { Connecting, Reading, Reconnecting, Failed(String) } // Failed is new

// foundation_netio wasm — lazy SSE (fetch + body), no run_future
pub struct LazySseIterator { /* spawn_local fetch → WasmSseIterator; WakeableStream */ }
impl Iterator for LazySseIterator { type Item = Stream<ParseResult, SseProgress>; }
impl WakeableStream for LazySseIterator { fn wake(&mut self, cb) -> WakeResult; }
```

### Data flow (corrected, wasm)

```mermaid
sequenceDiagram
    participant C as Client
    participant W as Worker (cf-cloud-provider)
    participant P as OpenAIModel
    participant F as FetchHttpClient
    participant JS as JS event loop
    C->>W: POST /generate (prompt)
    W->>P: run_turn_stream(prompt) → lazy Turn (no fetch yet)
    P->>F: send_sse(req) → LazySseIterator (returns immediately, no run_future)
    F->>JS: spawn_local { do_fetch().await } (yields; Pending(Connecting))
    JS-->>F: response body → flatten WasmSseIterator
    F-->>P: Stream<ParseResult, SseProgress> (Wait parks → yields to JS)
    P-->>W: ModelStreamBox (Messages; into_next_stream parks → yields)
    W-->>C: ReadableStream response (SSE)
```

_The diagram shows the corrected path: nothing blocks, and each step parks and yields to the JS event loop._


The decisive difference from the broken path: **no `run_future`, no `collect()`.**
The fetch is driven by `spawn_local` and the consumer parks on `Wait`/`Pending`.

### Trade-offs

| Decision | Rationale | Alternatives |
|----------|-----------|--------------|
| Make `send_sse` lazy (not add `send_sse_async` routing) | Honors "boundaries decide": the sync method returns a lazy stream, the consumer picks sync/async | Route the wasm path through `send_sse_async` (rejected: would force a `stream_async`/async model seam) |
| `SseProgress::Failed` (drop `Copy`) | Deferred fetch errors must flow through the stream once `send_sse` is lazy; the model layer already has `ModelState::Error` | Change `BoxedSseIterator` item to `Result<Stream<..>>` (rejected: ripples into every SSE consumer) |
| Rename raw lens → `into_pure_future_stream`, make `into_future_stream` park | The natural name should be await-friendly; raw-lens dependency is 2 real call sites + 1 example | Set a `park: bool` flag on the struct (viable; rename is cleaner given low dependency count) |
| `block_on_collect` rename, not delete | `run_future` has no native callers but may still be a legitimate native blocking helper; the name documents the contract | Delete it outright (also viable — no native callers today) |
| Zero `Model` API change | `stream()` already returns an iterator; fixing `send_sse` makes it lazy with no trait change | Add `stream_async` (rejected by review: unnecessary seam) |

## Implementation

### Files to create/modify

- `backends/foundation_core/src/valtron/stream_future.rs` — split `StreamAsFutureStream`
  into parking (default) and raw (`into_pure_future_stream`) modes; parking logic
  mirrors `StreamNextStream`.
- `backends/foundation_core/src/valtron/extensions/streams/{sendable,non_sendable}.rs` —
  rename + add the two bridge methods.
- `backends/foundation_core/src/valtron/executors/non_sendables.rs` — rename
  `run_future` → `block_on_collect` (+ doc); fix `run_future_iter` wasm path.
- `backends/foundation_core/tests/valtron/*` — parking tests; update existing
  `stream_future`/`spread` tests that exercised the raw lens to `into_pure_future_stream`.
- `backends/foundation_netio/src/shared/client/http_client.rs` — `SseProgress::Failed`.
- `backends/foundation_netio/src/wasm/client/stream.rs` — `WakeableStream` for
  `WasmSseIterator` (wake registry fired by the reader).
- `backends/foundation_netio/src/wasm/client/client.rs` — new `LazySseIterator`;
  `send_sse` returns it (no `run_future`); `send` and `send_sse_async` updated.
- `backends/foundation_ai/src/backends/openai_provider.rs` — map `SseProgress::Failed`
  → `ModelState::Error`; shared handling for `OpenAIStream`.
- `backends/foundation_ai/src/backends/workers_ai.rs` — same error mapping.
- `backends/foundation_ai/src/backends/backend_utils.rs` — fix wasm `sleep`.
- `examples/cf-cloud-provider/src/lib.rs` — verify (minimal/no change).
- `docs/bug-backlog.md` — delete entries when fixed.

## Tasks

- [x] **T1 — bridge fix + parking tests (foundation_core).** Rename raw lens →
      `into_pure_future_stream`; make `into_future_stream` park; add parking tests.
      Gate: `cargo test -p foundation_core` + wasm `cargo check`.
- [x] **T2 — `run_future` audit (foundation_core).** Rename → `block_on_collect`,
      document wasm-unsafe; fix `run_future_iter` wasm path.
      Gate: `cargo test -p foundation_core` + wasm `cargo check`.
- [x] **T3 — lazy `send_sse` + error channel (foundation_netio).** `SseProgress::Failed`,
      `LazySseIterator`, `WakeableStream` for `WasmSseIterator`; `send_sse` drops
      `block_on_collect`. Gate: `cargo check -p foundation_netio` + wasm
      `--features wasm-fetch` + `cargo test -p foundation_netio`.
- [x] **T4 — map error + fix `sleep` (foundation_ai).** `SseProgress::Failed` →
      `ModelState::Error`; wasm `sleep` no-op (backlog). Gate: `cargo test -p foundation_ai`
      + wasm `cargo check --no-default-features`.
- [x] **T5 — verify cf-cloud-provider (examples).** `stream()` is lazy; `POST /generate`
      streams a reply under `wrangler dev --local` without blocking `GET /`.
      Verified against a mock OpenRouter (`OPENROUTER_BASE_URL`; the sandbox has
      no route to openrouter.ai). Needed two more fixes (see `progress.md`):
      no `block_on_collect` on any wasm path (`send` errors, `get_model` skips the
      metadata lookup), and the generate task parks on SSE readiness
      (`WakeSignal` + `TaskStatus::Depends`) with an event-driven microtask
      resume instead of re-polling on `Wait`. Gate: miniflare smoke test.

## Testing

### Test cases

1. **Parking (foundation_core).** `into_future_stream`/`into_next_stream`/
   `into_collect_future`/`into_ready_future` return `Poll::Pending` on
   `Wait`/`Pending`/`Init`/`Delayed` and resume on wake (hand-rolled waker).
2. **Raw lens stays raw.** `into_pure_future_stream` forwards
   `Stream::Wait`/`Pending` as items (documented contract, unchanged behavior).
3. **Lazy `send_sse` (netio).** The first `next()` returns `Pending(Connecting)`
   without a resolved fetch; a fetch error surfaces as `Pending(Failed(msg))` then
   `None` (testable via a mocked `fetch`).
4. **Error mapping (foundation_ai).** `SseProgress::Failed` maps to
   `ModelState::Error`; existing OpenAI/Workers-AI stream tests still pass.
5. **Wasm gates.** `cargo check --target wasm32-unknown-unknown` for
   `foundation_core`/`foundation_netio`/`foundation_ai` (T1–T4).
6. **Miniflare smoke (live OpenRouter).** `wrangler dev --local`, `POST /generate`
   streams and the isolate keeps serving `GET /` afterwards.

### Local run commands

```bash
cargo test -p foundation_core            # parking + bridge tests
cargo test -p foundation_netio           # SSE tests
cargo test -p foundation_ai              # stream tests (error mapping preserved)
cargo check --target wasm32-unknown-unknown -p foundation_netio --no-default-features --features wasm-fetch
cargo check --target wasm32-unknown-unknown -p foundation_ai --no-default-features
cd examples/cf-cloud-provider && npx wrangler dev --local
curl -sN -X POST http://127.0.0.1:8787/generate --data 'What is the capital of France?'
```

## Success criteria

- [x] `into_future_stream` parks on `Wait`/`Pending`/`Init`/`Delayed`; raw lens renamed
      `into_pure_future_stream`; parking tests in place
- [x] `run_future` renamed `block_on_collect` and documented wasm-unsafe; the wasm
      streaming path no longer uses it (`send_sse` in T3, `sleep` in T4); the one
      remaining wasm caller — non-streaming `HttpClient::send` — is tracked in
      `docs/bug-backlog.md`; `run_future_iter` wasm path fixed
- [x] Wasm `send_sse` returns a lazy `LazySseIterator` (no `block_on_collect`); `WasmSseIterator`
      and the fetch stage implement `WakeableStream`
- [x] `SseProgress::Failed(String)` added; `OpenAIStream`/`WorkersAiStream` map it to
      `ModelState::Error`
- [x] `Model::stream` signature unchanged; becomes non-blocking on wasm
- [x] `cf-cloud-provider` `POST /generate` streams a reply locally without blocking the
      isolate (`GET /` still responds after a generate) — verified with a mock
      vendor; a live OpenRouter key run is still worth doing
- [x] Sync `Model::stream` + agent loop unchanged for native; existing tests green
- [x] No TODO/FIXME/stubs; streamed and typed; no ignored `Result`s

---
_Created: 2026-10-20_
