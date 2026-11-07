# Spec 72 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

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

- **01-http-app-clock** — not started (#234)
- **02-core-background-shutdown-deadline** — not started (#233)
- **03-wireguard-hole-punch** — not started (#223)
- **04-docker-container-clock** — not started (#220)
- **05-wasm-wasi-time-source** — not started (#219)
- **06-netio-load-tracker-rate** — not started (#214)
- **07-nativeapi-ptrace-dispatch** — not started (#205, #206, #207, #208)
- **08-nativeapi-setregs-and-truncate** — not started (#209, #210)
- **09-toolings-clock-and-wake** — not started (#201, #203)
- **10-keychain-macos-flake** — not started (#197)
- **11-ci-mobile-target-builds** — not started (#196)
- **12-ci-free-runner-disk** — not started (#194)

## Next steps

1. Batches of at most three agents: 01, 03, 06 first, then 02, 04, 05, then 07, 09, 10, then
   08, 11, 12.
2. Close each issue in the pull request that fixes it.

_Created: 2026-11-07_
