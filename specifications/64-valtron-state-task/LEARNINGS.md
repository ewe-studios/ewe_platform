# LEARNINGS — 64-valtron-state-task

## Design decisions and rationale

- **Global `Ctx` was rejected.** An engine-provided `Ctx` threaded through every
  `ExecutionIterator` conflicted with the `ExecutionIterator`-owns-the-task model
  and with `Send` migration under `multi`. The `Ctx` is instead **owned by the
  task** and shared by cloning an `Arc<CtxInner>`.
- **Native core iterators over inline macro output.** `StateTask`/`AsyncStateTask`
  are first-class `TaskIterator` types (parallel to `FutureTask`/`StreamTask`); the
  macro is a thin layer over them. This keeps the primitive composable and usable
  with a plain closure.
- **`ctx: Ctx` by value, not `&mut Ctx`.** A `&mut Ctx` borrowed across `.await`
  is self-referential when the struct also owns the future. Passing `Ctx` (a cheap
  `Arc` clone) by value makes the future own its clone and removes the borrow
  problem entirely — and it is what enables sub-task sharing.
- **Async core erases the transition and future** (`Box<dyn Fn>` + `Box<dyn
  Future>`) so `AsyncStateTask<T, R, P, S>` is nameable; an `async fn`'s `impl
  Future` return is opaque and cannot be named in a generated struct field.
- **Two-level `Pending` for async** (`TransitionPending::Awaiting | Domain(P)`):
  "future mid-await" is a different fact from "author advanced state", and both
  must be expressible. The sync core needs no wrapper.
- **Same-name struct** (`fn build_house` → `struct build_house`) relies on Rust's
  separate value/type namespaces; the `non_camel_case_types` lint is `#[allow]`ed
  on the generated struct.

## Challenges

- The `impl Future`-in-type-position problem for the async generated struct drove
  the erasure decision in `AsyncStateTask`.
- Keeping the macro's expansion hygienic under renamed dependencies required
  `crate_paths::foundation_core_path()` (the `resolve_crate` pattern already used
  by `valtron_entry.rs`).

## Patterns to reuse

- The `FutureTask` poll loop (drain stale wake tokens → poll once → `Depends`
  when parked / `Pending` on self-wake) is the reference implementation for any
  future-wrapping core iterator.
- The `multi`/`!multi` cfg-split for `Send`/`!Send` `TaskIterator` impls is the
  template for the state-task impls.

---

_Version: 1.0 — Created 2026-10-18_
