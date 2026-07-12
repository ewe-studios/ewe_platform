# Decision: Make `SendSafeBody` implement `Sync`

**Date:** 2026-07-12
**Status:** Proposed (not yet approved)

## Context

`SendSafeBody` is the unified body type for HTTP requests and responses in
`foundation_netio`. It carries all body variants: `None`, `Text`, `Bytes`, and
four stream variants (`Stream`, `ChunkedStream`, `LineFeedStream`, `SseStream`).

It currently implements `Send` but **not** `Sync`. This means `&SendSafeBody`
cannot be shared across threads, which restricts how body-bearing types
(`PreparedRequest`, `SimpleResponse`, `SimpleIncomingRequest`, etc.) can be used
in concurrent contexts — particularly valtron task pipelines where multiple
tasks may need to hold a shared reference.

## Why now

Feature 01 (DynNetClient + PreparedRequestBuilder alignment) is moving all
deployment crates to use `DynNetClient` + `open_exchange()` for HTTP. The
`open_exchange()` path returns `HttpExchangeClientTask` which is
`Box<dyn TaskIterator + Send>`. Making `SendSafeBody` `Sync` would allow the body
data (status, headers, body bytes) to be shared across split/observer patterns
in valtron pipelines, simplifying the response processing path.

## Root cause

`SendSafeBody` is `!Sync` for a single reason: the four stream variants hold
`Box<dyn Iterator<Item = ...> + Send>` trait objects. A `dyn Iterator + Send`
trait object is NOT `Sync` — `Sync` requires an explicit bound on the trait
object. The item types (`Data`, `ChunkedData`, `LineFeed`, `ParseResult`) are
all constructed from `String`/`Vec<u8>`/`Option<String>`/`u64` — all `Send +
Sync`. It's purely the missing `+ Sync` on the trait object.

The alias in question (`foundation_core/src/valtron/iterators.rs:112`):
```rust
pub type BoxedSendIterator<T> = Box<dyn Iterator<Item = T> + Send>;
```
Needs to become:
```rust
pub type BoxedSendIterator<T> = Box<dyn Iterator<Item = T> + Send + Sync>;
```

## What changes are needed

### Core change (foundation_core — ~5-10 lines)

| File | Line | What |
|------|------|------|
| `valtron/iterators.rs` | 112 | `BoxedSendIterator<T>` — add `+ Sync` |
| `valtron/iterators.rs` | 29 | `CloneableSendIterator` trait — add `+ Sync` bound |
| `valtron/iterators.rs` | 26 | `CloneableSendBoxIterator<T,E>` — implicitly Sync via trait bound change; verify |
| `valtron/iterators.rs` | 133 | `SendableIterator<T>` trait — may need `+ Sync` if `TransformSendIterator` requires it |
| `valtron/iterators.rs` | 144-145 | `TransformSendIterator` — stores `Box<dyn Fn(T) -> Option<V> + Send>` and `Box<dyn SendableIterator<T>>`; update if Sync needed |
| `io/readers/mod.rs` | 506 | `DataBytesIterator` — wraps `Box<dyn Iterator<Item = Result<Data, E>> + Send>`; needs `+ Sync` |
| `valtron/executors/sendables.rs` | — | `DrivenTaskIterator`, `DrivenStreamIterator`, `DrivenRecvIterator` — `unsafe impl Send` exists; may need `unsafe impl Sync` if used behind the new bound |

### Concrete iterators behind the trait object

The `+ Sync` bound means concrete types behind the `Box<dyn Iterator + Send +
Sync>` must also be `Sync`. Key types to verify:

| Type | Location | Likely Sync? |
|------|----------|-------------|
| `PushableBodyReader` | `foundation_netio/src/shared/http/pushable_body.rs:95` | YES — holds `Arc<PipeInner<Bytes>>` (Arc is Sync when T: Sync) |
| Various test iterators | tests/ | YES — typically stateless |
| `WasmSseIterator` | `wasm/client/stream.rs:168` | Needs `unsafe impl Sync` added |

### No changes in consuming crates

Adding `Sync` is purely additive. `SendSafeBody` and all types embedding it
(`PreparedRequest`, `SimpleResponse`, `SimpleIncomingRequest`, etc.) gain
`Sync` automatically. The 83 files across 12 crates that reference
`SendSafeBody` do not require source changes — they just recompile.

## Blast radius

| Crate | Files | Usage pattern |
|-------|-------|--------------|
| `foundation_netio` | 21 | Definition, constructors, iterators, streaming |
| `foundation_http` | 16 | Server handlers, middleware, static files |
| `foundation_auth` | 10 | OAuth, JWKS, IDP server |
| `foundation_connectrpc` | 7 | Protocol encode/decode, dispatch, H3 serve |
| `foundation_testing` | 5 | Test servers (HTTP, SSE, TLS) |
| `foundation_ai` | 3 | OpenAI, Anthropic providers |
| `foundation_db` | 2 | R2 blobstore, D1 KV store |
| Other crates | 19 | Proxy, vectors, browser, deployment, core (docs) |
| **Total** | **83 files** across **16 crates** | |

The majority (70+ files) only construct `SendSafeBody::Text(...)`,
`SendSafeBody::Bytes(...)`, or `SendSafeBody::None` — the non-streaming variants
are already `Sync`. No changes needed in those files.

## Cross-platform (wasm32)

`SendSafeBody` is defined in `shared/` — no platform variants. The wasm client
uses the same type. The only wasm-specific iterator (`WasmSseIterator`) needs
`unsafe impl Sync` if used via `Box<dyn Iterator + Send + Sync>` — but it's used
for SSE parsing, not stored directly in `SendSafeBody`. Likely no wasm blocker.

## Risk assessment

- **Compile-time, not runtime**: Adding `+ Sync` to a trait object bound is a
  compile-time check. If a concrete type behind the box is `!Sync`, the compiler
  rejects it. There's no silent behavior change — either it compiles or it
  doesn't.
- **The only risk**: some concrete iterator uses `RefCell`, `Cell`, `UnsafeCell`,
  or raw pointers internally, making it `!Sync`. The compiler will flag this.
  Based on code review, the main concrete iterators use `Arc`-based primitives
  and are likely already `Sync`-compatible.
- **Mitigation**: `cargo check --workspace` after the change. If failures occur,
  they're localized to the specific `!Sync` type and can be addressed case by
  case.

## Recommendation

**Proceed with the core type-alias change.** The change is:

1. One line in `foundation_core/src/valtron/iterators.rs` (line 112): add `+
   Sync` to `BoxedSendIterator`.
2. Update associated trait bounds (`CloneableSendIterator`, `SendableIterator`)
   to include `Sync`.
3. Update `DataBytesIterator` bounds in `io/readers/mod.rs`.
4. Add `unsafe impl Sync` for driven iterators if needed.
5. Run `cargo check --workspace` and fix any concrete type that fails the new
   bound.
6. Add a static assertion:
   ```rust
   const _: () = { fn assert<T: Sync>() {} let _ = assert::<SendSafeBody>; };
   ```

**Files requiring edits: ~3-5**
**Effort: Medium** (small code change, large recompilation surface)
**Backward compatibility: Fully additive** — `Sync` is an auto-trait, adding it
cannot break existing callers.

## Approval

- [x] Approved as standalone Feature 00 — SendSafeBody Sync (blocks Feature 01)
