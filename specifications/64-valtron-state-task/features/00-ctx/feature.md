---
feature: "Ctx"
description: "Ctx = Arc<CtxInner>: a cloneable, Send + Sync handle over an interior-mutable type map, seeded by the engine with standard elements (clock, config, RNG)."
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "small"
created: 2026-10-18
last_updated: 2026-10-18
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0
---

# Ctx

## Overview

`Ctx` is the engine-provided, per-poll context handed to every task. It is a
cloneable `Send + Sync` handle over a `CtxInner`. Two operations: `clone()`
**shares** the map (a cheap `Arc` refcount bump — used within one
ExecutionIterator's subtree), and `fork()` **copies** the map into a fresh handle
(used by the engine at each ExecutionIterator boundary, so siblings never mutate
each other's map).

## Design

```rust
// foundation_core/src/valtron/ctx.rs

pub struct Ctx(Arc<CtxInner>);

pub(crate) struct CtxInner {
    // general extension point: arbitrary Arc<T> entries keyed by TypeId
    map: Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
    // engine-seeded standard elements
    clock: Clock,                 // read-only; Clone
    config: Arc<Config>,          // read-only; shared
    rng: Arc<Mutex<Rng>>,         // single shared stream (open decision)
}

impl Ctx {
    pub fn new() -> Self;                                              // empty map + default standard elements
    pub fn insert<T: Any + Send + Sync>(&self, value: Arc<T>) -> Option<Arc<T>>;
    pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>>;
    pub fn get_or_insert<T: Any + Send + Sync>(&self, value: T) -> Arc<T>;
    pub fn remove<T: Any + Send + Sync>(&self) -> Option<Arc<T>>;
    pub fn contains<T: Any + Send + Sync>(&self) -> bool;

    pub fn config(&self) -> &Config;          // &Config (deref Arc)

    # TODO, should these not be Arc<T>? especially the Mutex<Rng>
    pub fn clock(&self) -> &Clock;

    # TODO, should these not be Arc<T>? especially the Mutex<Rng>
    pub fn rng(&self) -> &Mutex<Rng>;         // lock() to draw

    /// A new handle over a *copied* map. The standard elements stay shared (Arc);
    /// the type-map is a fresh HashMap whose `Arc` values are shared. The engine
    /// calls this once per ExecutionIterator so siblings never mutate each other's map.
    pub fn fork(&self) -> Ctx;
}

impl Clone for Ctx { /* clones the Arc — SHARES the map */ }
impl Default for Ctx { /* Ctx::new() */ }
// Ctx: Send + Sync automatically — Arc<CtxInner> is Send + Sync iff CtxInner is
// Send + Sync, and every field (Mutex<HashMap<…>>, Arc<dyn Any + Send + Sync>,
// Clock, Arc<Config>, Arc<Mutex<Rng>>) is Send + Sync.
```

- **`Mutex`** is the codebase's `compati::Mutex` (`std::sync::Mutex` under `std`).
- **Type erasure** is `Arc<dyn Any + Send + Sync>`; `get` downcasts via
  `Arc::downcast::<T>()` (`T: Any + Send + Sync + 'static`).
- **Gating:** `#[cfg(any(feature = "std", feature = "alloc"))]`.
- **`fork()` copies the map structure, not the values.** Each entry is an `Arc`,
  so the fresh `HashMap` holds shared `Arc` pointers — structural isolation
  (insert/remove/replace are private) while the values stay shareable on purpose.

```mermaid
sequenceDiagram
    participant E as Engine (owns Ctx)
    participant T as Task (gets ctx: Ctx clone)
    participant I as CtxInner (shared)
    participant S as Sub-task (another clone)

    E->>T: next_status(ctx.clone())
    T->>I: ctx.clock() / ctx.rng().lock() / ctx.insert(Arc<T>)
    T->>S: spawn(sub_task(ctx.clone(), …))
    S->>I: ctx.get::<T>()  # same Arc<CtxInner>
```

## Interface contract

| Method                           | Behaviour                  | Notes                                             |
| -------------------------------- | -------------------------- | ------------------------------------------------- |
| `new()`                          | empty map + defaults       | engine seeds further at init                      |
| `insert(Arc<T>)`                 | replace by `TypeId`        | returns prior entry                               |
| `get<T>()`                       | downcast                   | `None` if absent/mismatched                       |
| `get_or_insert(T)`               | wrap + insert if absent    | returns live `Arc<T>`                             |
| `remove<T>()` / `contains<T>()`  | remove / presence          |                                                   |
| `clock()` / `config()` / `rng()` | standard elements          | read-only / read-only / lock-to-draw              |
| `fork()`                         | copy map into a new handle | standard elements shared; map values `Arc`-shared |

## Open decision — RNG

Default recorded: a single shared `Arc<Mutex<Rng>>`. This is the simplest shape
that puts "the current RNG" in the shared Ctx, but it is a contention point and
loses per-task reproducibility. The alternative — per-task `Rng` seeded from the
engine seed — cannot live in a _shared_ `CtxInner` (all clones share one
`CtxInner`), so it would need a separate per-task seeding mechanism layered on
top. Confirm before feature 01 depends on it.

## Error handling

Infallible API. `get` maps both "absent" and "wrong type" to `None`. No `Ctx`
method returns a lock guard, so no lock can be held across an `await`.

## Trade-offs

- `Arc<dyn Any + Send + Sync>` erasure costs a `downcast` on read and an `Arc`
  allocation per entry — negligible next to task I/O.
- `Send + Sync` only is deliberate; `!Send` entries are deferred.
- `Mutex` (not `RwLock`) for the first cut; swappable later without an API change.

## Tasks

1. Add `ctx.rs` with `Ctx`/`CtxInner` (map + standard elements) and the methods.
2. Define/select `Clock`, `Config`, `Rng` concrete types (reuse `SeedableRng` where
   it fits); resolve the RNG open decision.
3. Re-export `Ctx` from `valtron/mod.rs`.
4. Unit tests: map round-trip; clone-shares-map; `Send + Sync` asserted at compile
   time; clock/config/rng reachable.
5. Doc example of the sub-task sharing pattern.

## Success criteria

- Map methods behave per the contract; clones observe each other's writes.
- `Ctx: Send + Sync` compiles; clock/config/rng are reachable from any clone.
