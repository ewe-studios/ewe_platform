# Spec 73 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

- 2026-11-07: feature 02 implemented on `spec73/f02-connectrpc-client-correctness`, commit
  `05bef17fd`, merged into `spec73/high-severity-fixes` as `832897fba`. `bidi_stream` takes
  only `ctx`; the generators and every caller use the facade's `send`. Every client response
  path picks a decompressor from the response head. `foundation_connectrpc` 0.0.7 to 0.0.8;
  `foundation_proxy` and `foundation_deployment_docker` requirements follow. The agent saw
  all four real defects fail on `48421e32c`. Filed #378 (high: request compression corrupts
  server-streaming requests) and #379 (low: the codegen crate fails clippy).

- 2026-11-07: feature 04 implemented on `spec73/f04-http-connection-correctness`, commit
  `a536471a8`. `KeepAliveConfig::idle_timeout` (75 s, nginx), the same field on
  `WebSocketConnectInfo`, and `WebSocketTask::with_idle_timeout`; the ignored `read_timeout`
  now sets the idle timeout. Old-code failures seen (101 ms vs 300 ms; 120 s vs 75 s).
  `foundation_http` 77 passed, `foundation_netio` 1302 passed. #284 confirmed fixed. Filed
  #375, #376 and #377. Both crates bump to 0.0.9 in a follow-up commit on the same branch.

- 2026-11-07: feature 10 implemented on `spec73/f10-macos-vm-ios-sdk`, commit `a8e9b73b9`, merged
  into `spec73/high-severity-fixes`. Pin `IOS_SDK_VERSION=26.2` (Xcode 26.3, the newest Xcode
  for the macOS 15 guest) in `components.env`; `provision.sh` installs and verifies it and
  writes `status/ios-sdk.txt`; the `macos-ssh` job reads that file and runs the feature 09
  `cargo check`. `bash -n`, PyYAML and `actionlint` pass. The VM run is the remaining proof.
  Filed #373 (medium: the macOS 15 guest caps the iOS SDK at Xcode 26.3) and #374 (low: the
  README provisioning estimate omits the 8 GB download).

- 2026-11-07: feature 09 implemented on `spec73/f09-ios-wasmtime-gating`, commit
  `94c4da99b`. `foundation_wasmtime` is optional and target-scoped; the Wasm items are absent
  on iOS, tvOS and watchOS. `cargo tree --target aarch64-apple-ios` counts 0. The local iOS
  `cargo check` cannot pass on this Mac: the iPhoneOS SDK is missing, and the build now stops
  at `objc2-exception-helper`, past wasmtime. The full iOS proof stays with feature 10.
  The feature found #372 (high, `get_responder` returns a reference its lock no longer
  protects) and #371 (medium, `WasmtimeResponder::respond` never calls `handle_request`). The
  #372 is fixed on the same branch in `db320c0fe`: the registry holds `Arc<dyn RouteResponder>`
  and `get_responder` returns the `Arc`; `foundation_platform` is 0.0.4; 220 tests pass. Both
  commits are merged into `spec73/high-severity-fixes` (`befa6597e`, `cd0384abc`).

- 2026-11-07: feature 07 verified (#317) in commit `1879b9e04`, merged as `0aef12e2a`. The
  generator emits `Option<bool>` (`generator.rs:2119-2123`) and serialises `"true"`/`"false"`
  (`:2693-2697`); `extract_boolean_query_params` fills the set. An existing test covered the
  mapping; the agent added one for the extraction step. The crate has no query-string parser,
  so a `?flag=true` parse assertion is not possible here. No new defect.

- 2026-11-07: feature 08 verified (#324). The main checkout resolves
  `target_directory` to `<repo>/target`; a worktree resolves it to `<worktree>/target`. They
  differ. `scripts/ci/ci.py:37-50` adds the same rule for CI, and `.agents/AGENTS.md:122-125`
  states it. Verification found #370: the `.agents` gitlink pinned commit `fb8b44f`, which was
  only on a local master, so a fresh clone could not init the submodule. Pushed
  `e295ca5..fb8b44f`; a fetch of the pinned SHA now succeeds; #370 is closed.

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
- **02-connectrpc-client-correctness** — implemented (#259, #261; #260 is fixed and closes
  with evidence). Commit `05bef17fd`, merged as `832897fba`.
  `features/02-connectrpc-client-correctness/feature.md`
- **03-connectrpc-h3-trailers-and-headers** — pending (#258; #253 is fixed). Design complete.
  `features/03-connectrpc-h3-trailers-and-headers/feature.md`
- **04-http-connection-correctness** — implemented (#288; #284 confirmed fixed). Commit
  `a536471a8`, plus the 0.0.9 bump.
  `features/04-http-connection-correctness/feature.md`
- **05-http1-rpc-worker-parking** — pending (#254, fixed on master). Verification plan ready.
- **06-h3-webtransport-accept** — pending (#273, fixed on master). Verification plan ready.
- **07-openapi-boolean-generation** — verified (#317). Commit `1879b9e04`.
  `features/07-openapi-boolean-generation/feature.md`
- **08-worktree-target-isolation** — verified (#324). The two `target_directory` values differ,
  and `ci.py:37-50` gives each checkout its own directory. The rule is already in
  `.agents/AGENTS.md:122-125`. The verification found #370: the submodule pinned an unpushed
  commit. Fixed; #370 is closed.
  `features/08-worktree-target-isolation/feature.md`
- **09-ios-wasmtime-gating** — complete (#354, plus #372 fixed). Commits `94c4da99b` and
  `db320c0fe`.
- **10-macos-vm-ios-sdk** — implemented, VM proof pending (#355). Commit `a8e9b73b9`.

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
