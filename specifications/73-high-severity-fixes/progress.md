# Spec 73 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

- 2026-11-07: the spec pull request is [#393](https://github.com/ewe-studios/ewe_platform/pull/393).
  It has ten features and closes fourteen high issues. The `macos-ssh` job is the #355 proof.

- 2026-11-07: feature 03 implemented (#258 fixed; #253 verified) on
  `spec73/f03-connectrpc-h3-trailers-and-headers`, commit `190aaf5a8`, merged as `2ca84edf5`.
  `HandlerExchange` carries the handler header slot; the h1/POST, h2 and h3 pumps merge it
  into the response head. A late `set_headers` is `FailedPrecondition` with a documented rule.
  Five new tests; three failed on the old code. Counts: default 273, `--features h3` 291,
  `foundation_proxy` 130. `foundation_connectrpc` 0.0.10. Filed #389 (medium: other framing
  headers are not reserved) and #390, #391, #392 (low).

- 2026-11-07: feature 06 verified (#273, closed) on `spec73/f06-h3-webtransport-accept`,
  commit `30dfe9bad`, merged as `1834f0fcb`. The three cases hold: pending HEADERS keeps the
  stream (`connection.rs:487`), an ordinary request returns its headers (473-480), a pending
  write keeps the stream (498). The existing test
  `a_stream_accepted_before_its_headers_is_not_lost` pins it; the commit adds a doc note only.
  1302 default tests pass, and 59 through `--features quic --test http3`. Filed #386 and #387
  (medium: the quic code fails clippy, and the default runs skip every quic target) and #388
  (low: the http3 suites run twice).

- 2026-11-07: feature 05 verified (#254) on `spec73/f05-http1-rpc-worker-parking`, commit
  `003d11bae`, merged as `b425b17c2`. The new test
  `a_long_rpc_does_not_block_a_short_request` pins the parking path. A `TaskStatus::Ignore`
  mutation makes it fail in 5.05 s. Counts: default 77, `--features multi` 156,
  `--features "multi quic"` 159. The agent fixed `test_server_config_defaults`, which still
  asserted the 120 s idle default that feature 04 changed to 75 s. Filed #384 and #385 (low).

- 2026-11-07: #378 fixed on `fix/378-request-compression`, commit `124248bb5`, merged into
  `spec73/high-severity-fixes` as `b2a9e642c`. The streaming calls compress in the envelope
  writer, so `FLAG_COMPRESSED` is set; the client no longer pre-compresses. The writer also
  applies `compress_min_bytes` and `send_max_bytes` to streaming frames.
  `foundation_connectrpc` 0.0.9. Five new tests; the server-stream and compress-min tests
  failed on `3fdfd85a1`. Filed #380 (medium: the streaming writer drops its error), #381
  (medium: unary requests ignore both send limits), #382 (low: an unformatted proxy test)
  and #383 (low: the `send_max_bytes` doc says body, the code applies it per message).

- 2026-11-07: feature 02 implemented on `spec73/f02-connectrpc-client-correctness`, commit
  `05bef17fd`, merged into `spec73/high-severity-fixes` as `832897fba`. `bidi_stream` takes
  only `ctx`; the generators and every caller use the facade's `send`. Every client response
  path picks a decompressor from the response head. `foundation_connectrpc` 0.0.7 to 0.0.8;
  `foundation_proxy` and `foundation_deployment_docker` requirements follow. The agent saw
  all four real defects fail on `48421e32c`. Filed #378 (high: request compression corrupts
  server-streaming requests) and #379 (low: the codegen crate fails clippy).

- 2026-11-07: feature 04 implemented on `spec73/f04-http-connection-correctness`, commits
  `a536471a8` and `0236e7e2a` (the bump), merged into `spec73/high-severity-fixes` as
  `dec71174d`. `KeepAliveConfig::idle_timeout` (75 s, nginx), the same field on
  `WebSocketConnectInfo`, and `WebSocketTask::with_idle_timeout`; the ignored `read_timeout`
  now sets the idle timeout. Old-code failures seen (101 ms vs 300 ms; 120 s vs 75 s).
  `foundation_http` 77 passed, `foundation_netio` 1302 passed. #284 confirmed fixed. Filed
  #375, #376 and #377. Both crates are 0.0.9; 69 manifests and 8 lockfiles follow. The merge
  kept `foundation_nativeapis` at 0.0.10 (master) instead of the branch's older 0.0.9.

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
- **03-connectrpc-h3-trailers-and-headers** — complete (#258 fixed; #253 verified). Commit
  `190aaf5a8`, merged as `2ca84edf5`.
  `features/03-connectrpc-h3-trailers-and-headers/feature.md`
- **04-http-connection-correctness** — complete (#288; #284 confirmed fixed). Commits
  `a536471a8` and `0236e7e2a`, merged as `dec71174d`.
  `features/04-http-connection-correctness/feature.md`
- **05-http1-rpc-worker-parking** — verified (#254, fixed on master). Commit `003d11bae`,
  merged as `b425b17c2`.
- **06-h3-webtransport-accept** — verified (#273, fixed on master and now closed). Commit
  `30dfe9bad`, merged as `1834f0fcb`.
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

1. Open the single spec 73 PR from `spec73/high-severity-fixes`.
2. Read the PR CI, including the `macos-ssh` job that is the feature 10 proof for #355.
3. After the merge, mark the requirements completed and archive spec 73.

_Created: 2026-11-07_
