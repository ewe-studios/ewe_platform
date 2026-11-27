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
