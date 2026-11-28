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

## Feature 00 tasks T5 and T6 (2026-11-28)

- **A skip can look like a pass.** The Vault test started a dev container, but a Vault dev
  server mounts no secrets engine. The key call returned 404, the test returned, and the run
  was green. The test now enables Transit, and the job fails when the output names a skip.
- **`up --wait` fails on a one-shot service.** The `vault-init` service exits 0 after it
  enables Transit, and compose treats the exited container as a failed service. The job now
  waits for `vault` only and runs `vault-init` with `run --rm`.
- **A test filter can match no case and still pass.** The `kms_tests` target root is
  `tests/kms/mod.rs`, so its cases are top-level modules. The filter `kms::kms_vault_transit`
  selected nothing; the filter `kms_vault_transit` selects the test. The job now also fails
  when the output does not name the case.
- **A wasm test build compiles every target of the crate.** `wasm-testbed ... bindgen-web`
  runs `cargo build --tests` for the package, so every native test target and
  dev-dependency must also build for wasm32. The native suites now declare
  `cfg(not(target_arch = "wasm32"))`, and their file-system and database dev-dependencies
  use the same cfg. `socket2` (through `foundation_testing`) was the first failure.
- **Cranelift cannot build wasm.** mise sets `CARGO_PROFILE_DEV_CODEGEN_BACKEND=cranelift`.
  A direct `cargo build --target wasm32-unknown-unknown` then fails with "Support for this
  target has not been implemented yet". Use `--profile uat`, or unset the variable; the
  wasm-testbed tool removes it for its nested build.

## Feature 02: sandbox follow-ups (2026-11-28)

- **A plain layer body hides the decode.** The layer stream test must serve a real gzip
  layer, so one test proves the stream bound and the gzip decode. A plain body tests only
  the download.
- **Two flattened generated structs shadow a field.** The Docker `HostConfig` is an `allOf`
  of `Resources` and the other fields, so the generated `HostConfig` already holds
  `Devices`. Flattening the generated `Resources` again wrote `Devices: null`. One generated
  struct is enough.
- **A missing child is not a transport error.** The nested watcher treated every `get`
  error as a stopped child. Only `NotFound` and `SandboxStopped` mean gone; a transport
  error is retried for the child's boot timeout. A nested stop must also respect the outer
  grace and kill a child that outlasts it.
- **A real socket needs the host clock.** The serve token tests ran a real server on a
  simulated clock that no thread advanced. They now use the host clock and close the client
  before shutdown. A 30 s watchdog exits the test binary, so a wedged stream fails the job
  in seconds instead of after the job timeout.
- **Two pools in one test process deadlock.** The slave tests started a second valtron
  pool and leaked its process-wide lifecycle guard. Every later `crate::pool::pool()` call
  then blocked on the gate, so the serve token tests and the new unpack test waited until
  the job was cancelled. The slave tests now use the one shared test pool with
  `block_on_future`, and the serve tests pass (issue #1104).
- **A test timeout hides the cause.** The first Linux run of the branch showed the compile
  error and then a hang. The hang was the leaked pool gate, not a slow test. A test that
  waits on a shared gate needs a report that names the gate, or the log shows only
  "running for over 60 seconds".
- **A plain root must not change the mode of a directory it did not make.** The slave
  tests used `/tmp` as the work directory. `mkdir_p` called `chmod` on it, and the runner
  user does not own `/tmp`, so `Prepare` failed with `EPERM` (os error 1) in 56 tests. The
  same call would have removed the sticky bit of `/tmp` for a root caller. `mkdir_p` now
  sets the mode only on the directory that it creates. The harness takes its work
  directory from `EWE_SANDBOX_WORK_ROOT`, then `RUNNER_TEMP`, then the system temporary
  directory, and makes a directory it owns (issue #1105).
- **A pipe write must park.** The unpack child writes to a pipe. A blocking `write_all`
  holds a valtron worker for the whole unpack. The fix uses `FdReadiness` and a readiness
  future, and reads the `ready` line the same way. The multi pool has at least two task
  workers, so a one-worker engine is not available; the test fills every task worker and
  shows that a queued task still runs.
- **A test that needs a registry should start one.** The differential listing never ran
  because no host set `EWE_TEST_REGISTRY`. It now starts a local `registry:2` and pushes
  Alpine. The BusyBox forms were checked on Alpine 3.22.
- **Bare links and empty assertions are pedantic lints.** The wasm clippy run catches
  lints that the host run does not, such as missing `# Errors`/`# Panics` sections and
  unchecked `Duration` subtraction. Run the crate's wasm feature sets before an override
  is removed.
- **The Firecracker network is a separate change.** The existing smoltcp data plane is
  Layer 3, and virtio-net is Layer 2. It needs a TAP device in a private namespace and a
  Linux host with KVM, so #1095 waits.

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

