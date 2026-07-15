---
feature: "Make SendSafeBody implement Sync"
description: "Add + Sync to BoxedSendIterator trait object bound in foundation_core; audit concrete iterators; add unsafe impl Sync where needed. Makes SendSafeBody and all body-bearing types (PreparedRequest, SimpleResponse, etc.) shareable across threads."
status: "completed"
priority: "high"
phase: 0
depends_on: []
estimated_effort: "small"
created: 2026-07-12
---
# Feature 00: Make `SendSafeBody` implement `Sync`

## Why

`SendSafeBody` implements `Send` but not `Sync`. This means
`&SendSafeBody` can't be shared across threads, restricting how body-bearing
types (`PreparedRequest`, `SimpleResponse`, `SimpleIncomingRequest`, etc.) can
be used in valtron pipelines — particularly in split/observer patterns.

Feature 01 (DynNetClient alignment) depends on this: `split_exchange()`
observers fan `HttpExchange` values through `ConcurrentQueue`s. When body data
flows through split queues, `Sync` on the body type simplifies sharing
semantics.

## Root cause

A single missing `+ Sync` bound on one type alias. In
`foundation_core/src/valtron/iterators.rs:112`:

```rust
pub type BoxedSendIterator<T> = Box<dyn Iterator<Item = T> + Send>;
```

The `dyn Iterator + Send` trait object is `!Sync` because `Sync` requires an
explicit bound. Every stream variant in `SendSafeBody` wraps this alias —
adding `+ Sync` makes the entire type `Sync`.

## What changes

### 1. `foundation_core` — type alias + trait bounds (~5 lines)

| File | Line | Change |
|------|------|--------|
| `valtron/iterators.rs` | 112 | `BoxedSendIterator<T>` → `Box<dyn Iterator<Item = T> + Send + Sync>` |
| `valtron/iterators.rs` | 29 | `CloneableSendIterator` trait → add `+ Sync` bound |
| `valtron/iterators.rs` | 133 | `SendableIterator<T>` trait → add `+ Sync` if `TransformSendIterator` needs it |
| `io/readers/mod.rs` | 506 | `DataBytesIterator` → add `+ Sync` to its `Box<dyn Iterator + Send>` |
| `valtron/executors/sendables.rs` | — | `DrivenTaskIterator`/`DrivenStreamIterator`/`DrivenRecvIterator` → add `unsafe impl Sync` if used behind the new bound |

### 2. Audit concrete iterators — verify they're `Sync`

The `+ Sync` bound means concrete types behind the trait object must also be
`Sync`. Key types to verify:

| Type | Location | Expected |
|------|----------|----------|
| `PushableBodyReader` | `foundation_netio/src/shared/http/pushable_body.rs:95` | Yes — holds `Arc<PipeInner<Bytes>>` |
| Various test iterators | tests/ | Yes — stateless |
| `WasmSseIterator` | `wasm/client/stream.rs:168` | Needs `unsafe impl Sync` |

### 3. No changes in consuming crates

`Sync` is an auto-trait — adding it is purely additive. The 83 files across
16 crates that reference `SendSafeBody` recompile without changes. `SendSafeBody`
and all types embedding it (`PreparedRequest`, `SimpleResponse`,
`SimpleIncomingRequest`, etc.) gain `Sync` automatically.

## Blast radius

| Crate | Files |
|-------|-------|
| `foundation_netio` | 21 |
| `foundation_http` | 16 |
| `foundation_auth` | 10 |
| `foundation_connectrpc` | 7 |
| `foundation_testing` | 5 |
| `foundation_ai` | 3 |
| `foundation_db` | 2 |
| Other crates | 19 |
| **Total** | **83 files** across **16 crates** |

The majority (70+ files) only construct `SendSafeBody::Text(…)`,
`SendSafeBody::Bytes(…)`, or `SendSafeBody::None` — already `Sync`.
Only the stream variants need the fix.

## Verification

- `cargo check -p foundation_core` — compiles
- `cargo check -p foundation_netio` — compiles
- `cargo check --workspace` — all crates recompile clean
- Static assertion: `const _: () = { fn assert<T: Sync>() {} let _ = assert::<SendSafeBody>; };` passes

## Acceptance criteria

1. `SendSafeBody: Sync` — the type is shareable across threads
2. `PreparedRequest: Sync`, `SimpleResponse<SendSafeBody>: Sync` — all
   body-bearing types transitively gain `Sync`
3. No source changes in consuming crates (83 files recompile, 0 edits needed)
4. All existing tests pass — no regression
