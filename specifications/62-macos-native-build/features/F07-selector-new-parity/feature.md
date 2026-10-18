---
workspace_name: "ewe_platform"
spec_directory: "specifications/62-macos-native-build"
feature_directory: "specifications/62-macos-native-build/features/F07-selector-new-parity"
this_file: "specifications/62-macos-native-build/features/F07-selector-new-parity/feature.md"

feature: "Windows + shell selector cross-compile parity"
description: "Install mingw-w64 + the x86_64-pc-windows-gnu target, add Selector::new() to the Windows and shell selectors, gate the unix-only modules, and fix the Windows selector/signal HANDLE bugs so foundation_nativeapis compiles for Windows"
status: "complete"
priority: "medium"
depends_on: ["F06-kqueue-selector-compile"]
estimated_effort: "medium"
created: 2026-10-17
last_updated: 2026-10-18
author: "Main Agent"
tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
---

# F07 — Windows + shell selector cross-compile parity

[spec](../../requirements.md)

## Overview

`Poll::new()` calls `sys::Selector::new()` (`native/poll/mod.rs:126`). Only the
Linux dispatch/epoll selector implemented `new()`; kqueue (F06), Windows
(`sys/windows/mod.rs`) and shell (`sys/shell/mod.rs`) exposed only
`new_with_registry()`.

This feature closed that parity gap **and** validated the Windows selector for
the first time by installing a mingw-w64 cross-toolchain. The original
"deferred" scope (add `new()`, delegate `new_with_registry()`) turned out to be
the thin end of a much larger wedge: once `cargo check --target
x86_64-pc-windows-gnu` was possible, the Windows selector/signal code failed
with 32 errors (27 in the first pass, 5 remaining) across three distinct causes.

## Root cause

The `new()`/`new_with_registry()` split predates the current `Poll::new()`.
Linux was migrated to `new()`; windows/shell were left behind. That part was
never compiled because CI is Linux-only.

Compiling Windows surfaced the rest, latent-broken exactly as F06 predicted
(`LEARNINGS.md` #7):

1. **Unix-only modules were feature-gated, not `cfg(unix)`-gated.** `native::net`,
   `native::fd`, `valtron::native`, and `SourceFd` use `std::os::unix` /
   `std::os::fd` / `libc` unconditionally, so they fail on Windows regardless of
   features.
2. **The Windows selector/signal code treated `HANDLE` as an integer.** `HANDLE`
   is `*mut core::ffi::c_void` in windows-sys 0.59, so `== 0`, `!= 0`,
   `Some(...)`, and `event.0` are all wrong.
3. **`Event` couldn't be `Copy` with an `Option<std::io::Error>` field**, but
   `Events::iter()` uses `.copied()` (`events.rs:35`), so `Copy` is required.

## Plan / work done

1. **Installer** — `scripts/setup/windows-cross.sh` installs mingw-w64 (brew on
   Darwin, pacman on Arch) and `rustup target add x86_64-pc-windows-gnu`; wired
   into `mise run setup:windows`. `cargo check` needs only the std target;
   `cargo build` needs mingw. `.cargo/config.toml` gains a
   `[target.x86_64-pc-windows-gnu] linker = "x86_64-w64-mingw32-gcc"` entry and
   `rust-toolchain.toml` gains the target.
2. **`Selector::new()` parity** — added `new()` to the Windows (IOCP) and shell
   (stub) selectors; `new_with_registry()` delegates to `new()` (single source,
   mirroring F06).
3. **Unix gating** — `native::net`, `native::fd`, `valtron::native`, and
   `SourceFd` are now `#[cfg(unix)]`-gated in addition to feature-gated.
4. **HANDLE correctness** — replaced integer comparisons with pointer semantics
   (`is_null()`, `INVALID_HANDLE_VALUE`), moved `WAIT_OBJECT_0` to
   `Win32::Foundation`, imported `CreateIoCompletionPort` from `Win32::System::IO`,
   and enabled the `Win32_System_Console` feature for the `CTRL_*` constants.
5. **`Event` `Copy` + `SignalHandle` `Send`/`Sync`** — the Windows `Event` now
   stores `Option<std::io::ErrorKind>` (not `io::Error`, which is neither `Clone`
   nor `Copy`); `error()` reconstructs the `io::Error` on demand. `SignalHandle`
   gained `unsafe impl Send/Sync` (its `HANDLE` variant is a thread-safe kernel
   handle).
6. **Signal delivery fix** — the Windows console handler now stores its event
   handle in `GLOBAL_EVENT` *and* calls `deliver_signal(kind)` (mirroring the
   Linux handler), so both the parked waiter wakes and the bus is notified.

## Acceptance criteria

1. No selector type exposes only `new_with_registry()` — epoll, kqueue, windows,
   and shell all have `new()`. ✅ (shell is review-only; no shell-target
   toolchain exists to compile it)
2. `cargo check --target x86_64-pc-windows-gnu -p foundation_nativeapis
   --profile uat` passes with zero errors and zero `foundation_nativeapis`
   source warnings. ✅
3. `cargo check -p foundation_nativeapis --profile uat` (macOS) still passes. ✅

## Out of scope

- Actually building/linking a Windows binary (`cargo build` for
  `x86_64-pc-windows-gnu`), which needs mingw-w64 (installed by the new script
  but not exercised here). `cargo check` validates type/lifetime correctness.
- Running Windows binaries; the IOCP selector's runtime behavior is still
  unverified on real hardware.
- The macOS signal handler's missing bus delivery (see `LEARNINGS.md`).
