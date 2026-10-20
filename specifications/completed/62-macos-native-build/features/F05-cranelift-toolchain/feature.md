---
workspace_name: "ewe_platform"
spec_directory: "specifications/62-macos-native-build"
feature_directory: "specifications/62-macos-native-build/features/F05-cranelift-toolchain"
this_file: "specifications/62-macos-native-build/features/F05-cranelift-toolchain/feature.md"

feature: "Cranelift toolchain"
description: "Install rustc-codegen-cranelift on all platforms and reconcile the duplicated dev profile so the default cargo build works"
status: "complete"
priority: "high"
depends_on: []
estimated_effort: "small"
created: 2026-10-17
last_updated: 2026-10-18
author: "Main Agent"
tasks:
  completed: 4
  uncompleted: 0
  total: 4
  completion_percentage: 100%
---

# F05 — Cranelift toolchain

[spec](../../requirements.md)

## Overview

The default `dev` profile uses the cranelift codegen backend, but the pinned
toolchain does not install that component. `cargo build` (dev profile) therefore
fails with:

```
error: failed to find a `codegen-backends` folder in the sysroot candidates:
  * /Users/.../.rustup/toolchains/nightly-2026-09-20-aarch64-apple-darwin
```

The `uat` profile (`codegen-backend = "llvm"`) works, so this does not block a
`--profile uat` build — it blocks the plain `cargo build` a fresh machine
expects. This feature installs the component everywhere and reconciles the
profile duplication.

## Root cause

- `rust-toolchain.toml` pins `nightly-2026-09-20` with
  `components = ["rustfmt", "rustc-dev", "rust-src"]` — no
  `rustc-codegen-cranelift`.
- `[profile.dev] codegen-backend = "cranelift"` appears in **two** places:
  root `Cargo.toml:102` and `.cargo/config.toml:44-45`.
- `.cargo/config.toml` also sets `[unstable] codegen-backend = true`, which is
  required for a non-default backend on nightly.

## Plan

1. **Determine the exact component name** for this nightly:
   `rustup component list --toolchain nightly-2026-09-20 | grep -i cranelift`.
   The name has historically been `rustc-codegen-cranelift-preview` and more
   recently `rustc-codegen-cranelift`; use whichever the toolchain reports, and
   pin that spelling everywhere.
2. **Add the component to `rust-toolchain.toml`** under `components`, so
   `rustup` installs it automatically when the toolchain is fetched.
3. **Add the same component to `scripts/setup/common.sh`**
   (`install_rust_components`), so `mise run setup` installs it idempotently on
   macOS and Arch.
4. **Reconcile the duplicated `[profile.dev] codegen-backend`.** Keep one source
   of truth (root `Cargo.toml`, the workspace manifest) and remove the
   `[profile.dev]` block from `.cargo/config.toml`, leaving only
   `[unstable] codegen-backend = true` there.

## Acceptance criteria

1. `rustc --version --verbose` (or `rustup component list`) shows the cranelift
   component installed for `nightly-2026-09-20` on both macOS and Arch.
2. `cargo build -p foundation_nativeapis` (default `dev` profile) succeeds on
   macOS — no `codegen-backends folder` error.
3. `[profile.dev]` is defined in exactly one place.
4. `mise run setup` is idempotent (re-running does not error on the component).

## Note

If cranelift does not support `aarch64-apple-darwin` on this nightly (cranelift
has historically lagged on some targets), the fallback is to point the default
`dev` profile at llvm (`codegen-backend = "llvm"`) and document `uat`/`dev` as
equivalent. Resolve this with a real build, not by assumption — record the
outcome here.

## Resolution

`rustc-codegen-cranelift` is installed for `nightly-2026-09-20` on both macOS
and Arch (added to `rust-toolchain.toml` `components` and to
`install_rust_components` in `scripts/setup/common.sh`). The duplicated
`[profile.dev]` block was removed from `.cargo/config.toml`; root `Cargo.toml`
is now the single source of truth for the profile.

Cranelift **does** support `aarch64-apple-darwin` on this nightly — the
workspace crates compile under it. But a plain all-cranelift `dev` build of
`foundation_ai` fails inside the `gemm-f16`/`gemm-common` candle dependencies,
which emit aarch64 half-precision SIMD inline asm
(`fadd v0.8h`, `fmla v0.8h`, `fmul v0.8h`) behind
`#[target_feature(enable = "fp16,neon")]`. Cranelift's aarch64 inline-asm
assembler does not assemble `fullfp16` instructions, even with
`-C target-feature=+fp16,+fhm` or `-C target-cpu=native` (verified via
`cargo build -v` that the flag reaches rustc — a genuine cranelift limitation,
not a config miss).

Resolution: keep `dev` pure cranelift — it is a local fast-iteration
convenience, not a "must build everything" bar. Crates whose dependencies
contain the fullfp16 asm (`foundation_ai` et al.) are built with
`--profile uat` (llvm), the real build path. No per-package backend forcing:
profiles are selected, never overridden. See F09 and LEARNINGS.md.
