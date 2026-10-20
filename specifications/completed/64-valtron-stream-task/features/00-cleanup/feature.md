---
feature: "Cleanup dead stream iterators"
description: "Remove StreamRecvIterator and ConcurrentQueueStreamIterator (dead ConcurrentQueue<Stream<D,P>> iterators) and fix stale docs, driven by a full cross-crate audit recorded as the task list."
status: "complete"
priority: "high"
depends_on: []
estimated_effort: "small"
created: 2026-10-18
last_updated: 2026-10-19
author: "Main Agent"
tasks:
  completed: 9
  uncompleted: 0
  total: 9
  completion_percentage: 100
---

# Cleanup dead stream iterators

## Overview

Valtron's `Stream<D, P>` channels now flow through `Pipe` (cross-crate) and
`NotifyQueue` (core-internal). Two legacy `ConcurrentQueue<Stream<D, P>>`
iterators in `streams.rs` are dead and safe to delete. This feature removes them
and records the cross-crate audit that proves safety.

## Cross-crate audit (findings)

| Element | Location | Callers | Cross-crate | Verdict |
|---------|----------|---------|-------------|---------|
| `StreamRecvIterator<D,P>` | `streams.rs:225` | **zero** (deprecated 1.3.0) | none | ❌ remove |
| `ConcurrentQueueStreamIterator<D,P>` | `streams.rs:315` | 7 tests only (`stream_iterators.rs:763–893`) | none (doc comments only) | ❌ remove |
| Combinator layer (`extensions/streams/*`, `extensions/tasks/*`) | `SCollectorStreamIterator`, `SplitCollectorContinuation`, `BroadcastContinuation`, `SplitUntil*`, `SplitCollectorMap*`, `CollectorStreamIterator`, … | alive | **netio** (`http/api.rs`, `shared/client/body_reader.rs`), **connectrpc** (`transport/h1.rs`, `ws.rs`) | ✅ keep |
| `Pipe` / `PipeSender` / `PipeReceiver` | `pipe.rs`, `pipe_mapped.rs` | alive | connectrpc, http, netio, nativeapis, proxy, docker | ✅ keep |
| `NotifyQueue` family | `executors/local.rs` | alive (execute path) | none (internal) | ✅ keep |
| `RecvIterator` (`synca/mpp`) | `mpp.rs` | `future_task.rs:677` (ThreadedIterFuture) | none direct | 🔁 migrate (feature 01) |

Key finding: the combinator layer also carries `Arc<ConcurrentQueue<Stream<D, P>>>`
internally, but it is **not** the legacy iterator path — it is the live
`split`/`broadcast`/`collect` fan-out and is used cross-crate. It is out of scope.

## Removal scope

1. **`StreamRecvIterator<D, P>`** (`streams.rs:225`) — struct + impls:
   - `is_empty` / `len` / `is_closed` (`:231–244`)
   - `new` (`:249–255`)
   - `Iterator` (`:258–265`)
2. **`ConcurrentQueueStreamIterator<D, P>`** (`streams.rs:315`) — struct + impls:
   - `new` / `chan` / `max_turns` / `park_duration` / `is_closed` / `len` / `is_empty` (`:321–376`)
   - `Iterator::next` (`:388–418`)
3. **Now-unused imports in `streams.rs`** — `use crate::synca::mpp::RecvIterator;`
   (`:25`, only `StreamRecvIterator` uses it), plus `ConcurrentQueue`, `PopError`,
   `Duration`, and the `std::thread` cfg block if no longer referenced. Verify with
   `cargo check -p foundation_core`.

## Tasks

1. Remove `StreamRecvIterator` and its three impl blocks from `streams.rs`.
2. Remove `ConcurrentQueueStreamIterator` and its two impl blocks from `streams.rs`.
3. Remove now-unused imports from `streams.rs` (`RecvIterator`, `ConcurrentQueue`,
   `PopError`, `Duration`, `std::thread`), confirming against remaining use.
4. Update `streams.rs` module docs (`:5–12`) — drop the `StreamRecvIterator` bullet
   and the "default iterator used by valtron executors" claim; point to
   `NotifyQueueStreamIterator` as the execution-path iterator.
5. Retire the `ConcurrentQueueStreamIterator` test section
   (`tests/valtron/stream_iterators.rs:755–893`, including the `use` at `:758–759`).
   Keep the combinator tests above (`:46–691`).
6. Fix stale doc comments that name the removed types:
   - `sendables.rs:352,395`
   - `non_sendables.rs:469`
   - `config.rs:6–9,27,29`
   - `constants.rs:14,19,24`
   - `multi/mod.rs:1023,1042,1053,1058`
   - `foundation_netio/src/shared/http/pushable_body.rs:14`
7. Grep the whole workspace for `ConcurrentQueueStreamIterator|StreamRecvIterator`
   and confirm zero remaining references (doc comments included).
8. Confirm `concurrent_queue` remains a dependency (used by `NotifyQueue`,
   `worker.rs`, `buffer_pool.rs`, `synca/mpp.rs`) and that `RecvIterator` itself is
   untouched.
9. Build `foundation_core` (both `multi` and `!multi`) and run the valtron test
   suite (`stream_iterators`, `notification_based_waiting`, `channel_backpressure_tests`).

## Out of scope

- The combinator layer's `ConcurrentQueue<Stream<D, P>>` internals — alive and
  cross-crate; migrating it to `NotifyQueue`/`Pipe` is a separate effort.
- Migrating `RecvIterator`'s `ThreadedIterFuture` caller to `NotifyQueue` — that is
  feature 01. `mpp` itself (`Broadcaster`/`Receiver`/`recv_timeout`) and `Pipe`
  stay.

## Success criteria

- `rg "ConcurrentQueueStreamIterator|StreamRecvIterator"` returns nothing outside
  git history.
- `stream_iterators.rs` keeps its combinator tests and only loses the
  `ConcurrentQueueStreamIterator` section.
- `foundation_core` compiles and its valtron tests pass under `multi` and `!multi`.
- No cross-crate consumer breaks (audit confirms none depend on the removed types).
