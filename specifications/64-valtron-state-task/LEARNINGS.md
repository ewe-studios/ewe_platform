# LEARNINGS — 64-valtron-state-task

## Design decisions and rationale

- **Engine-owned `Ctx`, forked per ExecutionIterator.** The engine owns a root
  `Ctx` (standard elements: clock, config, RNG). At each ExecutionIterator
  boundary (top-level schedule and every spawn) it forks the root — `fork()`
  copies the type-map into a fresh `Arc<CtxInner>`. Sibling ExecutionIterators
  therefore get unique maps and never pollute each other.
- **`clone()` shares, `fork()` copies.** Within one ExecutionIterator the wrapper
  forwards its ctx to its inner task by `clone()` (cheap `Arc` bump, shared map).
  `fork()` clones the `HashMap` structure only — the `Arc` values stay shared — so
  it is O(entries) and values remain shareable on purpose.
- **`Ctx` by value everywhere, no borrow.** `Ctx` wraps an `Arc`; passing it by
  value is a cheap clone. No `&Ctx`/`&mut Ctx` (a `&mut Arc` would be a mutable
  ref to the pointer, not the map). The async transition's future owns its clone,
  so no self-referential borrow.
- **Native core iterators over inline macro output.** `StateTask`/`AsyncStateTask`
  are first-class `TaskIterator` types (parallel to `FutureTask`/`StreamTask`); the
  macro is a thin layer over them.
- **Async core erases the transition and future** (`Box<dyn Fn>` + `Box<dyn
  Future>`) so `AsyncStateTask<T, R, P, S>` is nameable.
- **Two-level `Pending` for async** (`TransitionPending::Awaiting | Domain(P)`).
- **Same-name struct** (`fn build_house` → `struct build_house`) relies on Rust's
  separate value/type namespaces.

## Challenges

- Forking must be per-ExecutionIterator, not per-poll — a per-poll fork resets a
  task's map every turn and loses entries inserted on earlier turns.
- The `impl Future`-in-type-position problem drove the async erasure decision.

## Patterns to reuse

- `FutureTask`'s poll loop (drain stale wake tokens → poll once → `Depends`/`Pending`).
- The `multi`/`!multi` cfg-split for `Send`/`!Send` impls.

---

_Version: 2.0 — Created 2026-10-18_
