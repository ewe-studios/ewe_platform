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
   now parks on the injected timer for `SandboxLimits::create_poll_interval` (default 4 ms)
   and then pulls again. The test `tests/control/service_create.rs` fails on the old code.
   The remote create also uses `boot_timeout` (10 s), the create deadline, for the whole
   stream instead of the unary `rpc_timeout` (5 s). A cold create takes about 3.5 s on the
   local emulated container, so 5 s was too short.
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

The end-to-end suite on the rebased branch reports 6 passed and 11 failed on this macOS
host. The passed tests are `bad_id`, `model_token`, `not_found`, `token` and the two
restart tests. The failing tests are `acceptance`, `container_route`, `large_stream`,
`list`, `model_sandbox`, `no_internet`, `stale_index`, `stop`, `two_sandboxes`,
`versions` and `index_put_delete_list`.

Every failure is a create or a stop on the emulated amd64 container under heavy machine
load. The container misses the 5 s `Info` call deadline (`transport connect failure:
Timeout occurred`), so the create now retries an `Info` timeout until the boot deadline
(#1157). `two_sandboxes` needs a contract decision on a relative file path (#1154). The
native amd64 CI runner must confirm the timings that this host cannot.

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
