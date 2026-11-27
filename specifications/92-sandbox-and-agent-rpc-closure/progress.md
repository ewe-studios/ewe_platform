# Spec 92 Progress: closing specs 77 and 83

_Last updated: 2026-11-28._

## Status at a glance

| Feature | State |
|---|---|
| 00 Live proof on the real services | not started |
| 01 DST fidelity: the real registry | stage B in review |
| 02 Sandbox follow-ups | not started |
| 03 GCP KMS over gRPC | not started |
| 04 Live cloud resources with OpenTofu | not started |

## What is already true

Specs 77 and 83 are merged on `master` (`348620340`, then `39cd005f1` and `c9a913dc0`). The
combined pull request #1009 and the dependency pull request #1040 are merged.

The owner rejected spec 77 feature 16 (Docker Sandboxes, `sbx`) on 2026-11-27; decision D18.

The owner confirms that `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_WORKERS_AI_API_TOKEN` are in
the shell environment and in `~/.userenv`, which unblocks spec 83 feature 00 task T7.

## Work in review (2026-11-28)

- **Feature 01, stage B (the real registry in the DST)** is branch `feat/dst-real-registry`.
  `foundation_ai` splits `testing` into `testing-mock` (the substrate, no thread pool) and
  `testing` (the substrate and `multi`, the old contract). `foundation_agent_rpc` gains `dst`
  (the local registry, the guarded turn and the mocks, and no `multi`, SQLite or TLS). The DST
  `TestWorld` runs `LocalInstanceRegistry<InMemoryStores>` and `AgentSession`;
  `ScriptedCatalog` is the only fake and returns a real `ModelHandle` over
  `MockModelProvider`. The four DST tests pass, and the 32-seed trace test passes with
  identical traces.
  Two defects appeared under the simulator. (1) The `foundation_core` parking adapter parks on
  `Stream::Pending`/`Init` from a driving stream; `foundation_ai` now drives past them in
  `TurnItems` (#1129). (2) The single-threaded `foundation_core` `run_until` counts the task
  that is executing, so a turn driven from inside the client reader task never returns
  (#1128). The DST calls the real registry directly for a turn as a workaround; the client
  transport still serves enrollment, health, create, list and destroy.
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

## Next

1. Feature 00, task T2 first: make `sandbox-cloudflare` green. The red run's log names the
   step and now prints the failing value.
2. Feature 00, task T1: one live run against the owner's account, recorded here.
3. Feature 00, tasks T3 to T6: the KMS and WebCrypto tests.
