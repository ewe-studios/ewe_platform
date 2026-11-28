# Progress: Context-derived async timer

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

## What landed

- **T1.** The timer thread moved to `backends/foundation_context/src/timer.rs`. The public
  names stay the same: `sleep_until`, `sleep_for`, `Sleep`, `call_at`, `TimerCall`. The
  module also holds `ContextTimer`. `foundation_core::valtron::timer` re-exports every item,
  so each existing path compiles. The poison recovery now uses
  `PoisonError::into_inner`, because `foundation_core::compati` is not visible from this
  crate. The thread start failure writes to standard error, because `foundation_context` has
  no `tracing` dependency and no dependency changed.
- **T2.** `Ctx::timer()` returns a timer. The order is: the `Env` timer, the
  `Clock::timer()` override, then a cached `ContextTimer` over `Ctx::clock()`. `Clock` has a
  new method `timer()` that defaults to `None`. `CtxInner` caches the derived timer in a
  `OnceLock`. Every constructor builds a fresh cache, so `with_clock` uses the new clock.
- **T3.** `notify_clock_advanced` is public in `foundation_context::timer`.
  `SimulatedClock::advance` and `advance_wall` call it.
- **T4.** `foundation_event_stream::send_bounded` uses `Ctx::timer()`. It no longer falls
  back to an unbounded send when the context has no `Env`. GitHub issue #1090 is closed by
  this change. A `delivery_timeout_ms` of 0 still disables the engine bound on purpose.
  `foundation_agent_rpc::turn` and the two timer call sites in
  `foundation_ci::execution` use `Ctx::timer()` too.
- **T5.** On wasm, `Ctx::timer()` returns the `Env` timer. `Ctx::try_timer()` returns
  `TimerError::Missing` when no `Env` is installed. The native thread module compiles only
  with `cfg(not(target_family = "wasm"))`. `cargo check --profile uat --target
  wasm32-unknown-unknown -p foundation_context` passes.

## Tests

- `backends/foundation_context/tests/ctx_timer.rs`: `env_timer_overrides_the_default`,
  `clock_timer_wins_over_the_default`, `ctx_timer_defaults_to_the_clock`,
  `dropped_sleep_leaves_no_entry`.
- `backends/foundation_context/tests/timer_wasm.rs`:
  `wasm_without_env_timer_is_a_typed_error`. The file compiles only for a wasm target. There
  is no wasm test runner in this workspace, so the test does not run locally; the wasm check
  proves that it compiles.
- `backends/foundation_core/tests/valtron/ctx_timer.rs`:
  `advancing_the_clock_wakes_a_ctx_timer_sleep`.

## What is next

- T6: add a JavaScript default timer for a wasm context with no `Env`. GitHub issue #1093
  records it at severity medium. The change needs a host `setTimeout` binding; this feature
  added no dependency.

## Verify results

- `cargo test --profile uat -p foundation_context`: pass.
- `cargo test --profile uat -p foundation_core --features test-clock`: pass.
- `cargo test --profile uat -p foundation_event_stream`: pass.
- `cargo test --profile uat -p foundation_agent_rpc`: pass.
- Clippy for `foundation_context` and `foundation_core`: clean with `-D warnings`.
- `cargo check --profile uat --target wasm32-unknown-unknown -p foundation_context`: pass.
