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

## Feature 02 (2026-11-28)

Branch `fix/sandbox-follow-ups`, pull request #1111. It closes the rows of this feature
that do not need a Linux host with KVM.

- The first change closed #935, #966, #969, #970, #972, #1013, #1022 and #1023. It also
  fixed the serve token test hang (#1104) with a watchdog and the host clock, and fixed a
  dead export variant (#1106). The Linux run then showed the root cause of the hang: the
  slave test support leaked a second valtron pool's lifecycle guard, so every later
  `crate::pool::pool()` blocked. The slave tests now use the one shared test pool, and the
  serve token tests pass on Linux.
- The second change closes #977. `UnpackBegin` reads the `ready` line as a readiness
  future, and `UnpackChunk` and `UnpackCommit` write and read through non-blocking pipes
  that park on writability. `tests/slave/unpack.rs` fills every task worker with writers
  to a probe child that reads nothing for three seconds, then shows that a queued task
  runs; one writer streams a 100 MiB archive. The test compiles and runs on Linux only.
- The third change closes #978 as a design decision. Spec 77 feature 12 section 6.6 now
  names the `exec` child form and the reason, with the decision date 2026-11-27.
- The fourth change closes #1008. It clears the pedantic lints of `foundation_jsonschema`
  and of `foundation_compact` in its wasm runs, and removes the `clippy-deny-warnings`
  override of `foundation_jsonschema` from `ci/packages.toml`. `foundation_compact` has no
  override.
- #1105 is fixed with the same feature. The slave test harness used `/tmp` as its work
  directory, and the plain-root set-up changed the mode of that existing directory; the
  runner user does not own `/tmp`, so `Prepare` failed with `EPERM` and 56 slave tests
  failed. The set-up now sets the mode only on a directory it makes, and the harness takes
  a work directory it owns from `EWE_SANDBOX_WORK_ROOT`, then `RUNNER_TEMP`, then the
  system temporary directory.
- #1095 (a Firecracker guest network) is left for its own change. It needs a Linux host
  with KVM, and this machine has none.

Verified on this host: format, clippy and tests of `foundation_sandbox`,
`foundation_deployment_docker`, `foundation_deployment_cloudflare`,
`foundation_compact` and `foundation_jsonschema`. The Linux-only test targets could not be
compiled here because no Linux cross C compiler is installed; the `sandbox-e2e` job
proves them.

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
5. Feature 02, #1095: the Firecracker guest network on a Linux host with KVM.
6. Feature 00, task T2: make `sandbox-cloudflare` green. The red run's log names the step and
   prints the failing value.
7. Feature 00, task T1: one live run against the owner's account, recorded here.
8. Feature 00, tasks T3 to T6: the KMS and WebCrypto tests.
