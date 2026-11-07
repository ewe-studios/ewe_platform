# Spec 72 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

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
