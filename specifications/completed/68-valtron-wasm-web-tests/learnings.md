# Spec 68 learnings

_Written in ASD-STE100 Simplified Technical English (STE)._

## The old "nothing runs on wasm" claim was false

Plan version 1.0 said that nothing exercised valtron on wasm. The measured state
on 2026-11-20 contradicts it:

- `integration/wasm_tests` runs 18 cases in embedded Deno, including the
  executor, JS yield, `WakeSignal`, `EventQueue`, `ReadinessWaker`, sleep and
  timeout.
- `examples/cf-valtron-counter` runs `execute` over a `TaskIterator` in Workers.
- `wasm_js_yield_integration.rs` runs 15 wasm-bindgen cases under Deno.

The real gap is narrower: the stream-to-future bridges and `WakeRegistry` have
no wasm runtime test. `tests/valtron/mod.rs` gates `bridge_park`,
`stream_future` and `wakeable_stream` with `not(target_family = "wasm")`.

**How to apply:** measure the suite and the examples before you write a gap
statement. Name the exact API and the exact file that lack coverage.

## The native template runs on wasm with the spin CondVar

`foundation_nostd::primitives::condvar` has two implementations. With `std` and
without `js-event-loop`, it re-exports `std::sync::Condvar`. With `no_std` or
with `js-event-loop`, it uses a spin-wait implementation whose `wait_timeout`
loops with backoff and never parks a thread. `js-event-loop` is the wasm-safe
choice, and the `js-wasmbindgen` backend already enables it.

`foundation_core`'s `js-foundation-wasm` feature does not enable
`foundation_nostd/js-event-loop`. The test crate adds a direct
`foundation_nostd` dependency with `features = ["std", "js-event-loop"]`. Cargo
unifies features per build, so `NotifyQueue` uses the spin `CondVar` there.

The bridges also call `announce()` on the first poll, and that sets
`woken_consumer = true` on `NotifyQueueStreamIterator`. An empty `next()` then
returns `Stream::Wait` without calling `wait_for_item`.

**How to apply:** port the native `NotifyQueue` template as it is. Do not write
a test-local stream. Enable the spin `CondVar` when the case may wait.

## The queue wait failure is observable and precise

`notify_queue_wait_returns_after_timeout` calls `NotifyQueue::wait_for_item` on
an empty queue. Without `foundation_nostd/js-event-loop` in
`js-foundation-wasm`, the case fails with:

```
panicked at std/src/sys/thread/unsupported.rs:45:5:
can't sleep
```

With the feature, it returns `NotificationItem::None` after the timeout. That
makes the case a true regression test for #605.

**How to apply:** when you fix a wasm feature wiring, run the case with the line
removed and keep the red output. A wiring bug is invisible until one case
reaches the path.

## `std::sync::Condvar` cannot wait on default wasm

On `wasm32-unknown-unknown` without `-C target-feature=+atomics`, std selects
`sys/sync/condvar/no_threads.rs`:

- `notify_one` and `notify_all` do nothing.
- `wait` panics with "condvar wait not supported".
- `wait_timeout` calls `thread::sleep`, and `thread::sleep` is
  `sys/thread/unsupported.rs`, which panics with "can't sleep".

With `+atomics`, std selects the futex CondVar over `memory.atomic.wait32`. The
owned build does not enable that feature. `src/valtron/doc.rs` names the old
"time not implemented" message; the blocker is real, but the message differs.

**How to apply:** use `foundation_nostd`'s spin `CondVar`
(`js-event-loop`) on wasm. Never rely on `std::sync::Condvar` to block.

## The `run_future_iter` gate is too broad

Commit `58a68b048` put `run_future_iter` in a group with the blocking helpers
and gave it `#[cfg(not(all(target_family = "wasm", any(feature =
"js-wasmbindgen", feature = "js-foundation-wasm"))))]`. Its single-threaded body
is lazy: `ThreadedIterFuture::execute` polls the future once per `next()` call
and returns `ThreadedValue::Waiting` on `Poll::Pending`. It does not block. The
function has no caller today, and `block_on_collect`'s doc still sends wasm
callers to it.

**How to apply:** remove the gate from `run_future_iter` and from its
`ThreadedIterFuture` and `ThreadedValue` imports. Keep it on `block_on_future`,
`block_on_collect` and `sync_*`, which do run to completion. `run_stream_iter`
has the same gate and stays gated for now; it has its own issue.

## `block_on_future` has two names

The public `executors::block_on_future` is gated off on JS-hosted wasm. The
hidden `valtron::__private::block_on_future` is the unchecked variant, and the
`#[valtron]` and `#[valtron_test]` macros use it. A wasm call to the macro path
still runs. Only the public path is absent.

**How to apply:** say "absent from the public path" in the spec and the test
names. Do not claim that no blocking driver exists.

## The browser failure was a 30 s WebSocket read timeout

`mise run test:wasm-testbed:browser` failed at `Page.enable`, but the browser
was fine. Instrumented frame timestamps showed every CDP command took about
29.7 s:

```
[0.27s]  -> Target.attachToTarget
[29.84s] <- attached + response
[30.11s] -> Page.enable
[59.84s] <- response
```

The netio WebSocket task drains one outbound message per poll, then blocks in
a read for `read_timeout`. A send does not wake it. With
`read_timeout = 30 s`, a command sent during the read waits 30 s, and the
command deadline is also 30 s, so it times out.

**How to apply:** in the CDP engine, use a short socket read timeout (5 ms) so
the task re-polls its outbound pipe. Put the timeouts in `RpcConfig`, not in
constants. The root netio wake defect is #610. A browser failure at
`Page.enable` with a 30 s delay points here, not at the test cases.

After all four fixes (timeouts, MIME, verdict, async compile), the browser gate
passes 29 of 29 in 13 s. The whole browser path was broken in four places at
once, and each defect hid the next.

## The browser harness had three more defects behind the latency

With the CDP timeouts fixed, the page still failed, in this order:

1. **`.mjs` had no MIME mapping.** The static handler sent
   `application/octet-stream`, and Chrome refused the module script. The page
   stayed at `running…` (#611).
2. **The verdict took the first output line.** `ok <case>` does not contain
   `test result: ok`, so a green run exited 1 (#612).
3. **The wasm compiled and instantiated synchronously.** Chrome refuses both
   `new WebAssembly.Module(bytes)` and `new WebAssembly.Instance(module, imports)`
   above 8 MB on the main thread. A debug module with 29 cases is larger.
   `await WebAssembly.compile` and `await WebAssembly.instantiate` fix it
   (#614).

**How to apply:** when the browser page shows only `running…`, check the served
MIME types and the browser console before the test logic. Use the async compile
in every runner.

## A plain `async fn` drives the bridges with `.await`

Four cases are ordinary `async fn`s. They call `into_ready_future().await`,
`into_next_stream()` with `.next().await`, `into_future_stream()` with
`.next().await`, and `into_collect_future().await`. No case calls `poll()`.

The producer runs in a `register_schedule` callback. When the bridge parks, it
registers the executor waker. The push fires that waker, the harness re-polls the
async body, and the `.await` resolves. All four pass in embedded Deno and in
Chromium.

**How to apply:** a consumer of a bridge needs no manual driving. Use `.await`,
and let the host timer (or I/O) fire the wake.

## Task names moved

The mise task `test:wasm-testbed:web` no longer exists. The subcommand is now
`browser`, and the task is `test:wasm-testbed:browser`. The Deno task lives in
`mise.wasm.toml`, so it needs `MISE_ENV=wasm`. Both tasks already run
`integration/wasm_tests` and pass `--profile uat`.

**How to apply:** read `mise.toml` and `mise.wasm.toml` before you write a run
command into a spec.

## CI already runs the Deno suite

The synthetic `wasm-tests` package in `ci/packages.toml` runs
`mise run test:wasm-testbed:deno`, the wasm-bindgen Deno suite and the wrangler
suite. It also compiles the standalone crate with `cargo check --tests
--target wasm32-unknown-unknown` and checks its formatting. The old note that
"CI is compile-gate only" is wrong for this crate. The browser run stays local.

## `#[wasm_test]` async cases re-enter through the host

`WaitUntil` in `src/lib.rs` returns `Poll::Pending` on every unfinished poll.
Each pending poll goes round the owned re-poll loop, which runs host timers and
microtasks. A test that waits on a host callback therefore makes progress only
because of that loop. A synchronous spin loop would block the loop and fail.
