---
feature: "Pushable client request body over Pipe<Bytes> (D12 §7)"
description: "Client streaming request bodies via chunked TE with waker-hooked pipe backing (B6)"
status: "complete"
priority: "high"
phase: 1
depends_on: ["02-pipe-primitive"]
estimated_effort: "small"
created: 2026-07-03
---
# Feature 07-pushable-request-body: Pushable client request body over Pipe<Bytes> (D12 §7)

## Description

Back SendSafeBody::Stream with the Pipe<Bytes> primitive so the client can push request bytes after the request has started sending (io.Pipe parity, no whole-request buffering).

## Normative sources (single source of truth — read before writing code)

- decisions/12-foundation-enablement.md — §7 (normative; Pipe<Bytes>, NOT ConcurrentQueueStreamIterator)

## Scope

- Pushable backing for the existing SendSafeBody::Stream variant; no new body type
- Producer half held by the client-stream/bidi task; consumer drained by the request renderer

## Acceptance criteria

- Request bytes pushed after send-start reach the wire without full-request buffering; no polling handoff

## Verification (complete)

`backends/foundation_netio/src/simple_http/shared/pushable_body.rs`:
- `pushable_request_body()` / `pushable_request_body_with_depth(n)` pair a
  `PushableRequestBody` producer with a `SendSafeBody::Stream` (no new body type — §7). The
  producer holds a `PipeSender<Bytes>` (00-F4 `Pipe<Bytes>`), the returned `SendSafeBody::Stream`
  wraps a `PushableBodyReader` draining the `PipeReceiver<Bytes>`.
- `PushableRequestBody`: `try_push` (non-blocking), async `push` (parks on a full pipe via the
  vacancy waker), `close`/`is_closed`. The consumer maps `try_recv` → `Data::Bytes` (chunk),
  `Empty`→`Data::Retry` (transient, the driving task parks on the pipe readiness), `Closed`→end.
  Uses the waker-hooked pipe, **not** `ConcurrentQueueStreamIterator` (whose polling handoff
  00-F4 removes).

Tests (`tests/simple_http/pushable_body_tests.rs`): pushed chunks drain FIFO as `Data::Bytes`
(empty→`Retry`, close→`None`); **push-after-send-start** reaches the renderer; depth-bounded
backpressure (`try_push` → `Full`, drain frees a slot); async push **parks on a full pipe and
wakes on drain** (observed via `queue_waker`); dropping the producer ends the body. `5 passed`;
netio lib clean.
