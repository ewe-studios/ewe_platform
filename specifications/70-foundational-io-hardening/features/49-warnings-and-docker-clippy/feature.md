---
feature: "Lane A warnings: reactor wasm dead code, testbed deno_v8 lint, foundation_deployment_docker clippy debt"
status: "done"
lane: "A"
model: "haiku"
branch: "spec70/f47-f50-lane-a-mechanical"
crates: ["foundation_reactor", "foundation_testbed", "foundation_deployment_docker", "foundation_connectrpc", "foundation_netio", "foundation_deployment", "foundation_keychain", "foundation_proxy", "foundation_wireguard", "foundation_iogate"]
catalogue_rows: [13, 14, 15, 31]
---

# Feature 49: warnings and clippy debt in lane A

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## E1. `Poll::rearm_readable` is dead on wasm (backlog §16) — MOVED to feature 78 E4 (owner priority 2026-11-04); skip it here

`foundation_reactor/src/poll/mod.rs` ~323 is `pub(crate)`, called only from a native path
(`reactor.rs` ~607). Gate the method with the same `cfg` as its call site.
Verify: `cargo check --tests --target wasm32-unknown-unknown -p foundation_netio 2>&1 | rg rearm_readable` is empty.

## E2. `deno_v8` unused-dependency lint in `foundation_testbed` (backlog §16)

It is listed only to pick `deno_core`'s V8 backend. Add `use deno_v8 as _;` in the
embedded JS module under the same feature, with a comment that says why. Verify with the
lint that reported it (cargo nightly `unused_dependencies`) if available, else
`cargo check -p foundation_testbed --features wasm-embedded-js` without warnings.

## E3. `foundation_deployment_docker` clippy debt (backlog §8, ~231 lints)

Fix all clippy warnings in this crate (default features and `--all-features`). Mechanical
fixes only; no behaviour change. Then add the crate's `--all-features` clippy run to
`ci/packages.toml` if the CI does not lint it, so it stays at zero. Remove the docker part of
the §8 entry (lane B removes the rest).

## E4. `foundation_connectrpc` clippy debt (backlog §1)

`cargo clippy -p foundation_connectrpc --profile uat --all-targets --all-features` reports
`doc_markdown` (most), cast lints in `tests/` and `src/shared/transport/conn.rs`,
`src/shared/client.rs`, and `needless_pass_by_value`, `must_use`, `struct_excessive_bools`,
`similar_names`, `unused_async` in examples. Fix all; mechanical only (a cast becomes
`try_from` with the error handled, or carries an `#[allow]` with a one-line reason where the
value is proven in range). Delete the §1 entry. Verify the same command prints 0 warnings.

## E5. `foundation_netio` and `foundation_deployment` warnings (seen in feature 42)

- `foundation_netio`: the unused `rustls-pemfile` dependency and the remaining lib clippy
  warnings (`cargo clippy -p foundation_netio --profile dev --all-targets`). Remove the
  dependency if nothing uses it (check every feature), fix the lints.
- `foundation_deployment`: "unused `self` argument" (clippy). Fix it.
Verify both crates' clippy print 0 warnings.

## E6. `foundation_keychain` clippy debt (found in feature 44)

174 lib warnings (for example `manual_let_else` in `provisioning_route`) plus test warnings.
Fix all; mechanical only. Delete the §22 clippy entry.

## E7. `foundation_proxy` clippy and test warnings (found in feature 45)

About 50 lib warnings, plus `load_balancer_tests` unused imports and
`h2_integration_tests.rs:318` unused `mut`. Fix all; mechanical only. Delete the §9 entry.

## E8. `foundation_wireguard` wasm manifest warnings (found in feature 54)

Fix the wasm32 manifest warnings (`cargo check -p foundation_wireguard --target
wasm32-unknown-unknown`) by target-gating dependencies. **Do not remove
`foundation_errstacks`:** it is the project's error standard; feature 67 adopts it in this
crate. Remove only `thiserror` (unused; `derive_more` is the standard for the error enums).

## E9. `foundation_iogate` clippy (found in feature 70)

About ten `doc_markdown` warnings. Fix them; 0 warnings.

## E10. `foundation_core` and `foundation_nostd` clippy debt (found in feature 46) — DONE in feature 116

Feature 116 finished this task. The library lints in both feature sets are at
zero, and the test targets and examples are at zero as well. `foundation_core`
now denies clippy warnings in `ci/packages.toml` with `clippy-runs = ["", "multi"]`,
and `foundation_nostd` keeps its deny setting. The bare `--no-default-features`
build prints only its named `compile_error!`. See feature 116 for the measured
before and after counts.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo clippy -p foundation_deployment_docker --profile dev --all-targets --all-features 2>&1 | rg -c '^warning'   # 0
python3 scripts/ci/ci.py run foundation_deployment_docker --profile dev
python3 scripts/ci/ci.py run foundation_reactor --profile dev
python3 scripts/ci/ci.py run foundation_testbed --profile dev
python3 scripts/ci/ci.py run foundation_connectrpc --profile dev
```

## Done when

- [x] The three warnings sources are at zero; backlog entries (or their docker part) deleted.
      (E1's wasm dead code was finished by feature 78 E4.)

## Result

2026-11-05. E1 is skipped (moved to feature 78). E2 to E9 are done. E10 is
partly done.

| Task | Crate | Result |
|---|---|---|
| E2 | `foundation_testbed` | `deno_v8` is marked used with `use deno_v8 as _;` and a comment. Clippy is clean with default features and with `wasm-embedded-js`. |
| E3 | `foundation_deployment_docker` | 0 warnings with default features and with `--all-features`. The crate now has a CI clippy run with `--all-features` and denys warnings. |
| E4 | `foundation_connectrpc` | 0 warnings with `--all-features`. |
| E5 | `foundation_netio`, `foundation_deployment` | 0 clippy warnings. Removed `rustls-pemfile` from `ssl-rustls`; the `compression` feature now enables its gzip and deflate users. |
| E6 | `foundation_keychain` | 0 warnings in the library and tests. |
| E7 | `foundation_proxy` | 0 warnings in the library and tests. |
| E8 | `foundation_wireguard` | `cargo check` is clean for wasm32 and native. Removed `thiserror` and `web-sys`; kept `foundation_errstacks` and target-gated it. |
| E9 | `foundation_iogate` | 0 warnings. |
| E10 | `foundation_core`, `foundation_nostd` | Done in feature 116: both crates are at zero clippy warnings in both feature sets and all targets. `foundation_core` denies clippy warnings in CI (`clippy-runs = ["", "multi"]`); `foundation_nostd` keeps its deny setting. The bare `--no-default-features` build prints only its named `compile_error!`. |

Documented `#[allow]`s are used only where the fix would change a public
signature, split a deliberate state machine, or need a crate version bump. Each
carries a one-line reason. The backlog records the follow-ups.

Test commands and results:

- `foundation_testbed` tests: green.
- `foundation_deployment_docker --features docker`: every test suite green.
- `foundation_connectrpc`: default and h3 test runs green.
- `foundation_netio`: test run green.
- `foundation_keychain`: test run green.
- `foundation_proxy`: test run green (the default CI set).
- `foundation_wireguard --features spec55-complete`: check passes.
- `foundation_nostd --features std`: one pre-existing timing test fails on the
  amd64 emulation (`test_wait_duration_zero_duration`); backlog §5.

The docker generated files were rewritten by `cargo clippy --fix`. The generator
must emit the same shape or a regeneration restores the warnings (backlog §8).
