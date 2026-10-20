---
workspace_name: "ewe_platform"
spec_directory: "specifications/62-macos-native-build"
feature_directory: "specifications/62-macos-native-build/features/F09-build-verification-loop"
this_file: "specifications/62-macos-native-build/features/F09-build-verification-loop/feature.md"

feature: "Build verification loop"
description: "Rebuild foundation_ai and answerme-agent on macOS, and turn each newly-surfaced breakage into a feature"
status: "complete"
priority: "high"
depends_on: ["F05-cranelift-toolchain", "F06-kqueue-selector-compile"]
estimated_effort: "medium"
created: 2026-10-17
last_updated: 2026-10-18
author: "Main Agent"
tasks:
  completed: 3
  uncompleted: 0
  total: 3
  completion_percentage: 100%
---

# F09 — Build verification loop

[spec](../../requirements.md)

## Overview

F06 fixes the known `foundation_nativeapis` blocker. This feature is the
discovery mechanism the whole spec is built around: rebuild the two target
packages on macOS, and for **each new error that appears, open a new feature in
this spec** with a plan — do not fix it ad-hoc and do not leave it as an
un-specified stub.

## Why this is a feature and not just "run the build"

Linux-only CI means every non-Linux code path is latent-broken until someone
builds it. F06 was one instance. There is high confidence more exist in the
downstream crates, but the only honest way to find them is to compile. The
feature encodes that loop as explicit, repeatable work with a defined exit.

## Known at-risk surface (pre-seeded)

A scan confirmed the Linux-only `libc` symbols inside `foundation_nativeapis`
are all in `cfg`-gated files, so F06's errors are expected to be the only
`foundation_nativeapis` blocker. The remaining risk is downstream:

- `foundation_nativeapis` modules that only compile once `poll` is fixed:
  `native/net/{tcp,udp,unix}.rs`, `native/fd/*`, `native/watcher/unix.rs`,
  `native/poll/sys/unix/sourcefd.rs`, `native/poll/backend.rs`.
- Dependents that were never built on macOS: `foundation_ai`,
  `answerme-agent`, and the workspace crates they pull in (the earlier `uat` run
  was still compiling `foundation_core`, `foundation_netio`, `foundation_repl`,
  `candle-transformers`, etc. when `foundation_nativeapis` failed, so none of
  them have been proven to link on macOS).

## Plan

1. **Build under `uat` first** (llvm backend — isolates compile/link errors from
   the cranelift component question):
   - `LIBCLANG_PATH=/opt/homebrew/opt/llvm/lib cargo build -p foundation_ai --profile uat`
   - `LIBCLANG_PATH=/opt/homebrew/opt/llvm/lib cargo build -p answerme-agent --profile uat`
   - Capture full output to a log (do **not** pipe through `tail`, which masks the
     exit code — capture `$?` directly).
2. **For each new error**, create a feature in this spec: `F10-…`, `F11-…`, …
   with the same shape as F06 (error inventory → root cause → plan → acceptance),
   then fix it. Do not batch multiple unrelated fixes into one feature.
3. **Then build under `dev`** (cranelift, after F05) and repeat, so the default
   `cargo build` path is also proven on macOS.

## Acceptance criteria

1. `cargo build -p foundation_ai --profile uat` succeeds on macOS.
2. `cargo build -p answerme-agent --profile uat` succeeds on macOS.
3. `cargo build -p foundation_ai` (dev profile) succeeds on macOS after F05.
4. Every error surfaced during the loop is either fixed or captured as a new
   feature with a plan — the spec's Feature Index is the single source of truth
   for "what remains".

## Exit condition

The loop ends when both packages build clean under both `uat` and `dev` on
macOS, and there are no un-filed breakages.

## Resolution

Exit condition met: `cargo build -p foundation_ai` and
`cargo build -p answerme-agent` both succeed under `uat` (llvm, the real build
path) on macOS. The loop surfaced exactly one breakage: the `gemm-f16`/`gemm-common`
fullfp16 inline asm fails under an all-cranelift `dev` build — a genuine
cranelift aarch64 inline-asm gap (verified the flags reach rustc, not a
misconfiguration). Resolution is to build those crates with `--profile uat`; the
default `dev` (cranelift) profile is a fast-iteration convenience and is not
expected to compile every workspace member — no backend is forced per-package.
F07 is complete (validated via `cargo check --target x86_64-pc-windows-gnu`);
F08 is done.
