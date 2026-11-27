# Spec 92 Learnings

## Why this spec exists (2026-11-27)

Specs 77 and 83 merged with three kinds of debt:

1. **Proof debt.** The real Cloudflare container path was red, and no live run had used the
   owner's account. The real AWS and GCP KMS paths had never run.
2. **Fidelity debt.** The DST harness used a scripted registry, so the production registry
   code was not under simulation.
3. **Small correctness debt.** Eleven findings stayed open because each feature could land
   without them.

This spec exists to pay those debts. A feature of this spec is done when its issues are
closed, not when its code compiles.

## The Cloudflare create stall (2026-11-28)

- **The layer:** the Durable Object (DO), not the client. The client deadline
  (`rpc_timeout`, 5 s) only reported the stall.
- **The spin:** `ControlService::create` consumed the driving create iterator in a
  synchronous `futures::stream::poll_fn` loop. `Stream::Wait` means "the queue is empty;
  run the producer again", so the loop called `next()` again. A wasm host has one thread
  and one event loop: the loop never returned to it, so every promise (DO storage, the
  container fetch, the timers) stayed pending.
- **The wide effect:** the loop held the whole isolate. A raw request that the entry
  refuses with no DO also timed out. This ruled out a simple "slow create" and named a
  synchronous loop as the cause.
- **The fix:** park on the injected timer for `create_poll_interval` (4 ms) on `Wait`,
  then pull again. The event loop runs between the pulls. The remote create also uses the
  create deadline `boot_timeout` (10 s) for the whole stream; the unary `rpc_timeout`
  (5 s) was too short for a cold container start.
- **`fix/dst-wait-redrive` (#1116):** it changes the five parking stream-to-future
  adapters. It does not change this hand-written loop, so it would not fix this failure.
  It stays useful for the adapters. Its sibling issues #1128 and #1129 cover the
  nested-engine case and the `Pending` case.
- **The second cause:** the slave used `/.dockerenv` and `/run/.containerenv` to detect a
  container. The Cloudflare runtime writes neither, so the slave ran the virtual-machine
  boot duties in a container and died. It now also checks the `CLOUDFLARE_*` environment
  (issue #1133).
- **The next layer:** the create fix alone cannot make the job green. The end-to-end suite
  now exposes three independent defects: the streaming route reads a Connect envelope as
  JSON (#1140), the HTTP client does not decompress gzip (#1141), and the sandbox DO has no
  model configuration (#1142).

## The DST turn hang is a `Wait` re-drive (2026-11-27)

`Stream::Wait` is not `Pending`. It means "run the producer and check again". On a
multi-threaded pool another worker sends the wake. On the single-threaded simulation engine
nobody else runs, so a parked `Wait` never wakes. The layer was the stream-to-future
adapter, not the readiness harness: `sim_honours_depends_readiness` passed, and the control
test passed with a producer thread.

The fix is a signal, not a new engine. `drives_its_producer()` is `true` for a stream whose
`next()` runs the engine. The adapter re-polls such a stream with a bounded budget. A stream
that never makes progress still parks after it wakes itself. The wasm budget is 1, so the
adapter yields to the event loop at once.

Two traps:

1. A wrapper that forwards `wake`/`wake_send` but not `drives_its_producer` or
   `can_make_progress` hides the fix or the regression. Every forwarding combinator now
   forwards both.
2. A `Wait` does not always mean "run again". The single-threaded driver also returns `Wait`
   after it sleeps out its wait budget for a parked task. Re-driving that `Wait` 100 times
   cost 10 s and broke `depends_on_wake_signal_parks_then_resumes`. The driver now answers
   `can_make_progress()`, and the adapter re-drives only when the engine has runnable work.
3. `foundation_core`'s default and `multi` CI steps were already red (#1108, #1109): the
   `sim_tests` target imported a module gated on `testing`, and `SimRuntime::start` had no
   `# Panics` section. Run clippy on the crate you change, not only on your own targets.
