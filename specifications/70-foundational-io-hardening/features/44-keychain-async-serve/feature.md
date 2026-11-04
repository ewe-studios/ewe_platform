---
feature: "foundation_keychain: handlers run as ConnectionResult::Async with a real waker; bin recursion lint"
status: "pending"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_keychain"]
catalogue_rows: [5, 6]
---

# Feature 44: keychain server polls handlers correctly

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (backlog §22)

`src/server/native.rs` `Serve::serve` calls `poll_ready(provisioning_route(..))` and
`poll_ready(route(..))` (~lines 227, 236). `poll_ready` (~265) polls once with
`noop_waker` (~277) and panics on `Poll::Pending`. Any handler that awaits real I/O (a Turso
query, a channel) panics in production.

Second: `src/bin/keychain.rs:28` triggers the `recursion_depth_exceeding_limit`
future-incompatible lint on `DrivenStreamIterator<FutureTask<..>>: IntoIterator`.

## Fix

- Return `ConnectionResult::Async(Box::pin(async move { … route(..).await … write the
  response … }))`, as `foundation_http` handlers do after spec 70 feature 08. Move the owned
  data (method, path, body, bearer, an `Arc` of the context) into the future so it is
  `'static + Send`. Delete `poll_ready` and `noop_waker`.
- Bin: add `#![recursion_limit = "256"]` to `src/bin/keychain.rs` with a comment (same
  reason as `foundation_db/src/lib.rs`), unless a smaller type chain removes the warning
  cleanly — say which.

## Tests

- `keychain_handler_that_awaits_does_not_panic` (fails on master): a route whose store
  returns `Pending` once (a test store that yields once with a real wake) answers 200.
- Existing keychain server tests pass.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_keychain --profile dev
cargo check -p foundation_keychain --bins --profile dev 2>&1 | rg recursion   # no output
rg -n 'noop_waker|fn poll_ready' backends/foundation_keychain/src           # no output
```

## Done when

- [ ] New test fails on master (panic) and passes now. Both §22 entries deleted. CI green.
