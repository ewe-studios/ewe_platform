# Learnings: Context-derived async timer

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

## The move goes one crate down

The timer must sit below `foundation_core`, because `Ctx` lives in `foundation_context` and
`Ctx::timer()` must build it. `foundation_core` depends on `foundation_context`, not the
other way. The move needs no new dependency.

The timer thread used two `foundation_core` helpers. Each helper has a standard replacement:

- `crate::compati::recover_poison` becomes `std::sync::PoisonError::into_inner`. The module
  is native only, so one poison type serves both lock and condition-variable waits.
- `tracing::error!` has no replacement in `foundation_context`, because the crate has no
  `tracing` dependency and the task forbids a new one. The thread start failure now writes
  one line to standard error. This is the only behavior difference in the moved code.

## A cached default needs care in the copy constructors

`CtxInner` has six constructors that build a new inner value: `with_rng`,
`incomplete_for_test`, `fork`, `with_empty_map`, `rebase` and `with_clock`. Each one now makes
a fresh `OnceLock`. A shared cache would be wrong in `with_clock`, because the cache would
hold the old clock. A fresh cache costs one `ContextTimer` per handle. `Ctx::clone` shares
the inner value, so it shares the cache.

The clock override is checked before the cache on every call. A clock that starts to provide
a timer after the cache is full still wins, which follows requirement 3.

## The timer holds the clock, not the `Ctx`

The `Timer` trait takes a `Ctx` argument. If `ContextTimer` stored that `Ctx`, the context
map would hold the timer and the timer would hold the context, an `Arc` cycle. `ContextTimer`
stores `Arc<dyn Clock>` instead. A new private `sleep_on(clock, deadline)` builds the sleep,
and the public `sleep_until(deadline, &ctx)` calls it with `ctx.clock()`. The deadlines,
wakers, thread wait and `Drop` code do not change.

## A wake must take the same lock

`notify_clock_advanced` takes the pending-map lock and then calls `notify_all`, as the old
`nudge` did. The timer thread holds the same lock while it reads each clock and enters the
condition-variable wait. A move of the clock and its wake therefore cannot fall between the
thread's check and its wait. This makes the one-hour simulated test repeatable.

## wasm has no thread

The native module uses `std::thread`, `Condvar` and process-wide state. A single-threaded
wasm host has none of these. The module is native only. On wasm, `Ctx::timer()` returns the
`Env` timer. A typed `try_timer()` returns `TimerError::Missing` when no `Env` exists, so no
caller waits forever.

The wasm fallback test compiles for `wasm32-unknown-unknown` but cannot run here: the
workspace has no wasm test runner for `foundation_context` and the task forbids a new
dependency. GitHub issue #1093 is the follow-up that adds a JavaScript `setTimeout` default.

## Call sites found more than the feature named

The feature names two call sites. `foundation_ci` had two more (`run_command` and `sleep`)
that required an `Env` timer and failed with `MissingService("timer")`. They now call
`Ctx::timer()`. `guarded_turn` keeps its `Env` parameter because the registry passes it, but
the turn no longer reads the field; the field has an `allow(dead_code)` comment.
