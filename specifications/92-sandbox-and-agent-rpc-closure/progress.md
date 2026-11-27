# Spec 92 Progress: closing specs 77 and 83

_Last updated: 2026-11-28._

## Status at a glance

| Feature | State |
|---|---|
| 00 Live proof on the real services | not started |
| 01 DST fidelity: the real registry | stage A in review |
| 02 Sandbox follow-ups | not started |
| 03 GCP KMS over gRPC | in review: pull request #1121 |
| 04 Live cloud resources with the cloud CLI | done: pull request #1103; four live tests passed on 2026-11-28 |

## What is already true

Specs 77 and 83 are merged on `master` (`348620340`, then `39cd005f1` and `c9a913dc0`). The
combined pull request #1009 and the dependency pull request #1040 are merged.

The owner rejected spec 77 feature 16 (Docker Sandboxes, `sbx`) on 2026-11-27; decision D18.

The owner confirms that `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_WORKERS_AI_API_TOKEN` are in
the shell environment and in `~/.userenv`, which unblocks spec 83 feature 00 task T7.

## Work in review (2026-11-28)

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
localgcp v0.6.0 serves Cloud KMS on h2c `:8091`.

## Feature 03: GCP KMS over gRPC, tested on localgcp

State: in review. Branch `feat/kms-grpc`, based on `feat/live-kms`.

### What landed

| Piece | Where |
|---|---|
| The vendored Cloud KMS protos, their source commit and sha256 | `backends/foundation_gcp_kms/proto/` |
| The build script that generates the messages with `buffa-build` | `backends/foundation_gcp_kms/build.rs` |
| `GcpKmsGrpcBackend` and `GcpKmsGrpcConfig` | `backends/foundation_gcp_kms/src/backend.rs` |
| The `services localgcp` and `services localgcp-stop` tasks | `backends/foundation_ci/src/services/localgcp.rs` |
| `StartedService::pid` | `backends/foundation_tasks/src/process.rs` |
| The gated tests and the proto manifest check | `backends/foundation_gcp_kms/tests/` |
| The CI workflow `kms-gcp-local` | `.github/workflows/kms-gcp-local.yaml` |
| The `KeyStoreError::Config` variant | `backends/foundation_auth/src/keystore/mod.rs` |

The feature is a new crate, not a feature of `foundation_auth`, because Cargo rejects the
cycle `foundation_auth -> foundation_connectrpc -> foundation_auth` (#1107). See
`learnings.md`.

### The gRPC calls on localgcp

| Call | Result on localgcp v0.6.0 |
|---|---|
| `CreateKeyRing` | served |
| `CreateCryptoKey` | served |
| `GetCryptoKey` | served |
| `Encrypt` | served; it ignores `additional_authenticated_data` |
| `Decrypt` | served; a wrong key returns garbage, not an error (#1117) |
| `GetCryptoKeyVersion` | served |
| `DestroyCryptoKeyVersion` | served; the version keeps the `DESTROYED` state |
| `DeleteCryptoKey` | not implemented: `unimplemented: method DeleteCryptoKey not implemented` |
| `GenerateDataKey` | not a Cloud KMS v1 method: `unimplemented: unknown method GenerateDataKey for service google.cloud.kms.v1.KeyManagementService` |
| `ScheduleKeyDeletion` | not a Cloud KMS v1 method: `unimplemented: unknown method ScheduleKeyDeletion for service google.cloud.kms.v1.KeyManagementService` |

### Open issues from this feature

- #1107, severity high: the `foundation_connectrpc` optional `foundation_auth` dependency
  closes a cycle. The feature works around it; the layering fix is open.
- #1117, severity low: localgcp does not authenticate `Decrypt` and ignores the associated
  data.
- #1119, severity low: the feature's task table names two AWS methods.
- #1120, severity medium: the pinned toolchain reported formatting drift in `foundation_ci`
  and `foundation_tasks`; the base branch formats `foundation_ci` and this change formats
  `foundation_tasks`, so the issue is closed.
- #1122, severity medium: the h2c transport has no TLS, so the gRPC path reaches a local
  emulator only. Feature 04's live proof stays on the REST path.

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

1. Merge pull request #1103 (`feat/live-kms`) first, then #1121 (`feat/kms-grpc`).
2. Read the `kms-gcp-local` workflow result on pull request #1121.
3. Feature 04 uses the REST path on the real project. The gRPC path is local only until
   #1122 adds a TLS transport; localgcp does not serve `DeleteCryptoKey`, so only the live
   test proves it.
4. The owner decides whether to take #1107's layering fix, which also fixes #756.
