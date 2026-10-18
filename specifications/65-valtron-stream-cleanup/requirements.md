---
description: "Remove the dead ConcurrentQueue<Stream<D,P>> iterators — StreamRecvIterator and ConcurrentQueueStreamIterator — that were superseded by Pipe (cross-crate streaming) and NotifyQueue (core execution path). Includes a full cross-crate audit recorded as the cleanup checklist."
status: "in-progress"
priority: "high"
created: 2026-10-18
author: "Main Agent"
metadata:
  version: "1.0"
  last_updated: 2026-10-18
  estimated_effort: "small"
  tags:
    - valtron
    - stream-iterator
    - dead-code
    - cleanup
    - concurrent-queue
  stack_files: [rust.md]
  skills:
    - rust-valtron-usage
    - rust-valtron-iterator
  tools: []
has_features: true
has_fundamentals: false
builds_on: ""
related_specs:
  - "specifications/64-valtron-state-task"
features:
  completed: 0
  uncompleted: 1
  total: 1
  completion_percentage: 0
---

# Overview

Valtron has three generations of stream mechanics. `Pipe` (`PipeSender`/
`PipeReceiver`) is now the cross-crate streaming primitive, and the `NotifyQueue`
family (`NotifyQueueStreamIterator`/`NotifyRecvIterator`) is the core-internal
`execute()` path. Two older `ConcurrentQueue<Stream<D, P>>` iterators are dead:

- `StreamRecvIterator` — deprecated, **zero** references outside its own file.
- `ConcurrentQueueStreamIterator` — referenced only by its own doc comments and
  its tests; the executors actually use `NotifyQueueStreamIterator`.

This specification removes both, retires their tests, and fixes the stale
"default iterator" doc claims. A full cross-crate audit is recorded as the
checklist in feature 00.

This is intended to be worked on **before** `64-valtron-state-task`, as a
house-cleaning pass that shrinks the public stream surface.

# Goals

- Delete the two dead `ConcurrentQueue<Stream<D, P>>` iterators and their
  now-unused imports.
- Retire their tests; keep the combinator-layer tests that share the file.
- Correct every stale doc reference that still names the removed types.
- Confirm — with the audit — that nothing outside `foundation_core` depends on
  the removed types.

# Implementation Location

- `backends/foundation_core/src/valtron/streams.rs` — remove `StreamRecvIterator`
  (`:225`) and `ConcurrentQueueStreamIterator` (`:315`) + imports + module docs.
- `backends/foundation_core/tests/valtron/stream_iterators.rs` — retire the
  `ConcurrentQueueStreamIterator` test section (`:755–893`).
- Stale doc comments: `sendables.rs:352,395`, `non_sendables.rs:469`,
  `config.rs`, `constants.rs`, `multi/mod.rs:1023,1042,1053,1058`,
  `foundation_netio/src/shared/http/pushable_body.rs:14`.

# Known Issues

- `streams.rs` is re-exported via `pub use streams::*` (`valtron/mod.rs:37`), so
  removing the two `pub struct`s is a public-API deletion. The audit shows zero
  external callers, so it is safe.
- The `concurrent_queue` crate dependency must remain (still the primitive under
  `NotifyQueue`, `worker.rs`, `buffer_pool.rs`, `synca/mpp.rs`).

# Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | dead-code removal + doc fixes | `.agents/skills/rust-valtron-usage/skill.md`, `.agents/skills/rust-valtron-iterator/skill.md` |

# Feature Index

| # | Feature | Description | Dependencies | Status |
|---|---------|-------------|--------------|--------|
| 0 | [Cleanup](./features/00-cleanup/feature.md) | Remove dead stream iterators; full cross-crate audit as checklist | None | ⬜ Pending |

Status Key: ⬜ Pending | 🔄 In Progress | ✅ Complete

# Requirements Conversation Summary

- **Scope came from a cross-crate audit.** The user asked to find every
  `ConcurrentQueue<Stream<D, P>>` communicator with zero callers (or whose users
  also have zero callers), after `Pipe`/`NotifyQueue` superseded the old mechanics.
- **Audit result:** exactly two types are dead — `StreamRecvIterator` and
  `ConcurrentQueueStreamIterator`. The combinator layer (`extensions/{streams,tasks}/*`)
  also carries `Arc<ConcurrentQueue<Stream<D, P>>>` but is alive and cross-crate
  (netio, connectrpc), so it is explicitly **out of scope**.
- **Record the audit as todos.** The feature must map the full cross-crate audit
  up front and wrap each finding as a concrete task.

# High-Level Architecture

```mermaid
graph TD
    subgraph ALIVE["Alive (do not touch)"]
        PIPE["Pipe / PipeSender / PipeReceiver<br/>(cross-crate: connectrpc, netio, http, nativeapis, proxy, docker)"]
        NQ["NotifyQueue family<br/>(core-internal execute path)"]
        COMB["Combinator layer: extensions/streams + extensions/tasks<br/>Arc&lt;ConcurrentQueue&lt;Stream&lt;D,P&gt;&gt;&gt; (cross-crate: netio, connectrpc)"]
        RCV["RecvIterator (synca/mpp)<br/>(future_task.rs, channel API)"]
    end
    subgraph DEAD["Dead (remove)"]
        SRI["StreamRecvIterator<br/>streams.rs:225 — zero callers, deprecated"]
        CQI["ConcurrentQueueStreamIterator<br/>streams.rs:315 — test-only callers"]
    end
```

- The two dead iterators sit in `streams.rs`; neither is used by the live
  combinator layer, so removal is self-contained.
- The combinator layer's `ConcurrentQueue<Stream<D, P>>` usage is a separate,
  still-live mechanism (fan-out for `split`/`broadcast`/`collect`) and is not in scope.

# Success Criteria (Spec-Wide)

1. `StreamRecvIterator` and `ConcurrentQueueStreamIterator` are gone from
   `streams.rs` and from the public `valtron` namespace.
2. `rg "ConcurrentQueueStreamIterator|StreamRecvIterator"` finds no remaining
   reference outside git history (doc comments included).
3. The `concurrent_queue` dependency and the combinator layer are untouched and
   still compile; the full `foundation_core` test suite passes.
4. `tests/valtron/stream_iterators.rs` retains its combinator tests and drops only
   the `ConcurrentQueueStreamIterator` section.

# Out of Scope

- Migrating the combinator layer (`extensions/{streams,tasks}/*`) from
  `ConcurrentQueue` to `NotifyQueue`/`Pipe`. It is alive and cross-crate; a
  migration is a separate spec if ever wanted.
- Removing `RecvIterator` (`synca/mpp.rs`) — still used by `future_task.rs`.
- Any change to `Pipe` or the `NotifyQueue` family.

# Module References

- `backends/foundation_core/src/valtron/streams.rs` — the two dead types
- `backends/foundation_core/src/valtron/extensions/{streams,tasks}/{sendable,non_sendable}.rs` — live combinator layer (out of scope)
- `backends/foundation_core/src/valtron/executors/local.rs` — `NotifyQueue*` (live replacement)
- `backends/foundation_core/src/valtron/pipe.rs` — `Pipe` (cross-crate replacement)
- `backends/foundation_netio/src/shared/http/pushable_body.rs` — stale doc reference
- `backends/foundation_core/src/valtron/mod.rs:37` — `pub use streams::*`

---

_Created: 2026-10-18_
_Last Updated: 2026-10-18_
_Structure: Feature-based (has_features: true)_
