---
feature: "Ctx"
description: "A cloneable Send + Sync scratch bag: Ctx wraps Arc<CtxInner>, and CtxInner holds an interior-mutable type map of Arc<T> entries keyed by TypeId."
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "small"
created: 2026-10-18
last_updated: 2026-10-18
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0
---

# Ctx

## Overview

`Ctx` is the per-task scratch bag the state-machine iterators (feature 01) and
the `#[valtron::task]` macro (feature 02) hand to the transition function. It is
**owned by the task**, `Send + Sync` by default, and cheaply cloneable, so the
same underlying map can be handed to sub-tasks and to async futures.

## Design

```rust
// foundation_core/src/valtron/ctx.rs

/// A cloneable handle to a shared, interior-mutable type map.
///
/// Clone is cheap (an `Arc` refcount bump) and shares the SAME map, so a
/// sub-task or an in-flight future mutates what the parent sees.
pub struct Ctx(Arc<CtxInner>);

pub(crate) struct CtxInner {
    map: Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl Ctx {
    pub fn new() -> Self;
    pub fn insert<T: Any + Send + Sync>(&self, value: Arc<T>) -> Option<Arc<T>>; // returns the previous entry, if any
    pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>>;
    pub fn get_or_insert<T: Any + Send + Sync>(&self, value: T) -> Arc<T>;       // wraps in Arc; returns the existing entry if present
    pub fn remove<T: Any + Send + Sync>(&self) -> Option<Arc<T>>;
    pub fn contains<T: Any + Send + Sync>(&self) -> bool;
}

impl Clone for Ctx { /* clones the Arc */ }
impl Default for Ctx { /* Ctx::new() */ }
// Ctx: Send + Sync holds automatically: Arc<CtxInner> is Send + Sync because
// CtxInner (Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>) is Send + Sync.
```

- **`Mutex`** is the codebase's `compati::Mutex` (`std::sync::Mutex` under
  `std`), matching the existing convention.
- **Type erasure** is `Arc<dyn Any + Send + Sync>`; retrieval downcasts via
  `Arc::downcast::<T>()`, which requires `T: Any + Send + Sync + 'static`
  (`Any` implies `'static`).
- **Gating:** `#[cfg(any(feature = "std", feature = "alloc"))]`, mirroring
  `FutureTask`. `Arc`/`HashMap`/`TypeId`/`dyn Any` all need `alloc`/`std`.

```mermaid
sequenceDiagram
    participant T as Task (owns Ctx)
    participant C as Ctx (Arc<CtxInner>)
    participant M as Mutex<Map<TypeId, Arc<dyn Any+Send+Sync>>>
    participant S as Sub-task (clone of Ctx)

    T->>C: insert::<Auth>(Arc<AuthToken>)
    C->>M: lock; map.insert(TypeId::of::<Auth>(), arc)
    T->>S: spawn(sub_task(ctx.clone(), …))
    S->>C: get::<Auth>() → Some(Arc<AuthToken>)   # same Arc<CtxInner>
```

## Interface contract

| Method | Behaviour | Notes |
|--------|-----------|-------|
| `new()` | empty map | |
| `insert(Arc<T>)` | replace by `TypeId` | returns prior entry; `T: Any + Send + Sync` |
| `get<T>()` | downcast `Arc<dyn Any + Send + Sync>` | `None` if absent or type mismatch |
| `get_or_insert(T)` | wrap + insert if absent | returns the live `Arc<T>` |
| `remove<T>()` | remove by `TypeId` | returns removed entry |
| `contains<T>()` | presence check | |

## Error handling

None — the API is infallible. `get` returns `Option<Arc<T>>` (a missing key and
a wrong type both read as `None`, which is the intended behaviour for a
type-keyed bag). The `Mutex` is only ever locked for the duration of a single
map operation; no `Ctx` method returns a guard, so no lock can be held across an
`await` by accident.

## Trade-offs

- **`Arc<dyn Any + Send + Sync>` erasure** costs a `downcast` on read and an
  `Arc` allocation per entry — negligible relative to task I/O, and it is the
  price of heterogeneous storage keyed by `TypeId`.
- **`Send + Sync` only** is a deliberate default; `Rc`/`!Send` entries are out of
  scope (see requirements Out of Scope).
- **Interior mutability via `Mutex`** (not `RwLock`) keeps the first cut simple;
  `RwLock` can be swapped in later without an API change if read-heavy workloads
  demand it.

## Tasks

1. Add `ctx.rs` with `Ctx`/`CtxInner` and the six methods; `#[cfg(any(feature =
   "std", feature = "alloc"))]`.
2. Re-export `Ctx` from `valtron/mod.rs`.
3. Unit tests: insert/get/remove/contains round-trip; `get` of wrong type → `None`;
   `insert` returns the previous value; clone shares the map (mutate via one
   clone, observe via another); `Send + Sync` is asserted by a compile-time
   `fn assert_send_sync<T: Send + Sync>()` instantiation.
4. Doc example showing the sub-task sharing pattern.

## Success criteria

- All six methods behave per the contract table.
- Clones observe each other's writes.
- `Ctx: Send + Sync` compiles.
