# Spec 71 handoff

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations.** Hypertext Transfer Protocol (HTTP); Server-Sent Events (SSE); JavaScript Object
Notation (JSON); time to live (TTL); Continuous Integration (CI); virtual file system (VFS);
Representational State Transfer (REST); Amazon Simple Storage Service (S3).

This file states the exact state of the work and the next steps. Read it before you change code.

## 1. Verified gates

| Gate | Result | Command |
|---|---|---|
| Server conformance | 332 pass, 6 skip, 0 fail | `ci/durable_conformance.sh` |
| Client conformance | 208 pass, 0 fail, 61 skip | `ci/durable_conformance.sh` |
| Core durable tests | 81 pass | `cargo test -p foundation_core --features multi,test-clock --test mod durable` |
| HTTP adapter tests | 30 pass | `cargo test -p foundation_http --test durable_adapter` |
| Client core tests | 24 pass | `cargo test -p foundation_netio --features multi --test foundation_netio_tests durable` |

The 61 client skips are SSE only. The adapter announces `sse: false`.

## 2. What works

- The transport-neutral core in `foundation_core::durable::shared`: create, append, close, read,
  head, delete, fork, `Stream-Seq`, producer fencing and dedup, TTL, JSON mode, soft-delete and
  cascade, bounded pages, and the notifier. The architecture test proves it names no transport or
  valtron code path.
- The HTTP surface: request adapter, catch-up, long-poll, SSE, ETag and `304`, cursor, cross-origin
  headers, forks, `Stream-Expires-At`, producer and closure error headers, a bounded body guard.
- The client core with an injected transport, a native transport, and a JSON-line conformance
  adapter. Batching, auto mode, dynamic headers, conditional reads and `Retry-After` work.

## 3. Blockers

### 3.1 Issue #963: the native SSE stream blocks in `poll_next`

- Location: `foundation_netio/src/event_source/native/task.rs`, state
  `EventSourceState::ReadingStream`.
- Cause: the state calls `iterator.next()` on a blocking iterator. The future stream calls the
  task from `poll_next`, so the poll parks in the socket read.
- Measured: a probe printed a `data` and a `control` event, then the next poll never returned.
  A self-waking deadline, a socket read timeout and a readiness check before the read all failed.
- Fix: make the SSE body iterator incremental. Read the buffered bytes, return no event until the
  buffer holds a complete event, and return `TaskStatus::Wait` with a wake from the connection.
- Then set `sse: true` in the adapter and run `ci/durable_conformance.sh`.

### 3.2 Issue #947: the SSE open hides the status and headers

- Location: `foundation_netio/src/shared/client/http_client.rs`, `send_sse_async`.
- Cause: the method returns only the event stream. The client cannot answer `404` or `410` and
  cannot read `stream-sse-data-encoding` from the head.
- Partial fix: the client sends one `HEAD` request first and uses its status and encoding header.
- Full fix: return a head with the stream.

## 3.3 Owner corrections (2026-11-27)

- The client core and the `ClientTransport` trait live in `foundation_core::durable::client`.
  `foundation_netio` holds only the native transport. Done.
- The reserved subscription APIs are in scope. Feature 10 and issue #1000 record the work.
  The six server skips are a gap, not an approved exclusion.
- Use `foundation_core::valtron::block_on_future_scoped` in tests. It drives a borrowed,
  non-`Send` future on the caller's thread. Do not use `smol` in the durable tests.
- Where one future yields many values, return a `futures_core::Stream` (the client SSE read).

## 4. Next features

1. Feature 05: the file VFS in `foundation_nativeapis`. The first implementation is in
   `foundation_nativeapis/src/native/vfs/durable_file.rs` and compiles under `vfs-native`. It has
   no tests yet. Add restart, fork, soft-delete and conformance tests, then run the crate tests.
2. Feature 09: the S3/R2 VFS. The design is in the feature file.
3. Feature 06: the ConnectRPC surface.
4. Feature 07: the valtron client wrapper. Resolved: the client core is in
   `foundation_core::durable::client`, and both valtron wrappers are in
   `foundation_core::durable::valtron`. `foundation_netio` implements the transport trait.
5. Feature 10: subscriptions. A new `foundation_event_stream` crate owns the work. It re-exports
   `foundation_core::durable` as `foundation_event_stream::durable`, and reuses
   `foundation_auth::shared::jwks` for signing keys and the JWKS endpoint. The six skipped server
   tests must pass, and the gate must set `subscriptions: true`.
5a. Issue #1019, after feature 10: the wasm bridge builds a parsed fetch request with a `Stream`
   body without `with_preserved_headers`, so it drops `Content-Length`. The native parser already
   sets the flag. Fix `foundation_netio/src/wasm/bridge/{cf,web}.rs`, add a wasm test, close the
   issue in the same change.
6. Feature 08: the CI wiring. `ci/durable_conformance.sh` is the command to call.
6. The wider oracle matrix: differential, crash and recovery, deterministic simulation testing
   (DST) across VFS implementations.

## 4.1 Working tree

- Uncommitted: `foundation_nativeapis/src/native/vfs/durable_file.rs` and the `vfs/mod.rs` line.
  The first file VFS implementation compiles under `vfs-native`. It needs tests before a commit.
- Uncommitted: `Cargo.lock`, from the removed test dependency. Check it before the next commit.

## 5. Rules that matter here

- Run `ci/durable_conformance.sh` before you call a protocol change done.
- Every skip needs a reason and an owner. Today the only reason is #963.
- A server or client workaround for #963 is measured and rejected. Change the netio event task.

_Created: 2026-11-27._
