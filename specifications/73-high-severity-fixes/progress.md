# Spec 73 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

- 2026-11-07: PR [#368](https://github.com/ewe-studios/ewe_platform/pull/368) archives spec
  72. PR [#369](https://github.com/ewe-studios/ewe_platform/pull/369) holds the four
  high-severity fixes, rebased on master.

- 2026-11-07: feature 01 is complete on `spec73/f01-connectrpc-grpc-status-conversion`
  (commit `028ca4a15`), merged into `spec73/high-severity-fixes` as `48421e32c`. The
  transmute is replaced by `Code::from_u32(status).unwrap_or(Code::Unknown)`. The test covers
  0..=255. The source gate finds no `mem::transmute`. The parser became a `#[doc(hidden)]`
  test hook, because an integration test cannot reach a `pub(crate)` function.

- 2026-11-07: full re-verification of all 14 issues against master. Six are already fixed and
  still open: #253, #260, #273, #284, #317, #324. Eight are real: #265, #258, #259, #261,
  #288, #254, #354, #355. Correction: #254 is fixed too (`ConnectionResult::Async`), so seven
  are real and seven are stale. Features 05, 06, 07 and 08 are now verification-only. The
  feature files record the measured state with line numbers.

- 2026-11-07: the 10 feature files are written under `features/`. Features 01–04 and 09–10
  hold a completed design. Features 05–08 hold the verification plan.

- 2026-11-07: four `severity: high` defects were already in flight before this spec started.
  They are separate fix branches, not spec 73 features. They merge first:
  - #334 `fix/334-wasm-init-bindings` (commit `79a96918a`): the Wasm and Cloudflare
    singleton context bag now reaches the app. Verified: wasm32 check passes, 73 host tests
    pass.
  - #340 `fix/340-shutdown-single-deadline` (commit `da28fb94d`): one absolute shutdown
    deadline. Measured 2.036 s before, 1.03 s after. 812 tests pass.
  - #347 `fix/347-file-watcher-event-queue` (commit `981f78a0b`): the watcher queues every
    event from one poll. The regression test fails on the old code. 13 watcher tests pass.
  - #348 `fix/348-toolings-background-job` (commit `429327b3f`): `run_background_job`
    returns `Err` instead of panicking; `ToolingError::BackgroundJob`; `foundation_toolings`
    0.0.8.
  All four are merged into `fix/high-severity-fixes` (merge `f0b9bf07b`) with clean merges.
  Integration tests pass: `foundation_core --features multi` exit 0; `foundation_toolings`
  12 + 11 + 8 pass; `foundation_nativeapis` (with and without the watcher target) pass;
  `foundation_http` pass.

- 2026-11-07: spec written. The input is the 14 open `severity: high` issues on 2026-11-07.
  The owner gave the rule: all high-severity issues are priority work.

## Feature status

- **01-connectrpc-grpc-status-conversion** — complete (#265). Commit `028ca4a15`. Merged
  into the spec branch. `features/01-connectrpc-grpc-status-conversion/feature.md`
- **02-connectrpc-client-correctness** — in progress (#259, #261; #260 is fixed and closes
  with evidence). Agent handed off from the spec branch.
  `features/02-connectrpc-client-correctness/feature.md`
- **03-connectrpc-h3-trailers-and-headers** — pending (#258; #253 is fixed). Design complete.
  `features/03-connectrpc-h3-trailers-and-headers/feature.md`
- **04-http-connection-correctness** — in progress (#288; #284 is fixed). Agent handed off.
  `features/04-http-connection-correctness/feature.md`
- **05-http1-rpc-worker-parking** — pending (#254, fixed on master). Verification plan ready.
- **06-h3-webtransport-accept** — pending (#273, fixed on master). Verification plan ready.
- **07-openapi-boolean-generation** — pending (#317, fixed on master). Verification plan
  ready.
- **08-worktree-target-isolation** — in progress (#324, fixed on master). Agent handed off.
  `features/08-worktree-target-isolation/feature.md`
- **09-ios-wasmtime-gating** — pending (#354). Design complete.
- **10-macos-vm-ios-sdk** — pending (#355). Design complete.

## Issues filed while writing the spec

- #365 low: `tests/watcher/native.rs` `poll_until_events` is dead code on non-Linux hosts.
- #366 medium: the ptrace backend ignores `ptrace::syscall` failures, so a tracee stays
  stopped.
- #367 low: the two Cloudflare example lockfiles pinned `foundation_nativeapis` 0.0.9. Fixed
  on the spec 72 feature 08 branch.

## Next steps

1. Open one PR for `fix/high-severity-fixes` (four fixes, #334, #340, #347, #348).
2. Verify the feature 01 and 04 reports.
3. Start feature 02 after 01 merges (same crate), then 03. Run 05–08 verification lanes.
4. Then 09 and 10.

_Created: 2026-11-07_
