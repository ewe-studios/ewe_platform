# Spec 92 Progress: closing specs 77 and 83

_Last updated: 2026-11-28._

## Status at a glance

| Feature | State |
|---|---|
| 00 Live proof on the real services | not started |
| 01 DST fidelity: the real registry | stage A in review |
| 02 Sandbox follow-ups | not started |
| 03 GCP KMS over gRPC | not started |
| 04 Live cloud resources with the cloud CLI | done: pull request #1103; four live tests passed on 2026-11-28 |

## What is already true

Specs 77 and 83 are merged on `master` (`348620340`, then `39cd005f1` and `c9a913dc0`). The
combined pull request #1009 and the dependency pull request #1040 are merged.

The owner rejected spec 77 feature 16 (Docker Sandboxes, `sbx`) on 2026-11-27; decision D18.

The owner confirms that `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_WORKERS_AI_API_TOKEN` are in
the shell environment and in `~/.userenv`, which unblocks spec 83 feature 00 task T7.

## Work in review (2026-11-27)

- **Feature 01, stage A (the DST `Wait` hang)** is branch `fix/dst-wait-redrive`. Cause:
  `Stream::Wait` means "the queue is empty; run the producer and check again". The parking
  adapters in `stream_future.rs` treated it as `Pending` and parked on a wake that a
  single-threaded engine never sends. Fix: `WakeableStream` gains `drives_its_producer()`.
  A driving stream reports `true`, and the five adapters re-poll it on `Wait` with a bounded
  budget (`DEFAULT_WAIT_REDRIVE_BUDGET`, 100 on native and 1 on wasm). A driving stream whose
  engine is parked reports `can_make_progress() == false`, so the adapter parks instead of
  sleeping out the driver's wait budget once per re-poll. A queue-backed stream keeps the old
  park behaviour. Tests: `wait_redrives_a_driving_stream` (the real `execute` driver under
  `SimRuntime`, one poll), plus `wait_redrive.rs` (park, bound, reset, wrapper forwarding,
  parked driving stream, property test). It also fixes #1108 and #1109, which made the default,
  `multi` and `testing` CI steps red.
- **Feature 00, task T3 (AWS KMS locally)** is pull request #1055. It adds the endpoint
  override (`AWS_ENDPOINT_URL_KMS`), the moto test, `mise run kms:moto` and the free CI job
  `kms-local`. It also fixes a duplicate `Content-Type` (#1054) and makes `Decrypt` send
  `KeyId`, so a backend for one key cannot open another key's blob.
- The full `packages=all` run on the earlier master head is still working. It is superseded,
  but its `sandbox-cloudflare` log is read before it is discarded.

Local stack verified on this host: moto serves AWS KMS (create, encrypt, decrypt, data key,
alias) on port 5055; Vault serves Transit on 8200 with its engine enabled by an init service;
localgcp v0.6.0 serves Cloud KMS on h2c `:8091` and is not yet wired to a client.

## Feature 00 task T2: the Cloudflare create (2026-11-28)

The create timeout has two causes.

1. `ControlService::create` pulled the create progress iterator again in the same poll after
   `Stream::Wait`. The iterator is a driving valtron stream: `Wait` means that the queue is
   empty and that it must run again. On the single-threaded wasm host that loop starved the
   event loop, so the create never finished and the Worker stopped answering. The service
   now parks on the injected timer for `SandboxLimits::create_poll_interval` (4 ms). The
   Worker installs a JavaScript `setTimeout` timer in its `Ctx`: the executor's wasm timer
   drove the engine from inside the poll, re-entered the create task at
   `WorkerContainer::running`, and crashed the wasm instance on the runner (`Critical
   error`, then `Cannot invoke closure from previous WASM instance`). A self-wake instead
   wedged the local Worker, so the timer remains. The test
   `tests/control/service_create.rs` fails on the old code.
   The remote create uses the Worker's container boot deadline
   (`CloudflareConfig::boot_timeout`, 60 s) for the whole stream. The runner's create took
   23.8 s, so the unary defaults (5 s, then 10 s) were too short.
2. The slave did not detect the Cloudflare container runtime: the runtime writes neither
   `/.dockerenv` nor `/run/.containerenv`. The slave ran the virtual-machine boot duties,
   failed, and never listened on the slave port. It now also treats the `CLOUDFLARE_*`
   environment as a container (issue #1133).

Local result on macOS with `wrangler dev --local`: a create returns `Created` and the Worker
stays healthy. The local host runs the amd64 container under emulation, so the timings
depend heavily on the machine load.

The three follow-up defects are fixed on the same branch.

- #1140: the Worker route reads the sandbox identifier from one Connect envelope, so
  `Exec` and `FileRead` reach the Durable Object. A unary call keeps its bare JSON body.
- #1141: the Connect client decodes a gzip unary body that carries no `Content-Encoding`,
  and a body that a proxy compressed twice. The edge that fronts the local Worker
  compresses a chunked body and omits the header.
- #1142: the sandbox Durable Object builds its model configuration from `MODEL_*`, so a
  model spec reaches the model driver.

A second runner run (job `140770667220`) passed 7 tests: creates, `Exec` and the
container `Get` work, and the closure errors are gone. Two failures remained.

- A stop crash: `cleanup` asked `ContainerApi::running` on a container that had stopped.
  The Cloudflare host throws `Critical error` instead of answering false, and the Durable
  Object went down. The substrate handle now remembers that it destroyed the container,
  destroys at most once, and never reads the host state. A fake container that panics on
  `running` proves the old code fails.
- The 5 s call deadline: the runner's first `Exec` stream ended at 5.1 s because the
  transport read timeout was `rpc_timeout`, and `FileWriteBegin` answered 504 at 5.0 s
  because the slave call used `rpc_timeout`. `CloudflareConfig::call_timeout` (default
  30 s) is now the deadline for one call; the Durable Object applies it and the remote
  client uses it for `rpc_timeout`.

A third runner run (job `140784573393`) passed 11 tests: creates, `Exec`, the container
`Get`, the file writes and the model sandbox work. Four failures remained, now addressed.

- The index Durable Object read a `js_sys::Map` with `Object::values`, so `List` was always
  empty on a real Worker. It iterates the Map values now.
- An `Exec` stream error was replaced with `StreamLost`, which hid its cause. The reported
  error reaches the caller now, so the next runner log names the stage.
- The stop test counted the runtime's `proxy-everything` sidecar: its name contains
  `ewe-sandbox` and it stays after a stop. The e2e helper counts only the workload
  container.
- A relative guest path joins the workdir now (#1154), with the rule in `shared/spec`.

The end-to-end suite on this macOS host reports 7 passed and 10 failed. Every local
failure is the same one: `the substrate exited during the start (code None)` on the
emulated amd64 container. The local host does not show the runner's failures. The native
Cloudflare suite passes (92 tests). The native amd64 CI runner must confirm these fixes.

A fourth runner run (job `140809210247`) passed 13 tests. Four failures remained, now
addressed. The `List` and the `index_put_delete_list` tests pass.

- `no_internet`: the runner answered `port error: Cannot perform I/O on behalf of a
  different Durable Object (I/O type: TcpPortState)`. A Cloudflare container is an
  input/output object of the Durable Object that made it. The Worker adapter kept one
  `worker::Container` for its whole life. It now holds the Durable Object state and asks
  it for the container at each call, so the container and the TCP port belong to the
  object that uses them. The end-to-end test is the regression test, because this defect
  shows only on the real runtime.
- `stop`: the stop found a workload container after the destroy. The substrate handle
  marked the container as destroyed even when the destroy call failed, so `cleanup`
  skipped its only second chance. It sets the flag only after a successful destroy. The
  fake container fails one destroy, and `cleanup_retries_a_failed_destroy` fails on the
  old code. The stop test waits up to 10 s for the runtime to remove the container and
  prints the remaining names.
- `two_sandboxes`: the relative path `tmp/a` joins the workdir (`/workspace/tmp/a`), and a
  write does not make a missing parent. The test now runs `mkdir -p /workspace/tmp` first.
  The rule is recorded on #1154.
- `large_stream`: the command prints `x` then a newline. A stdout frame can end between
  the two bytes, so the old check `data.len() % 2 == 0` failed on a one-byte frame. The
  stream was correct; the check now follows the pattern across frame boundaries and
  checks the order.

A native boundary test also caught that `shared/control/service.rs` read `boot_timeout`
directly. `SandboxLimits::create_deadline()` gives the value, and the remote client reads
that accessor.

## Feature 04 result (2026-11-28)

`foundation-ci live-kms` passed against the real services:

| Item | Value |
|---|---|
| AWS region | `us-east-1` |
| GCP project, location | `ewestudiosnigeria`, `global` |
| Tests | 4 passed, 0 failed |
| Teardown | done: both AWS keys are `PendingDeletion` (7 days) and both aliases are deleted |

The Google Cloud key ring and two key entries stay listed. Google Cloud cannot delete a key
ring, and it keeps a key entry until every version finishes destruction. The run destroys
every version.

Two findings from the runs:

- **#1113** (`severity: medium`, fixed): a run after a teardown found a key with every version
  scheduled for destruction, and the destroyed version was still the primary. The task now
  creates a version when none is enabled and makes the newest enabled version the primary.
- **#1114** (`severity: low`, open): the teardown should retry a transient precondition error
  right after the billing link.

## Next

1. Feature 00, task T2 first: make `sandbox-cloudflare` green. The red run's log names the
   step and now prints the failing value.
2. Feature 00, task T1: one live run against the owner's account, recorded here.
3. Feature 00, tasks T3 to T6: the KMS and WebCrypto tests.
