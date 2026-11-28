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

## A `Drop` that blocks a pool worker deadlocks the pool (2026-11-28)

The KMS local test hang (#1169) looked like a stuck error-body read. A trace shows that the
`400` body read ends. The hang starts one step later, when the test cleans up its keys. The
old `KeyCleanup::drop` spawned a thread that ran `block_on_future`, then joined. The `Drop`
runs on the pool worker that polls the test future. The join blocked that poll. The nested
drive inside the cleanup could not schedule its request. The workers only parked and
unparked until the watchdog fired.

Two lessons:

1. Run cleanup in the async body, not in `Drop`. Await the work on the same future. Wrap the
   assertions in `catch_unwind` when a failed assertion must still clean up. A blocking wait
   in `Drop` runs on a pool worker. A worker that never returns from a poll stops the pool.
2. The error-body path was healthy. Read the trace before you change the layer that the
   symptom names. The `400` response had `Content-Length: 34` and `Connection: close`. The
   body reader returned all 34 bytes, then `None`.

## macOS refuses a timeout on a peer-closed socket (2026-11-28)

The focused local-server test failed before it reached the body read. The server wrote the
whole response and closed. macOS then answered `EINVAL` to `SO_RCVTIMEO` in
`Connection::set_read_timeout_as`, and `send_request::hand_off` turned that into a failed
request (#1172). The Unix-domain arm already mapped this error through
`unix_timeout_result`. The TCP and TLS arms did not.

Lesson: BSD sockets reject a socket option after the peer shuts the socket down, even when
the option cannot matter. Map `EINVAL` for every socket arm, not only the Unix-domain one.
The response bytes and EOF are already at hand, so the missing timeout changes nothing.

