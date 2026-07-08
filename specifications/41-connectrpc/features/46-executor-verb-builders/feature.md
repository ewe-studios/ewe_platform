---
feature: "Executor verb-builders — decompose the monolithic task builder into per-verb builders (sendable/non_sendable twins) and drop the Mapper machinery"
description: "Replace the single six-type-parameter ExecutionTaskIteratorBuilder (and multi's parallel ThreadPoolTaskBuilder) with a cfg-agnostic config stage plus one builder per ExecutionEngine verb (Schedule/Lift/Sequence/Broadcast), each defined once in sendable.rs (#![cfg(multi)]) and once in non_sendable.rs, presenting an identical spawn()/recv()/stream() surface with no per-function #[cfg]; and remove the Mapper/TaskStatusMapper type parameter entirely now that TaskIteratorExt combinators cover upstream transformation"
status: "pending"
priority: "high"
phase: 1
depends_on: ["00-valtron-async-readiness", "45-delivery-fanout-backpressure"]
estimated_effort: "large"
created: 2026-07-07
---
# Feature 46-executor-verb-builders: split the task builder by engine verb and delete the Mapper machinery

## Description

The valtron task-spawning surface is funnelled through **one** builder struct,
`ExecutionTaskIteratorBuilder<Done, Pending, Action, Mapper, Resolver, Task>`
(`executors/builders.rs`), plus a near-duplicate `ThreadPoolTaskBuilder<…>` in
`executors/multi/mod.rs`. A single struct carries **four unrelated dispatch
verbs** (`schedule`, `lift`, `sequenced`, `broadcast` — the four functions
`ExecutionEngine` exposes) × **three delivery forms** (spawn / recv-iterator /
stream-iterator), across **two build configurations** (`multi` on/off). Those
axes have *different* trait-bound requirements, and a single generic struct
cannot express them without festooning every method with `#[cfg]` — which is
exactly the state Feature 45 left the tree in (duplicate inherent definitions
under `multi`, missing `'static` bounds under single, and `broadcast()` vanishing
under `not(multi)`; see "Problem" below).

This feature does two coupled things:

1. **Decompose by verb.** Replace the mux-everything builder with a small
   cfg-agnostic **config stage** and one **verb builder per engine verb**
   (`ScheduleBuilder`, `LiftBuilder`, `SequenceBuilder`, `BroadcastBuilder`).
   Each verb builder is defined **twice** — once in `builders/sendable.rs`
   (`#![cfg(feature = "multi")]`, `Send` bounds) and once in
   `builders/non_sendable.rs` (`#![cfg(not(feature = "multi"))]`, no `Send`) —
   with an **identical public method surface** (`spawn()`, `recv()`, `stream()`,
   `stream_with_config()`). The `#![cfg]` lives at **module scope**, so there is
   **zero per-function `#[cfg]`**. This mirrors the tree's existing
   `executors/{sendables,non_sendables}.rs` and `extensions/tasks/{sendable,
   non_sendable}.rs` split.

2. **Delete the Mapper machinery.** Drop the `Mapper: TaskStatusMapper` type
   parameter, the `mappers: Vec<Mapper>` field, and `with_mappers()` from every
   builder and from `StreamConsumingIter` / `ConsumingIter` / `ReadyConsumingIter`
   (and `OnNext`). Post-task transformation is already expressible upstream via
   `TaskIteratorExt` (`map_ready` / `map_pending` / `filter_ready` /
   `split_collector` …) applied to the task **before** it enters the builder.
   Mappers are dead weight: the only `with_mappers` call sites are internal, the
   default is always `ZeroMapping` (identity), and no caller anywhere supplies a
   real mapper.

## Normative sources (read before writing code)

- `backends/foundation_core/src/valtron/task.rs` — `ExecutionEngine` trait
  (`sequenced` / `lift` / `schedule` / `broadcast` / `broadcast_or_lift` /
  `broadcast_or_sequence`, lines ~1103-1190); `BoxedExecutionIterator` vs
  `BoxedSendExecutionIterator` (lines ~941-945); `GlobalTask` (cfg alias, lines
  893/896); `EventReadinessPtr` (cfg alias — `Send + Sync` under `multi`, bare
  under `not(multi)`; the reason the delivery forms diverge by cfg);
  `TaskStatusMapper` + `FnMapper` / `FnOptionMapper` / `ZeroMapping` (lines
  ~1418-1520, the machinery being removed).
- `backends/foundation_core/src/valtron/executors/builders.rs` — the current
  `ExecutionTaskIteratorBuilder`, `spawn_builder`, `spawn_broadcaster`,
  `on_next` / `on_send_next` impls.
- `backends/foundation_core/src/valtron/executors/multi/mod.rs` — the parallel
  `ThreadPoolTaskBuilder` / `spawn2` (same six type params; same treatment).
- `backends/foundation_core/src/valtron/executors/task_iters.rs` —
  `*ConsumingIter` (the `mappers` field + application loop being removed).
- `backends/foundation_core/src/valtron/executors/{sendables,non_sendables}.rs`
  and `extensions/tasks/{sendable,non_sendable}.rs` — the blessed module-gated
  twin pattern to follow.

## Problem (with current-code evidence)

### 1. One struct, four verbs, diverging bounds — inexpressible without per-method `#[cfg]`

`ExecutionEngine::{schedule, lift, sequenced}` take a **non-`Send`**
`BoxedExecutionIterator`; `broadcast` takes `GlobalTask` (a
`BoxedSendExecutionIterator` under `multi`). The three *delivery forms* differ
again:

- **spawn form** (`schedule()`, `lift()`, `sequenced()`) wraps the task in
  `OnNext` / `DoNext` and needs no `Send` (single-threaded local dispatch).
- **delivery forms** (`*_iter`, `stream_*`) wrap the task in a `*ConsumingIter`
  that, since Feature 45 Part A, returns `State::Depends(QueueVacancyReadiness)`
  on a full delivery queue. Under `multi`, `EventReadinessPtr` is
  `Arc<dyn EventReadiness + Send + Sync>`, so `QueueVacancyReadiness<T>` must be
  `Send + Sync` ⇒ **`T: Send`** ⇒ the whole `*ConsumingIter: ExecutionIterator`
  impl (and every builder method that boxes it) needs `Done/Pending/Action:
  Send` **under `multi` only**.
- **broadcast form** always needs `Send` (`GlobalTask`), in both cfgs.

So the `Send` requirement is `(multi AND delivery) OR broadcast` — a per-method,
per-cfg predicate. On one struct that forces a method-level `#[cfg]` fan-out.
The current tree shows the failure mode directly: `cargo check -p foundation_core`
(single) fails with `broadcast` not found (moved into a `#[cfg(multi)]` block)
and `E0310` missing-`'static`; `--features multi` fails with **11 `E0592`
duplicate-definition** errors (`new`, `with_task`, `schedule`, … defined in both
an ungated and a `#[cfg(multi)]` impl) plus `E0034`/`E0277`. The mux design has
no clean expression.

### 2. The six-type-parameter struct is unergonomic and duplicated

`ExecutionTaskIteratorBuilder<Done, Pending, Action, Mapper, Resolver, Task>` has
six type parameters; `spawn_builder` / `spawn_broadcaster` each spell out a
`Box<dyn …Mapper>` / `Box<dyn …Resolver>` pair in their return type. The same
shape is re-implemented as `ThreadPoolTaskBuilder` in `multi/mod.rs`. Two of the
six parameters (`Mapper`, and the `Resolver`/`Mapper` pairing in every
`match (self.resolver, self.mappers)`) exist only to thread a transformation that
`TaskIteratorExt` already provides.

### 3. Mapper is dead weight

`TaskStatusMapper::map(Option<TaskStatus>) -> Option<TaskStatus>` is applied in a
loop inside each `*ConsumingIter` (`task_iters.rs:826`) and inside `OnNext`. But:
- No call site outside the builders supplies a mapper (`with_mappers` is only
  called internally); the default is `ZeroMapping` (identity).
- Every real transformation it could do — rewrite `Ready`, drop `Pending`,
  filter, short-circuit — is already offered by `TaskIteratorExt::{map_ready,
  map_pending, filter_ready, stream_collect, split_collector, …}`, applied to the
  `TaskIterator` **before** it reaches the builder, where it composes and is
  testable in isolation.
- `TaskStatusMapper` has **no references outside `foundation_core`**.

Keeping mappers costs a type parameter, a `Vec` allocation and per-item loop on
the hot delivery path, and a two-arm `match (resolver, mappers)` in every
builder method.

## Design

### Part A — Delete the Mapper machinery

- Remove the `Mapper` type parameter from `ExecutionTaskIteratorBuilder`,
  `ThreadPoolTaskBuilder`, and their `spawn_builder`/`spawn_broadcaster`/`spawn2`
  constructors. The builder keeps `Resolver` (the `on_next` ready-handler path is
  a genuinely distinct mechanism and is used).
- Remove `mappers: Option<Vec<Mapper>>`, `with_mappers()`, and every
  `match (self.resolver, self.mappers)` collapses to a single `self.resolver`
  check (`Some(resolver)` → `OnNext`; `None` → `DoNext`; the delivery forms build
  a `*ConsumingIter` directly).
- Remove the `mappers` field and the `for mapper in &mut self.mappers { … }`
  application loop from `StreamConsumingIter`, `ConsumingIter`,
  `ReadyConsumingIter`, and `OnNext`. `*ConsumingIter::new(task, iter_chan)` loses
  its `mappers` argument.
- Delete `TaskStatusMapper`, `IntoBoxed*TaskStatusMapper`, `FnMapper`,
  `FnOptionMapper`, `ZeroMapping`, and `BoxedTaskStatusMapper` /
  `BoxedSendTaskStatusMapper` — unless a residual internal user remains, in which
  case narrow it to that user and document why. (Audit: expected to be fully
  removable.)
- **Migration for anyone who wanted a mapper:** apply the equivalent
  `TaskIteratorExt` combinator to the task first, e.g.
  `spawn_builder(engine).with_task(task.map_ready(f)).as_scheduled().spawn()`.

### Part B — One builder per engine verb, split sendable/non_sendable

#### B1 — Config stage (cfg-agnostic)

A single `TaskSpawnConfig<Done, Pending, Action, Resolver, Task>` (rename of the
slimmed `ExecutionTaskIteratorBuilder`) holds `engine`, `task`, `parent`,
`resolver`, `panic_handler`. It carries **only** the setters and verb
transitions — no dispatch, no `Send`, one ungated `impl`:

- setters: `with_task`, `with_parent`, `maybe_parent`, `with_resolver`,
  `with_panic_handler`, `on_next`, `on_next_mut` (and `on_send_next*` stay under
  their existing `Send`-bounded impls).
- verb transitions (consume the config, hand off to a verb builder):
  - `as_scheduled(self) -> ScheduleBuilder<…>`
  - `as_lifted(self) -> LiftBuilder<…>`
  - `as_sequenced(self, parent: Entry) -> SequenceBuilder<…>`
  - `as_broadcast(self) -> BroadcastBuilder<…>`
  - `as_broadcast_or_lift(self) -> BroadcastBuilder<…>` /
    `as_broadcast_or_sequenced(self, parent) -> BroadcastBuilder<…>`
    (the fallback-carrying variants; see B3).

`spawn_builder(engine)` / `spawn_broadcaster(engine)` remain the entry points and
return a `TaskSpawnConfig` (the `spawn_broadcaster` return carries the `Send`
resolver box as today).

#### B2 — Verb builders with a uniform terminal surface

Each of `ScheduleBuilder`, `LiftBuilder`, `SequenceBuilder`, `BroadcastBuilder`
exposes the **same four terminals**:

```rust
fn spawn(self)                         -> AnyResult<SpawnInfo, ExecutorError>;
fn recv(self, wait: Duration)          -> AnyResult<NotifyRecvIterator<TaskStatus<Done, Pending, Action>>, ExecutorError>;
fn stream(self, wait: Duration)        -> AnyResult<NotifyQueueStreamIterator<Done, Pending>, ExecutorError>;
fn stream_with_config(self, wait: Duration, max_turns: usize)
                                       -> AnyResult<NotifyQueueStreamIterator<Done, Pending>, ExecutorError>;
```

#### B2a — `Send` is confined to the delivery terminals, NOT the spawn form

`ExecutionEngine::{schedule, lift, sequenced}` take a **non-`Send`**
`BoxedExecutionIterator` in *both* cfgs; only `broadcast` takes the `Send`
`GlobalTask`. And `Send` is *only ever* introduced by `State::Depends`: a delivery
iterator (`recv`/`stream`) parks on `QueueVacancyReadiness<T>`, which under `multi`
must coerce to the `Send + Sync` `EventReadinessPtr` ⇒ `T: Send`. The **spawn
form** (`OnNext`/`DoNext`) never builds a readiness and never touches a delivery
queue, so it needs **no `Send`, even under `multi`**. (`?Send` on the alias is not
an option — auto traits are not relaxable, and the `multi` alias must stay
`Send+Sync` for external crates that store readiness in `Send` iterators.)

Therefore the impls are placed by *what actually needs `Send`*, not by verb:

- **`mod.rs` (ungated, `'static` only):** `spawn()` for `ScheduleBuilder`,
  `LiftBuilder`, `SequenceBuilder`. Callers of `…as_scheduled().spawn()` pay **no
  `Send` tax** in either cfg — this is the common path and it stays free.
- **`sendable.rs` (`#![cfg(multi)]`, `Send`) / `non_sendable.rs`
  (`#![cfg(not multi)]`, no `Send`):** the delivery terminals `recv()` /
  `stream()` / `stream_with_config()` for all four verb builders, **plus**
  `BroadcastBuilder::spawn()` (multi = `engine.broadcast`; off = fallback per B3).

This is still zero *per-function* `#[cfg]`: the split is per-**impl-block**, and
those impl blocks live in the module-gated twin files. The verb-builder *structs*
themselves are declared once (ungated) in `mod.rs`.

- `spawn()` is the old `schedule()` / `lift()` / `sequenced()` / `broadcast()`.
- `recv()` is the old `schedule_iter` / `lift_iter` / `sequenced_iter` /
  `broadcast_iter`.
- `stream()` / `stream_with_config()` are the old `scheduled_stream_iter` /
  `stream_lift_iter` / `stream_sequenced_iter` / `stream_broadcast_iter`.

The verb-builder **structs** (`ScheduleBuilder`, `LiftBuilder`, `SequenceBuilder`,
`BroadcastBuilder`) are declared **once**, ungated, in `builders/mod.rs`, together
with the Send-free `spawn()` impls for the three local verbs (per B2a). Only the
`Send`-diverging impls are written twice:

- `builders/sendable.rs` — `#![cfg(feature = "multi")]`; the delivery-terminal
  impls (`recv`/`stream`/`stream_with_config`) for all four verb builders with
  `Done/Pending/Action: Send + 'static`, **plus** `BroadcastBuilder::spawn()`
  (real `engine.broadcast`).
- `builders/non_sendable.rs` — `#![cfg(not(feature = "multi"))]`; the same
  delivery-terminal impls with `…: 'static` (no `Send`), **plus**
  `BroadcastBuilder::spawn()` fallback (B3).

`builders/mod.rs` also holds `TaskSpawnConfig`, the constructors, the `on_next`
impls, and `pub use {sendable,non_sendable}::*;` under the matching `#[cfg]`
(exactly as `executors/mod.rs` re-exports `sendables`/`non_sendables`). Identical
method names in the two twins ⇒ portable call sites compile unchanged under both
cfgs.

#### B3 — Broadcast under `multi = off`

Per the resolved question, `BroadcastBuilder` exists in **both** twins with the
same surface. Its `non_sendable` twin dispatches via the engine's existing
`broadcast_or_lift` / `broadcast_or_sequence` fallback (multi=off lifts or
sequences to the local queue). Because a fallback needs a target, the config
captures the intended local fallback at the transition:
`as_broadcast_or_lift()` (optional parent) and `as_broadcast_or_sequenced(parent)`
produce a `BroadcastBuilder` whose `non_sendable::spawn()` calls
`engine.broadcast_or_lift(task, parent)` / `broadcast_or_sequence(task, parent)`,
while its `sendable::spawn()` calls `engine.broadcast(task)`. Plain
`as_broadcast()` under `not(multi)` defaults to the `broadcast_or_lift` (no
parent) behaviour, matching today's semantics.

#### B4 — Apply the same split to `multi/mod.rs`'s builder

`ThreadPoolTaskBuilder` / `spawn2` are the pool's own copy of the same shape.
Slim them the same way (drop `Mapper`) and route them through the shared
`TaskSpawnConfig` + verb builders rather than re-deriving the dispatch, so there
is one builder implementation, not two.

### Why this removes the cfg churn

The `Send` predicate `(multi AND delivery) OR broadcast` is no longer evaluated
per method on one struct. Instead:
- The **config stage** and the **spawn form** never need `Send`; one ungated
  impl.
- The **delivery + broadcast forms** live entirely inside the module-gated
  `sendable.rs` / `non_sendable.rs`; the `Send` bound is a property of *which file
  the type is defined in*, applied once at `#![cfg]` module scope. No method ever
  carries a `#[cfg]`.

## Scope

- Part A: ✅ **done.** `Mapper` removed from all builders; `mappers` field +
  application loop removed from `*ConsumingIter`s and `OnNext`;
  `TaskStatusMapper`, `FnMapper`, `FnOptionMapper`, `ZeroMapping`, and the
  `IntoBoxed*`/`Boxed*` aliases deleted. Zero references remain.
- Part B1-B3: ✅ **done.** `builders/{mod,sendable,non_sendable}.rs` created;
  `TaskSpawnConfig` + `as_*` transitions; four verb builders × two twins with
  `spawn/recv/stream/stream_with_config`; `#[cfg]` re-exports wired; both cfgs
  compile clean.
- Part B4: **pending.** Fold `multi/mod.rs`'s `ThreadPoolTaskBuilder`/`spawn2`
  onto the shared verb builders — `multi/mod.rs` still re-implements its own
  dispatch methods (`schedule_iter`, `schedule`, `spawn`, etc.) rather than
  routing through `builders/`.
- Migration: ✅ **done.** In-tree call sites (`actions.rs`, test callers,
  `sendables.rs`/`non_sendables.rs` wrappers) updated to `as_<verb>().{spawn,
  recv, stream}` surface.
- Preserve behaviour: dispatch semantics, delivery-queue bounding (Feature 45
  Part B: `sequenced` bounded, `lift`/`schedule` unbounded), and the Feature 45
  Part A vacancy-park are all unchanged; this is a surface reshape, not a
  behaviour change.

## Out of scope

- The Feature 45 delivery/fan-out **backpressure** semantics themselves (Part A
  vacancy-park, Part C split parks, Part D transport fan-out) — 46 only reshapes the
  builder that 45's delivery forms hang off, and fixes the `'static` bounds 45
  left on the `not(multi)` `*ConsumingIter` impls.
- The `EventReadinessPtr` cfg-split design (ratified by Feature 45 / Decision 00);
  46 consumes it, does not change it.
- Any new engine verb or delivery form; the surface is a 1:1 remap of today's
  methods onto verb builders.

## Open questions (resolve during implementation)

1. **`TaskSpawnConfig` vs. keeping the `ExecutionTaskIteratorBuilder` name.**
   Renaming clarifies the staged design but touches doc-comments/imports. Default:
   rename to `TaskSpawnConfig`, keep a `pub type ExecutionTaskIteratorBuilder<…>
   = TaskSpawnConfig<…>` alias only if an external name is depended on (none found
   yet).
2. **`recv()` return-name symmetry.** The spawn form returns `SpawnInfo`; `recv()`
   returns `NotifyRecvIterator<TaskStatus<…>>` and `stream()` returns
   `NotifyQueueStreamIterator<Done, Pending>` (Stream items). Confirm these are the
   exact types today's `*_iter` / `stream_*_iter` return so the remap is
   type-identical.
3. **Full removability of `TaskStatusMapper`.** Audit `OnNext` and the pool paths
   for any residual `map()` reliance before deleting the trait outright; if one
   remains, scope the deletion and note the survivor.
4. **`on_send_next` placement.** `on_send_next` / `on_send_next_mut` are `Send`-
   bounded config setters — decide whether they live on the ungated config impl
   (guarded by their own `Send` where-clause, as today) or move into
   `sendable.rs`. Default: keep as today (own `Send` where-clause on the config).

## Acceptance criteria

- `cargo check -p foundation_core` (single/wasm) **and**
  `cargo check -p foundation_core --features multi` both compile clean — no
  per-function `#[cfg]` in the builder modules; the only `#[cfg]` is the module-
  scope `#![cfg]` on `sendable.rs`/`non_sendable.rs` and the `mod.rs` re-exports.
- The four verb builders present an **identical** `spawn/recv/stream/
  stream_with_config` signature set in both twins; a portable call site
  (`spawn_builder(e).with_task(t).as_sequenced(p).stream(wait)`) compiles under
  both cfgs with no cfg at the call site.
- `TaskStatusMapper` and `with_mappers` are gone from the tree (or reduced to a
  documented residual); no `Mapper` type parameter remains on any builder; the
  `*ConsumingIter`s have no `mappers` field or application loop.
- All existing valtron tests pass on both cfgs (`#[valtron_test]`, never
  `#[test]`/`#[serial]`); behaviour (dispatch, bounding, vacancy-park) unchanged.
- `multi/mod.rs` no longer re-implements the builder dispatch; it routes through
  the shared verb builders.

## Module references

- `backends/foundation_core/src/valtron/executors/builders.rs` → becomes
  `builders/mod.rs`; add `builders/sendable.rs`, `builders/non_sendable.rs`.
- `backends/foundation_core/src/valtron/executors/task_iters.rs` — drop `mappers`
  from `*ConsumingIter`; fix the `not(multi)` `ExecutionIterator` `'static` bounds
  (Feature 45 residue).
- `backends/foundation_core/src/valtron/executors/on_next.rs` — drop mapper
  application.
- `backends/foundation_core/src/valtron/executors/multi/mod.rs` — fold
  `ThreadPoolTaskBuilder`/`spawn2` onto the shared path; drop `Mapper`.
- `backends/foundation_core/src/valtron/executors/{sendables,non_sendables}.rs` —
  update the high-level wrappers that consume the builder.
- `backends/foundation_core/src/valtron/executors/actions.rs` — update
  `spawn_broadcaster(…).broadcast()` call site.
- `backends/foundation_core/src/valtron/task.rs` — delete `TaskStatusMapper` &
  friends; `ExecutionEngine` verbs and `EventReadinessPtr` are read-only here.

## Language Stack

- **Rust** — all implementation

---

_Created: 2026-07-07_
