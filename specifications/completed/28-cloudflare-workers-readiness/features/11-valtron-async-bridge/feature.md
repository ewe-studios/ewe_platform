---
feature: "Valtron Stream-to-Future Async Bridge"
description: "Add wrapper types that convert any StreamIterator into Rust Future and Stream types, bridging valtron's sync iterator model with async contexts"
status: "implemented"
priority: "high"
depends_on: []
estimated_effort: "medium"
created: 2026-05-20
last_updated: 2026-05-20
author: "Main Agent"
tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100%
---

# Feature: Valtron Stream-to-Future Async Bridge

## Overview

Add wrapper types in `foundation_core/src/valtron/stream_future.rs` that convert any `StreamIterator` into standard Rust `Future` and `futures_core::Stream` types. This bridges valtron's synchronous `StreamIterator` model (which yields `Stream<D, P>` states via `.next()`) with async code that expects `Future<Output = T>` or `impl futures_core::Stream`.

No combinators — these are standalone wrapper types that own a `StreamIterator` and implement `Future` or `Stream` on it.

## Architecture

### Four Wrapper Types

| Type | Implements | What it does |
|------|-----------|-------------|
| `StreamCollectFuture<SI>` | `Future<Output = Vec<SI::D>>` | Drives iterator to completion, collects all `Next` values |
| `StreamReadyFuture<SI>` | `Future<Output = Option<(SI::D, SI)>>` | Drives until first `Next(D)`, returns value + remaining iterator |
| `StreamPendingFuture<SI>` | `Future<Output = Option<(SI::P, SI)>>` | Drives until first `Pending(P)`, returns context + remaining iterator |
| `StreamAsFutureStream<SI>` | `futures_core::Stream<Item = Stream<D, P>>` | Yields each `Stream<D, P>` as-is, one-to-one |

### Module Layout

```
backends/foundation_core/src/valtron/
├── streams.rs                    # add extension methods to StreamIteratorExt
├── stream_future.rs              # NEW: all four wrapper types
└── mod.rs                        # mod + pub use stream_future::*
```

Tests in `backends/foundation_core/tests/valtron/stream_future.rs`.

## Implementation Details

### Ownership via Option<SI> — no Box::pin needed

Each wrapper struct owns its `StreamIterator` via `Option<SI>`. The `Option` is used purely for `take()` semantics (extracting the iterator once resolved); there is no self-referential data, no pinned references, and no need to `Box::pin` the inner iterator. The struct owns the iterator outright, so there are no dangling reference risks.

### Pin and Unpin requirements

`Pin<&mut Self>::get_mut()` requires `Self: Unpin`. Since the structs contain `Option<SI>`, and `Option<T>` is only `Unpin` when `T: Unpin`, all `Future` and `FuturesStream` impl blocks require `SI: Unpin`:

```rust
impl<SI> Future for StreamCollectFuture<SI>
where
    SI: StreamIterator + Unpin,
    SI::D: Unpin,
{
    // ...
}
```

Additionally, `Vec<SI::D>` (used in `StreamCollectFuture`) is only `Unpin` when `SI::D: Unpin`. This is a Rust stdlib quirk — `Vec<T>`'s internal `RawVec<T>` is not unconditionally `Unpin`. So `StreamCollectFuture` requires both `SI: Unpin` and `SI::D: Unpin`.

All valtron iterators (regular structs, `Vec` iterators, combinator wrappers) are `Unpin` by default since none contain self-referential data.

### Waker registration on Poll::Pending

When the poll loop encounters a `Stream::Pending`, `Stream::Delayed`, or `Stream::Init` state — or a `Stream::Next` when looking for `Pending` — it returns `Poll::Pending`. Since the wrapped iterator is synchronous and always has data ready, there is no real async wait. The `poll` implementation must call `cx.waker().wake_by_ref()` before returning `Poll::Pending`:

```rust
Some(Stream::Pending(_) | Stream::Delayed(_) | Stream::Init) => {
    cx.waker().wake_by_ref();
    return Poll::Pending;
}
```

**Why this is correct:** `Poll::Pending` contracts with the runtime: "I'm not resolved yet, but schedule me again." Without `wake_by_ref()`, the runtime never re-wakes the task, causing hangs under tokio/smol. Calling `wake_by_ref()` immediately signals "data is ready — poll me right away." The runtime coalesces the wake and re-polls on the same tick, so this does not introduce spurious scheduling overhead.

**Why returning `Poll::Pending` at all is correct (vs. looping until Ready):** `Stream::Pending` in valtron represents a real pause — e.g., an HTTP request in flight, a database query pending. Returning `Poll::Pending` lets the async runtime interleave other tasks during the wait. In a real async context, something would later change and the waker would fire. For the sync iterator test harness, `wake_by_ref()` simulates that "something changed" so the poll can continue.

### std / no_std pattern

- `Future` is in `core::future` — works in no_std.
- `futures_core` is already `no_std`-aware.
- Only `StreamCollectFuture` uses `Vec`, so only that type needs gating:

```rust
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct StreamCollectFuture<SI: StreamIterator> { ... }

#[cfg(any(feature = "std", feature = "alloc"))]
impl<SI> Future for StreamCollectFuture<SI>
where
    SI: StreamIterator + Unpin,
    SI::D: Unpin,
{ ... }
```

`StreamReadyFuture`, `StreamPendingFuture`, and `StreamAsFutureStream` are all `no_std`-compatible (no allocation).

### StreamCollectFuture

```rust
pub struct StreamCollectFuture<SI: StreamIterator> {
    inner: Option<SI>,
    collected: Vec<SI::D>,
}
```

Poll loop:
- `Some(Stream::Next(v))` → push to `collected`, continue looping
- `Some(Stream::Ignore)` → skip, continue looping
- `Some(Stream::Pending(_) | Stream::Delayed(_) | Stream::Init)` → `wake_by_ref()`, return `Poll::Pending`
- `None` → return `Poll::Ready(take(&mut collected))`

Batch `Next` and `Ignore` items in one poll call since they're synchronous. Return `Pending` on anything that signals the stream isn't ready yet.

### StreamReadyFuture

```rust
pub struct StreamReadyFuture<SI: StreamIterator> {
    inner: Option<SI>,
}
```

- `Some(Stream::Next(v))` → return `Poll::Ready(Some((v, take(inner))))`
- `Some(Stream::Ignore)` → skip, continue
- `Some(Stream::Pending(_) | Stream::Delayed(_) | Stream::Init)` → `wake_by_ref()`, return `Poll::Pending`
- `None` → `Poll::Ready(None)`

Returns `Option<(SI::D, SI)>` — the `SI::D` value and the remaining iterator so iteration can continue. No `Clone` bounds needed on either `D` or `P`.

### StreamPendingFuture

```rust
pub struct StreamPendingFuture<SI: StreamIterator> {
    inner: Option<SI>,
}
```

- `Some(Stream::Pending(p))` → return `Poll::Ready(Some((p, take(inner))))`
- `Some(Stream::Ignore)` → skip, continue
- `Some(Stream::Next(_) | Stream::Delayed(_) | Stream::Init)` → `wake_by_ref()`, return `Poll::Pending`
- `None` → `Poll::Ready(None)`

Returns `Option<(SI::P, SI)>` — the `Pending` context and remaining iterator. No `Clone` bounds needed.

### StreamAsFutureStream

```rust
pub struct StreamAsFutureStream<SI: StreamIterator> {
    inner: Option<SI>,
}
```

One-to-one pass-through implementing `futures_core::Stream`:
- `Some(item)` → `Poll::Ready(Some(item))`
- `None` → `Poll::Ready(None)`

The `poll_next` method returns `Poll::Ready` for every item — the `Stream<D, P>` enum itself communicates states to the caller; the `futures_core::Stream` wrapper doesn't interpret them.

### Ergonomic constructors on StreamIteratorExt

Added to existing `StreamIteratorExt` trait in `streams.rs`:

```rust
fn into_collect_future(self) -> StreamCollectFuture<Self>
where Self::D: Clone;  // only needed for the existing Collect combinator, kept for consistency

fn into_ready_future(self) -> StreamReadyFuture<Self>;

fn into_pending_future(self) -> StreamPendingFuture<Self>;

fn into_future_stream(self) -> StreamAsFutureStream<Self>;
```

`into_collect_future` retains the `#[cfg(any(feature = "std", feature = "alloc"))]` gate and `Self::D: Clone` bound (matching the existing `collect()` combinator). The other three have no feature gate and no extra bounds beyond `StreamIterator + Send + 'static` (the blanket impl requirement on `StreamIteratorExt`).

## Testing

Tests in `backends/foundation_core/tests/valtron/stream_future.rs`.

### Test design principles

1. **Sync tests use noop_waker** for direct `poll()` assertions. The noop waker never re-schedules, so tests that expect multiple polls must call `poll_once` explicitly for each expected poll.

2. **Async tests use tokio and smol** to validate `.await` works across runtimes. Because the source `poll` implementation calls `wake_by_ref()`, these tests work naturally — the runtime re-polls on `Pending` without manual intervention.

3. **Test both target-first and target-after-other-states**: When the target variant (`Next`, `Pending`) comes first, the Future resolves in one poll. When it comes after non-matching states, verify the Future still correctly re-polls until it finds the target or exhausts.

4. **Exhaustion requires correct poll count**: For `StreamPendingFuture` with two `Next` values, each `Next` produces `Poll::Pending`, then exhaustion produces `Poll::Ready(None)`. Tests must do 3 polls, not 2.

### Sync test helpers

```rust
fn noop_waker() -> Waker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RAW_WAKER, |_| {}, |_| {}, |_| {},
    );
    const RAW_WAKER: RawWaker = RawWaker::new(core::ptr::null(), &VTABLE);
    unsafe { Waker::from_raw(RAW_WAKER) }
}

fn poll_once<F: Future + Unpin>(f: &mut F) -> Poll<F::Output> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    Pin::new(f).poll(&mut cx)
}

fn poll_stream_next<S: FuturesStream + Unpin>(s: &mut S) -> Poll<Option<S::Item>> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    Pin::new(s).poll_next(&mut cx)
}
```

### Sync test cases (noop_waker)

**StreamCollectFuture:**
- `test_collect_future_all_next_completes_in_one_poll` — all `Next`, single poll resolves
- `test_collect_future_with_pending_returns_pending_then_completes` — `Next, Pending, Next`: first poll returns `Pending`, second resolves
- `test_collect_future_skips_ignore` — `Ignore` values are skipped, only `Next` collected
- `test_collect_future_empty_returns_empty_vec` — empty iterator returns `Ready([])` immediately

**StreamReadyFuture:**
- `test_ready_future_returns_first_next_with_remaining` — first `Next` returned with remaining iterator
- `test_ready_future_skips_init_and_pending` — skips `Init` and `Pending` (each produces `Pending`), finds `Next` on third poll
- `test_ready_future_none_on_no_next` — `Ignore, Init`: two polls (`Pending` then `Ready(None)`)

**StreamPendingFuture:**
- `test_pending_future_returns_first_pending` — hits `Next` first (`Pending`), then finds `Pending` on second poll
- `test_pending_future_none_on_no_pending` — two `Next` values require 3 polls: `Pending`, `Pending`, `Ready(None)`

**StreamAsFutureStream:**
- `test_future_stream_yields_all_items_one_to_one` — each `Stream<D, P>` yielded as-is
- `test_future_stream_empty` — empty iterator produces no items

### Async test cases (tokio)

- `test_tokio_await_collect` — `vec![Next(1), Next(2), Next(3)].into_collect_future().await`
- `test_tokio_await_ready_future` — target `Next(42)` comes first, resolves in one `.await`
- `test_tokio_await_pending_future_first` — target `Pending("done")` comes first
- `test_tokio_await_pending_future_after_next` — `Next(1), Next(2), Pending("done")`: validates re-polling until target found
- `test_tokio_await_future_stream` — manual poll loop with noop_waker for full iteration

### Smol runtime tests

- `test_smol_run_collect` — `smol::block_on` with `.into_collect_future().await`
- `test_smol_run_ready_future` — `Init, Next(99)`: validates `Init` produces `Pending`, then resolves
- `test_smol_run_future_stream` — manual poll loop for full iteration

### Combinator chaining tests

- `test_combinator_then_collect_future` — `.map_done(|v| v * 10).into_collect_future().await`
- `test_combinator_then_ready_future` — `.map_pending(|p| p.len()).into_ready_future().await`
- `test_combinator_then_future_stream` — `.map_done(|v| v * 2).into_future_stream()`

## Tasks

1. [x] Create `stream_future.rs` with four wrapper types and `poll` implementations with `wake_by_ref()`
2. [x] Wire module in `valtron/mod.rs`
3. [x] Add ergonomic constructors to `StreamIteratorExt` in `streams.rs`
4. [x] Add `tokio = { version = "1", features = ["macros", "rt"] }` and `smol = "2"` as dev-dependencies
5. [x] Write sync + async tests (tokio + smol + combinator chaining) in `tests/valtron/stream_future.rs`

## Verification

```bash
cargo check -p foundation_core
cargo check -p foundation_core --target wasm32-unknown-unknown
cargo test -p foundation_core --test mod -- valtron::stream_future
```

All 26 tests pass.

## Key Learnings

### Pin::get_mut() requires Self: Unpin

`Pin::get_mut()` on `StreamCollectFuture`, `StreamReadyFuture`, etc. requires the struct itself to be `Unpin`. Since these contain `Option<SI>`, and `Option<T>` is only `Unpin` when `T: Unpin`, the impl blocks need `SI: Unpin`. This is safe for all valtron iterators (they're regular structs).

### Vec<T> requires T: Unpin

`Vec<SI::D>` is only `Unpin` when `SI::D: Unpin` (due to internal `RawVec` implementation). `StreamCollectFuture` needs both `SI: Unpin` and `SI::D: Unpin` on its `Future` impl.

### wake_by_ref() is required on Poll::Pending

Returning `Poll::Pending` without calling `cx.waker().wake_by_ref()` causes the runtime to never re-poll, hanging forever. Since the wrapped `StreamIterator` is sync (data always ready), `wake_by_ref()` signals the runtime to re-poll immediately. This is correct Future semantics — not a workaround.

### No Clone bounds needed on Ready/Pending futures

`StreamReadyFuture` and `StreamPendingFuture` don't need `Clone` on either type parameter — they only pattern-match on values, never copy them.

### Test type annotation for empty iterators

Empty `vec![]` into `StreamAsFutureStream::new()` fails type inference. Use `Vec::<Stream<i32, &str>>::new()` or annotate the variable explicitly.

### noop_waker never re-schedules

In sync tests, the noop_waker's `wake()` is a no-op. Tests expecting `Pending` must manually re-poll. For a `StreamPendingFuture` with N `Next` values before exhaustion, you need N+1 polls to reach `Ready(None)`.

## Dependency Impact

No new runtime dependencies. `tokio` and `smol` are `[dev-dependencies]` for async tests only.

---

_Created: 2026-05-20_
