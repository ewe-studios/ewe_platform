# Spec 72 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

- 2026-11-07: feature 10 (keychain streamed body, #197) is in review as PR #359. It is a
  test-only change to `tests/streamed_body.rs`: `read_response` returns a typed `ReadError`
  and maps an expired `SO_RCVTIMEO` to `Timeout` on macOS. The test holds both upload bodies
  open with `mpsc` ready channels, asserts the third request answers before the release, and
  releases the uploads one at a time. The stale one-use comment is gone. Twenty consecutive
  runs pass on macOS and 20 more in the Linux CI image. The sensitivity check broke the
  property on purpose and the test failed 3/3 at the progress assertion. Strict clippy stops
  on #330 and #323. Filed #357 (high, two concurrent Turso reads fail with "concurrent use
  forbidden") and #358 (low, `start_server` sleeps 150 ms instead of synchronizing).

- 2026-11-07: feature 09 (toolings clock and wake, #201, #203) is in review as PR #349.
  `mpp::Receiver` had no wakeable form, so `foundation_core` gains
  `mpp::Receiver::readiness()` and its senders wake the parked task. The builder task parks on
  the watch source and the result queue, drains with `try_iter`, and the never-popped
  `change_queue` is deleted. The sleeper already used the injected clock on master; its test
  now uses `SimulatedClock`. `foundation_core` is 0.0.18 and `foundation_toolings` 0.0.7, with
  every requirement and lockfile updated. Four tests; all four failed on the old code (deadline
  assertion, 2000 ms timeout, 5000 ms timeout with zero builds, "not ready after it drains")
  and pass now. `foundation_toolings` 25 passed; the wake binary is 20/20. `foundation_core`
  and `foundation_toolings` are clippy clean under `--no-deps`; #336 still stops the plain
  run. Filed #347 (medium, `FileWatcherTask` returns only the first event of a poll) and #348
  (medium, `submit_build` panics with no background pool).

- 2026-11-07: feature 04 (Docker container clock, #220) is in review as PR #345. The
  `fork_ctx().clock()` fallback is gone. `ContainerConfig::resolve_clock()` returns
  `DockerError::MissingClock`, and `start_async` resolves it before any I/O. `#[docker_container]`
  accepts `clock = <expr>`; without it the macro reads `pool_clock()` from the valtron pool and
  returns the typed error, or logs a skip for a `()` function, instead of panicking. This is a
  breaking change: `foundation_deployment_platform` is 0.1.5 and `foundation_macros` 0.0.11,
  with every requirement and all 10 example lockfiles updated. Five tests; three failed on the
  old code (two by compile error, one by panic) and pass now. Suites: 53 passed in
  `foundation_deployment_platform`, 99 in `foundation_macros`; the non-ignored macro tests ran
  against a real Docker daemon. The `#[ignore]` Docker integration tests were not run. Filed
  #346 (low, `pool_clock` can panic when the pool drops between the check and `fork_ctx`).

- 2026-11-07: feature 07 (ptrace dispatch, #205–#208) is in review as PR #344. `open` passes
  `arg2` and `openat` passes `arg3` as the mode; `flags_to_perm` is deleted. `dup`/`dup2` clear
  `O_CLOEXEC`, `dup3` reads `arg2`, and the fd range check returns `EBADF` against the tracee's
  soft `RLIMIT_NOFILE` from `/proc/<pid>/limits`. `O_APPEND` writes seek to the file size first.
  Two files changed and no public API change, so `foundation_nativeapis` stays 0.0.9. Five new
  tests, and each failed with the source fix reverted. Default-feature suite 42 passed; the
  Linux image ran 26 non-spawn ptrace tests; the 13 spawn tests fail under amd64 emulation on
  this arm64 host, as `ci/packages.toml` documents. Strict clippy still stops on `--no-deps`
  blockers #336 and #200. Filed #342 (low, `dup3` accepts flags other than `O_CLOEXEC`) and
  #343 (low, a `Directory` entry cannot record `O_CLOEXEC`).

- 2026-11-07: feature 02 (bounded background shutdown, #233) is in review as PR #339. New
  `BackgroundJobRegistry::shutdown_until(clock, deadline)` checks the deadline between jobs;
  `shutdown(clock)` is a wrapper that computes it from the configured timeout, and the new
  `shutdown_timeout()` accessor exposes that value. `PoolGuard::shutdown_within` passes
  `clock.now() + min(timeout, bg.shutdown_timeout())`. `foundation_core` is 0.0.17 with 64
  consumer requirements updated. `shutdown_within_bounds_a_stuck_background_job` failed on
  the old code (it waited the 30 s default) and passes now; the suite is 756 passed without
  `multi` and 808 with it, doctests 71 and 74. The design changed because `PoolGuard` does not
  hold `PoolConfig`. Two follow-ups filed: #340 (medium, the worker phase still gets a fresh
  full timeout, so one call can take about 2× the argument) and #341 (low, `wait_timeout`
  rebuilds the deadline and can drift).

- 2026-11-07: feature 05 (WASI time source, #219) is in review as PR #338. `foundation_wasm`
  gains a `TimeSource` trait and a `WasiHost` environment (`new().with_time_source(..).install()`,
  `uninstall()`); a missing source is `WasiHostError::TimeSourceNotInstalled`. The clock-reading
  functions return `Result` and never fall back to the real clock; the `wasi_*` exports map the
  error to documented sentinels. The marker is gone and the time-source gate reports 0
  violations. `foundation_wasm` is 0.0.7 with 27 consumer requirements updated. The premise in
  the issue was wrong: the read was a plain Rust helper, not an extern-C boundary. Two `wasi`
  tests pass and both fail with the old real-clock read restored. The strict clippy needed
  seven small pre-existing lint fixes in `foundation_wasm` files, reported in the PR. Linux CI
  could not run in the worktree (`link-submodules.nu` needs a lock file the worktree lacks);
  GitHub CI is the proof.

- 2026-11-07: feature 06 (netio load tracker, #214) is in review as PR #337. The rate now
  comes from connections since `last_calculation` over the injected clock's elapsed time, in
  `u128` fixed-point to avoid the f64 cast lints. `LoadTracker::connection_rate() -> u64` keeps
  the fraction in `connection_rate_milli`. `calculation_interval` was dead configuration and
  is now the minimum window, default zero. The constructors take `Arc<dyn Clock>` and the
  `Default` impl is gone, so `foundation_netio` is 0.0.8 and 67 consumer manifests, `Cargo.lock`
  and the example lock are updated. No caller outside the crate uses the changed API. Two
  `SimulatedClock` tests: the rate test failed on the old code with 0 and passes now; host run
  1226 passed, Linux CI image 1227 and 1302 (`quic`) passed. The same PR fixes a soundness
  bug: `ConnectionGuard` held a raw tracker pointer with `unsafe Send/Sync` and now borrows
  it. Filed #336 (medium: 121/122 `foundation_reactor` clippy warnings block a dependent
  crate's strict `-D warnings`).

- 2026-11-07: feature 01 (HTTP app clock, #234) is in review as PR #333. `HttpApp` gains
  `with_ctx`, `with_context` and `with_clock`; every plain constructor routes through
  `with_ctx(Ctx::host())`, and only those host entry points call `Ctx::host()`. The wasm
  bridges gain ctx-taking initializers and fork the app bag per request. The real constructors
  are `ContextBag::from_ctx(&Ctx)` and `Ctx::new(Arc<dyn Clock>)`. `foundation_http` is 0.0.8
  and every consumer requirement is updated. Four tests in `tests/http_app_ctx_tests.rs`;
  default run 73 passed, `multi quic` run 154 passed. Clippy needs `--no-deps` because of the
  `foundation_boringtun` dev-dependency lints (#330). Filed #334 (medium: the web wasm
  singleton drops its context bag) and #335 (low: unused `getrandom` optional dependency).

- 2026-11-07: feature 03 (wireguard hole punch, #223) is in review as PR #332. The panic is
  gone; a failed punch with no relay returns `WgError::NoRelayForPunchedSession` and leaves the
  session `Disconnected`. `foundation_wireguard` is 0.0.6 and
  `foundation_deployment_platform` requires it. The regression test panicked on the old code and
  passes now; `relay_tests` is 27/27, the crate suite 79 passed. The strict
  `--all-targets -D warnings` clippy stops on 7 pre-existing lints in `foundation_boringtun`
  (filed as #330, low); the crate's own lib and test targets are clean. #331 (unused test
  imports on non-Linux) filed, low.

- 2026-11-07: the retired backlog is gone. The remaining unchecked checklists in umbrella issues
  #99, #100, #101, #102, #103, #106, #107 and #110 became 76 individual issues, #253–#288 and
  #290–#329, each with one `severity:` label (12 high, 39 medium, 25 low). One item duplicated
  issue #222 and was not re-filed. `docs/bug-backlog.md` is deleted. The 12 high issues are
  outside this spec's medium scope; they are reported to the owner and not fixed here.
- 2026-11-07: spec written. The input is the 17 open `severity: medium` issues that spec 70
  left. No feature started yet (0/12).

## Feature status

- **01-http-app-clock** — in review, PR #333 (#234)
- **02-core-background-shutdown-deadline** — in review, PR #339 (#233)
- **03-wireguard-hole-punch** — in review, PR #332 (#223)
- **04-docker-container-clock** — in review, PR #345 (#220)
- **05-wasm-wasi-time-source** — in review, PR #338 (#219)
- **06-netio-load-tracker-rate** — in review, PR #337 (#214)
- **07-nativeapi-ptrace-dispatch** — in review, PR #344 (#205–#208)
- **08-nativeapi-setregs-and-truncate** — not started (#209, #210)
- **09-toolings-clock-and-wake** — in review, PR #349 (#201, #203)
- **10-keychain-macos-flake** — in review, PR #359 (#197)
- **11-ci-mobile-target-builds** — not started (#196)
- **12-ci-free-runner-disk** — not started (#194)

## Next steps

1. Batches of at most three agents. Done: 01, 03, 05, 06. Running: 02, 04. Next: 07, 09, 10,
   then 08, 11, 12.
2. Close each issue in the pull request that fixes it.

_Created: 2026-11-07_
