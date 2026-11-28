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
  then pull again. The timer must be a host timer. On the native runner the executor's wasm
  timer (`sleep_until_async` through `into_ready_future`) drove the engine from inside the
  poll, re-entered the create task at `WorkerContainer::running`, and crashed the wasm
  instance (`Critical error`, then `Cannot invoke closure from previous WASM instance`).
  The macOS run did not show that crash. `sandbox_worker::ctx` now installs a JavaScript
  `setTimeout` timer, so a wait wakes from the host event loop and never drives the engine.
  A self-wake instead wedged the local Worker, because the engine kept the task runnable.
  The remote create uses the Worker's container boot deadline
  (`CloudflareConfig::boot_timeout`, 60 s) for the whole stream; a runner create took
  23.8 s, so the unary `rpc_timeout` (5 s) or the spec `boot_timeout` (10 s) were too short.
- **`fix/dst-wait-redrive` (#1116):** it changes the five parking stream-to-future
  adapters. It does not change this hand-written loop, so it would not fix this failure.
  It stays useful for the adapters. Its sibling issues #1128 and #1129 cover the
  nested-engine case and the `Pending` case.
- **The second cause:** the slave used `/.dockerenv` and `/run/.containerenv` to detect a
  container. The Cloudflare runtime writes neither, so the slave ran the virtual-machine
  boot duties in a container and died. It now also checks the `CLOUDFLARE_*` environment
  (issue #1133).
- **The next layer:** the create fix alone cannot make the job green. The end-to-end suite
  then exposed three independent defects, now fixed on the same branch:
  - A server-streaming scoped call sends a Connect envelope (a flag byte and a length), so
    the route check read the envelope as JSON and refused `Exec` and `FileRead` (#1140).
    The route now skips one whole envelope. A unary call keeps its bare body.
  - The edge that fronts the local Worker compresses a unary body and omits
    `Content-Encoding`; it also compresses an encoded response a second time (the request
    advertised gzip). A client cannot decode by the header alone, so the Connect client
    reads the gzip magic and decodes at most two more layers (#1141).
  - The sandbox Durable Object built `CloudflareConfig::default()`, so its model
    configuration was empty and every model create was `UnsupportedCapability` (#1142).
    The object now reads `MODEL_*` once and passes the result.
- **A slow container aborts the create.** The readiness wait says it calls `Info` until the
  slave answers, but a per-call `deadline_exceeded` was fatal. A container that misses one
  5 s call while it starts ended the create. `deadline_exceeded` is now retryable until the
  boot deadline (#1157).
- **A relative file path has no agreed owner.** The e2e writes `tmp/a` and the slave
  refuses it, because it requires an absolute guest path. Either the slave joins the
  workdir or the API says that the path is absolute (#1154).
- **A stopped container must not be asked whether it runs.** `cleanup` called
  `WorkerContainer::running` after the container had stopped. The host throws
  `Critical error` instead of answering false, and the Durable Object went down; that was
  the second runner crash (job `140770667220`). The handle now remembers that it destroyed
  the container, destroys at most once, and never reads the host state.
- **One call to a cold container or Durable Object needs more than 5 s.** On the runner,
  the first Exec stream ended at 5.1 s because the transport read timeout was
  `rpc_timeout`, and a `FileWriteBegin` answered 504 at 5.0 s because the slave call used
  the same 5 s. `CloudflareConfig::call_timeout` (default 30 s) now bounds one call; the
  Durable Object applies it, and the remote client uses it for `rpc_timeout`.
- **`Storage::list_with_options` returns a `js_sys::Map`.** The index Durable Object read
  it with `Object::values`, which sees no entries of a Map, so `List` was always empty on a
  real Worker. A native fake hides this; only the end-to-end test showed it. The index
  iterates the Map values now.
- **A stream error is not `StreamLost`.** `exec_session` and the event stream replaced
  every stream error with `StreamLost`, so the runner log could not name the failing
  stage. The reported error reaches the caller now; only a missing `Exit`, a bad sequence
  or a missing `Started` stay `StreamLost`.
- **The runtime proxy is not the sandbox.** The local Cloudflare runtime runs a
  `proxy-everything` sidecar for each container. Its name contains `ewe-sandbox` and it
  stays after a stop, so the stop test counted the sidecar as a leaked sandbox. The e2e
  helper counts only the workload container.
- **A relative guest path joins the workdir.** The slave required an absolute path, but
  the contract on #1154 is that a relative path joins the instance workdir, is normalised
  and must stay inside it. The resolver is in `shared/spec`, so the rule runs on every
  target.
- **`wrangler dev` gives a working TCP port to one container only.** The local container
  runtime refuses the port of every container after the first in one Worker process:
  `Cannot perform I/O on behalf of a different Durable Object (I/O type: TcpPortState)`.
  One test at a time on a fresh Worker shows it: `no_internet` passes the first time and
  fails the second time; `two_sandboxes` starts both containers and then fails on the
  first sandbox's `run`. The Worker code is not at fault, and it already takes the
  container from its own Durable Object state at each call. Issue #1221 tracks it. A
  test run that reuses a Worker is not a clean result; a full suite needs one container
  per Worker process, and `two_sandboxes` needs more than the runtime gives.
- **A `--keep-running` Worker harness still runs the tests.** `run.sh --keep-running`
  leaves the Worker up, but the script continues into the full `cargo test` suite. A
  person who starts it, prints the PID, and then runs one test by hand runs two suites at
  once. This produced duplicate sandboxes, leaked containers and a false
  `TcpPortState` result until the second suite was stopped. Read the whole script before
  reusing a harness for a manual experiment.
- **A destroy can fail, and the flag must follow the result.** The substrate handle set
  its stopped flag even when the destroy call failed. The cleanup then skipped its only
  second chance, and the container stayed. Set the flag only after a successful destroy.
  A fake container that fails one destroy is a small red test for this.
- **A stdout frame is not aligned to the command output.** A frame can hold one byte of a
  two-byte pattern. A check that assumes whole pairs fails on a correct stream. Follow the
  pattern and its order across frames.
- **A boundary module must not name a limit field.** The boundary test refuses the
  `boot_timeout` name in `shared/control/service.rs`. A small accessor on
  `SandboxLimits` keeps one value and satisfies the rule.

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
