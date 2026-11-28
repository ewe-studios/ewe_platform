# Spec 92 Progress: closing specs 77 and 83

_Last updated: 2026-11-28._

## Status at a glance

| Feature | State |
|---|---|
| 00 Live proof on the real services | in review: pull request #1144 fixes the Cloudflare create stall; #1055 (moto) merged |
| 01 DST fidelity: the real registry | stages A merged (#1116); stage B #1130 and stage C #1136 in review |
| 02 Sandbox follow-ups | in review: pull request #1111, 12 issues closed, #1104 fixed |
| 03 GCP KMS over gRPC | in review: pull request #1121, conflicts resolved |
| 04 Live cloud resources with the CLI | done: pull request #1103 merged; four live tests passed on 2026-11-28 |
| 05 Remove the auth back edge from connectrpc | planned: after #1121, fixes #1107 and #756 |

## What is already true

Specs 77 and 83 are merged on `master` (`348620340`, then `39cd005f1` and `c9a913dc0`). The
combined pull request #1009 and the dependency pull request #1040 are merged.

The owner rejected spec 77 feature 16 (Docker Sandboxes, `sbx`) on 2026-11-27; decision D18.

The owner confirms that `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_WORKERS_AI_API_TOKEN` are in
the shell environment and in `~/.userenv`, which unblocks spec 83 feature 00 task T7.

## Merged on 2026-11-28

#1116 (the `Wait` re-drive), #1055 (moto for AWS KMS), #1049 (the DST workspace isolation),
#1103 (the live AWS and Google Cloud KMS proof), #1102 (the three master failures), #1075
(the wasm `has_root` fix) and #1145 (the Linux `is_ok_and` repair of #1102).

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

Master red for Linux after #1102: its clippy rewrite called `is_some_and` on a `Result`, and
`control_from_cmdline` compiles only on Linux. #1145 repairs it. macOS clippy cannot see
`cfg(target_os = "linux")` code; the amd64 CI image can.

## Feature 01: the deterministic simulation test (DST) turn hang

- **Stage A** (merged): `Stream::Wait` means "the queue is empty; run the producer and check
  again". The parking adapters treated it as `Pending` and parked on a wake that a
  single-threaded engine never sends. `WakeableStream` gained `drives_its_producer()` and
  `can_make_progress()`; the five adapters re-poll a driving stream with a bounded budget
  (`DEFAULT_WAIT_REDRIVE_BUDGET`, 100 on native, 1 on wasm). It also fixed #1108 and #1109.
- **Stage B** (#1130): the real `LocalInstanceRegistry` and `AgentSession` run under the
  simulator; `ScriptedCatalog` is the only fake. `foundation_ai` splits `testing-mock` from
  `testing`, and `foundation_agent_rpc` gains `dst`. Closes #908.
- **Stage C** (#1136): `run_until` no longer counts the executing task (#1128); the adapters
  also re-drive `Init` and `Pending` (#1129). The DST turn now goes through
  `AgentServiceClient::chat_stream` over the in-memory network.

## Feature 02: sandbox follow-ups

Pull request #1111 closes #966, #969, #970, #972, #935, #1013, #1022, #1023, #977, #978,
#1008 and #1106. #1095 (the Firecracker guest network) waits for a Linux host with KVM.

**#1104 root cause:** `tests/slave/support.rs` started a second valtron pool and leaked the
process-wide lifecycle guard with `mem::forget(initialize_pool(1, Some(4)))`. Every later
`pool()` call blocked on the gate, which held `serve::tcp_accepts_the_token` and
`serve::tcp_needs_token` for two hours. The tests now use the one shared test pool.

Linux run in the amd64 CI image: 311 passed, 19 failed. Every failure is an emulation limit
(one syscall qemu does not implement, and the fixed 5 s call deadline); #1137 records it.

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

- **#1113** (`severity: medium`, fixed): a run after a teardown found a key with every version
  scheduled for destruction, and the destroyed version was still the primary. The task now
  creates a version when none is enabled and makes the newest enabled version the primary.
- **#1114** (`severity: low`, open): the teardown should retry a transient precondition error
  right after the billing link.

## Today's findings

- **#1125** (`high`, fixed by #1144): `ControlService::create` spun on `Stream::Wait` in a
  synchronous loop on the single-threaded wasm host, so the Durable Object stopped answering.
- **#1133** (`high`, fixed by #1144): the slave did not recognise the Cloudflare container
  runtime, because it checked only `/.dockerenv` and `/run/.containerenv`.
- **#1140**, **#1141**, **#1142** (`high`, in #1144): the next Cloudflare failures: a
  streaming scoped call refused by the route check, a gzip unary body decoded as JSON, and a
  missing model configuration in the Durable Object.
- **#1107** (`high`, feature 05): the optional `foundation_auth` dependency makes a Cargo
  cycle that blocks any crate that needs both.
- **#1105**, **#1110**, **#1117**, **#1118**, **#1119**, **#1122**, **#1137**, **#1138**,
  **#1139**: medium and low items that wait their turn.

## Next

1. Merge the open pull requests in the order the continuous integration (CI) agent holds:
   #1144 → #1121 → #1111 → #1112 → #1130 → #1136 → #1123.
2. One full `packages=all` run on the final master; close #1104 with its `sandbox-e2e` link.
3. Feature 05 after #1121.
4. The Cloudflare demo under `examples/`, after #1144 merges.
5. Feature 04 uses the REST path on the real project. The gRPC path is local only until
   #1122 adds a TLS transport; localgcp does not serve `DeleteCryptoKey`, so only the live
   test proves it.
6. The owner decides whether to take #1107's layering fix, which also fixes #756.
