# Spec 68 progress

_Last updated: 2026-11-20. Written in ASD-STE100 Simplified Technical English (STE)._

All six tasks are done. `requirements.md` is version 2.1.

## Results

| Check | Result |
|---|---|
| `cargo test -p foundation_core valtron` | 471 pass, 1 fail (pre-existing #608) |
| `MISE_ENV=wasm mise run test:wasm-testbed:deno` | 33 of 33 pass |
| `mise run test:wasm-testbed:browser` | 33 of 33 pass in Chromium |
| `foundation_browser` smoke | 18 of 18 pass in 3.5 s (was 6 of 18 in 61 s) |
| `foundation_http` static file tests | 10 of 10 pass |
| `cargo check` for the owned wasm tree | green |
| `cargo clippy` (core, browser, http, testbed) | green with `-D warnings` |
| `cargo fmt --check` (all changed crates) | green |

## What changed

### Test surface

`integration/wasm_tests/src/lib.rs` gained the helpers and 11 cases:

1. Ready future parks, then wakes.
2. Collect future parks, then wakes.
3. Next stream parks, then wakes.
4. Full-enum stream from `into_future_stream` parks, then wakes.
5. A host timer wakes the parked bridge.
6. The raw lens forwards control items.
7. `StreamPendingFuture` re-parks after a spurious wake.
8. `WakeRegistry` fires each waiter once.
9. A `NotSupported` stream self-wakes.
10. `run_future_iter` makes progress inline.
11. `NotifyQueue::wait_for_item` returns after the timeout.
12. `async_ready_future_awaits_a_host_push`: `.await`, no manual poll.
13. `async_next_stream_yields_values`: `.next().await`, then `None`.
14. `async_future_stream_forwards_items`: `.next().await`.
15. `async_collect_future_collects_pushed_items`: `.await` returns the values.

Case 11 fails with `can't sleep` when the `js-event-loop` feature is removed, so
it is a true regression test.

### Production fixes

1. `foundation_core/Cargo.toml`: `js-foundation-wasm` enables
   `foundation_nostd/js-event-loop` (#605).
2. `non_sendables.rs`: `run_future_iter` is no longer gated off on JS wasm
   (#606).
3. `foundation_browser`: `RpcConfig` holds the call, read and sleep timeouts;
   the default read timeout is 5 ms (#609).
4. `foundation_http`: `.mjs` maps to `application/javascript` (#611).
5. `foundation_testbed`: the browser verdict is the `test result:` line (#612);
   the runner uses `await WebAssembly.compile` and `await
   WebAssembly.instantiate` (#614).

`foundation_browser` is 0.0.8; `foundation_testbed` requires it.

## Issues

| Issue | Severity | State |
|---|---|---|
| #605 `js-foundation-wasm` feature | high | fixed in this change |
| #606 `run_future_iter` gate | high | fixed in this change |
| #607 `run_stream_iter` gate | medium | open |
| #608 pipe closed-readiness double wake | medium | open, pre-existing |
| #609 browser host `Page.enable` timeout | high | fixed in this change |
| #610 netio does not wake on send | high | open, root cause |
| #611 `.mjs` MIME | high | fixed in this change |
| #612 browser verdict line | high | fixed in this change |
| #613 `[DEPENDS]` already-ready warning | medium | open |
| #614 sync wasm compile/instantiate | high | fixed in this change |

## Next

1. Commit the change and close #605, #606, #609, #611, #612 and #614.
2. Keep #607, #608, #610 and #613 open.

## Known gaps

- The browser suite stays a local task. CI runs the Deno suites only.
- The netio root fix (#610) removes the need for the 5 ms read timeout.
- `run_future_iter` has no production caller today. The spec 83 path names it.
- Spec 88 feature 01 covers the deeper suites (cancellation, backpressure,
  `#[task]` transitions, the wasm-bindgen `mod` target on every host).
