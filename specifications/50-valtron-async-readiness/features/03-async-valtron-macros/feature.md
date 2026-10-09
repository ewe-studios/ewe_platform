---
feature: "Async #[valtron] / #[valtron_test]"
description: "Accept async fn in the valtron entry/test macros by wrapping the body as a driven future; sync path unchanged"
status: "pending"
priority: "high"
depends_on: ["01-waker-queue-bridge"]
estimated_effort: "small"
created: 2026-06-17
---

# Feature 03: Async `#[valtron]` / `#[valtron_test]`

## Description

Today both macros REJECT `async fn`
(`backends/foundation_macros/src/valtron_entry.rs`):

```rust
if let Some(asyncness) = &func.sig.asyncness {
    return syn::Error::new_spanned(asyncness,
        "#[valtron] functions are synchronous — ... remove `async`").to_compile_error();
}
```

With F01 making futures park instead of busy-spin, we can drive an async body to
completion inside the engine. Replace the rejection with a wrapping path.

## Behaviour

- **Sync body** → today's expansion, byte-for-byte unchanged.
- **Async body** → wrap as a future, schedule on the engine, drive to
  completion, return the future's output. `return` / `?` inside the body keep
  their meaning (they exit the test/fn).

## Expansion sketch

The macro already sets up the pool guard (`initialize_pool` / `PoolGuard`) and
the `seed` / `threads` args. The async arm wraps the moved body:

```rust
// when sig.asyncness.is_some():
#test_attr            // #[test] for valtron_test; nothing for valtron
#(#attrs)*
#vis fn #name() #output {
    let __guard = #fc::valtron::initialize_pool(#seed, #threads);

    // Body moved into an async block; same return type so ? / return work.
    let __out = {
        let __fut = async move #block;
        // Drive to completion on the engine. Pick the helper that matches the
        // active executor; both rely on F01 so Pending parks, not spins.
        //   multi:   #fc::valtron::collect_one(#fc::valtron::execute(
        //                #fc::valtron::from_future(__fut), None))
        //   single:  #fc::valtron::run_until_complete-based driver
        #fc::valtron::block_on_future(__fut)   // ← see "Driver helper" below
    };

    ::core::mem::drop(__guard);
    __out
}
```

### Driver helper (foundation_core)

To keep the macro simple and executor-agnostic, add ONE helper in
`foundation_core` that drives a future to completion on whichever executor is
active and returns its output:

```rust
/// Drive a future to completion on the active valtron executor, returning its
/// output. Uses from_future + the executor's run-to-completion path. With F01,
/// Pending parks on the wake queue rather than busy-spinning.
pub fn block_on_future<F>(fut: F) -> F::Output
where F: Future + Send + 'static, F::Output: Send + 'static;   // multi
// (single/wasm variant without Send bounds, cfg-gated like from_future)
```

The macro calls `block_on_future` and stays tiny; executor specifics live in
foundation_core where `from_future` / `run_until_complete` / `execute` already
are.

> Note: `#[valtron]`'s existing guard handling (named guard outliving the body,
> explicit drop after) and the "no arguments" check stay. Only the
> async-rejection branch changes, plus calling `block_on_future` for async
> bodies.

## Usage after this feature

```rust
use foundation_core::valtron::valtron_test;

#[valtron_test]                       // emits #[test]; pool up/down; NO #[serial]
async fn fetches_and_parses() {
    let body = some_async_client().get("/x").await?;   // parks, doesn't spin
    assert_eq!(body.status, 200);
}

#[valtron(threads = 8)]
async fn main() {
    run_pipeline().await;
}
```

## Module changes

- `backends/foundation_macros/src/valtron_entry.rs` — replace the async
  rejection with the wrapping expansion; keep the sync path identical
- `backends/foundation_core/src/valtron/` — add `block_on_future` (multi +
  single cfg variants)
- Update module docs in `valtron_entry.rs`: async is now supported; the
  `#[valtron_test]` REPLACES `#[test]` / no `#[serial]` contract is unchanged

## Documentation updates

- `.agents/skills/rust-valtron-usage/skill.md` — add async examples for both
  macros (note: async bodies park via F01, no busy-spin)
- `backends/foundation_core/src/valtron/docs/` — short note that async entry
  points are supported and how the driver works

## Testing (use `#[valtron_test]` — including async forms)

- `#[valtron_test] async fn` with an `.await` that resolves → passes; output
  returned.
- `?` inside an async `#[valtron_test] -> Result<(), E>` short-circuits
  correctly on `Err`.
- `return` inside an async body returns from the test, not the async block.
- Sync `#[valtron_test]` unchanged (regression).
- An async body whose future is `Pending` then woken completes without
  busy-spinning (ties to F01 assertions).
- `#[valtron] async fn main()` in a small example binary runs to completion.
