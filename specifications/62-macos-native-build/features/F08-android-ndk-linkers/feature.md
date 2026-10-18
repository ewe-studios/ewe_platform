---
workspace_name: "ewe_platform"
spec_directory: "specifications/62-macos-native-build"
feature_directory: "specifications/62-macos-native-build/features/F08-android-ndk-linkers"
this_file: "specifications/62-macos-native-build/features/F08-android-ndk-linkers/feature.md"

feature: "Android NDK linkers"
description: "Make the Android cross-compile linker paths OS-relative so they don't hard-code a Linux home dir"
status: "complete"
priority: "low"
depends_on: []
estimated_effort: "small"
created: 2026-10-17
last_updated: 2026-10-18
author: "Main Agent"
tasks:
  completed: 2
  uncompleted: 0
  total: 2
  completion_percentage: 100%
---

# F08 — Android NDK linkers

[spec](../../requirements.md)

## Overview

`.cargo/config.toml` hard-codes Android NDK linker paths to a Linux home dir:

```toml
[target.aarch64-linux-android]
linker = "/home/darkvoid/android-sdk/ndk/26.1.10909125/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android34-clang"
```

These paths do not exist on macOS, so Android cross-compilation (spec-55) breaks
on this machine. This does **not** block the macOS native build of
`foundation_ai`/`answerme-agent` (those are `aarch64-apple-darwin`), so it is
explicitly lower priority.

## Plan

1. **Replace the absolute paths** with a resolved path, e.g. honour
   `ANDROID_NDK_HOME`/`ANDROID_NDK_ROOT` (with a documented default per OS), or
   move the `[target.*android*]` blocks out of the shared config into a
   per-OS/opt-in include.
2. **Reconcile the NDK version**: the comment says NDK r27 at
   `~/Android/ndk/27.0.12077973/`, the linker paths say 26.1.10909125. Pick one
   and align the comment with the paths.

## Acceptance criteria

1. Android cross-compilation no longer references `/home/darkvoid/...`.
2. The NDK version in the comment matches the version in the paths (or the paths
   are derived from the installed NDK so the mismatch cannot recur).

## Resolution

Done. The Android linker paths are no longer hard-coded to a Linux home dir:

1. `scripts/android-ndk/{aarch64-linux-android,x86_64-linux-android,armv7-linux-androideabi}`
   thin wrappers call a shared `scripts/android-ndk/cc` resolver that locates the
   NDK at build time (`ANDROID_NDK_HOME`/`ANDROID_NDK_ROOT`, else the Android
   Studio default location) and auto-selects the per-OS host prebuilt
   (`darwin-x86_64` vs `linux-x86_64`) — so one `.cargo/config.toml` works on
   both macOS and Linux.
2. `scripts/setup/android-ndk.sh` installs the pinned NDK (r27,
   `27.0.12077973`, `.zip` on Linux / `.dmg` on macOS) plus the Rust std targets,
   wired as `mise run setup:android`.
3. `rust-toolchain.toml` now lists all three Android targets
   (`x86_64-linux-android` and `armv7-linux-androideabi` were missing).
4. Version reconciled: comment and paths now both say r27 (was r27 comment vs
   26.1.10909125 paths).

Verified the resolver end-to-end with a stub NDK (correct host prebuilt, correct
`armv7a` tool-name mapping, clear error when absent). The installer was
syntax-checked, not run (the download is ~660 MB+).

## Out of scope

- Publishing Android builds (APK/AAB) or adding CI that runs them. The NDK
  installer script is provided and was not executed in this feature (large
  download) — running `mise run setup:android` is the acceptance step.
