---
feature: "CI compiles the iOS and Android wireguard targets and the iOS example"
status: "pending"
priority: "medium"
depends_on: []
crates: ["foundation_wireguard"]
issues: [196]
model: "haiku"
branch: "spec72/f11-ci-mobile-target-builds"
---

# Feature 11: CI mobile target builds

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** continuous integration (CI); operating system (OS); integrated
development environment (IDE, as in `src-tauri`); AeroSpace and Defence Industries Association
of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Compile the declared mobile targets in CI, so a break is visible in the pull request. Close
issue [#196](https://github.com/ewe-studios/ewe_platform/issues/196).

## 2. Problem

Two mobile builds are declared and never compiled.

- `foundation_wireguard`: `src/lib.rs:27-34` declares `pub mod android` (`target_os =
  "android"` or `spec55-complete`) and `pub mod ios` (`target_os = "ios"` or
  `spec55-complete`). Both `src/ios/mod.rs` and `src/android/mod.rs` exist. No CI job builds
  for `aarch64-apple-ios` or `aarch64-linux-android`.
- `examples/platform_ios`: the `macos-ssh` job in `.github/workflows/platform-vms.yaml`
  (around lines 93-108) runs only `ci.py run foundation_jsonschema` and
  `cargo build -p infrastructure_llama_cpp --examples`. Nothing adds the `aarch64-apple-ios`
  target or checks `examples/platform_ios/src-tauri`.

## 3. Requirements

1. Add `rustup target add aarch64-apple-ios` to the macOS job.
2. Run `cargo check -p foundation_wireguard --target aarch64-apple-ios` and the Android
   equivalent with `--features spec55-complete`.
3. Check `examples/platform_ios/src-tauri` for `aarch64-apple-ios` after `cargo tauri ios
   init` if the project needs it.
4. The new steps fail the job on a compile error. Do not add `|| true`.
5. The jobs stay inside a time bound (`timeout-minutes`).

## 4. Design

Edit `.github/workflows/platform-vms.yaml`. Keep the existing job structure. Add the target
install and the checks in the `macos-ssh` run block. If the Android target needs a
cross-toolchain not present on the runner, add it in the same step and record the choice.

## 5. Tasks

- [ ] Read the workflow and the two `mod` declarations.
- [ ] Add the iOS target and the two checks.
- [ ] Add the Android check.
- [ ] Run the workflow in a pull request and read the result.
- [ ] Close #196 in the PR body.

## 6. Tests

The CI job is the test. Report the job name and its result. A local `cargo check` for the iOS
target is a bonus, not a substitute.

## 7. Verify

```bash
# Local shape, when the target is installed:
rustup target add aarch64-apple-ios
cargo check -p foundation_wireguard --target aarch64-apple-ios
# The real proof is the pull request's workflow run.
```

## 8. Done when

- The macOS job builds the iOS declared module and the iOS example.
- The Android declared module has a check.
- The pull request run shows both.
- Issue #196 is closed by the PR.
- `progress.md` records the job names and their results.
