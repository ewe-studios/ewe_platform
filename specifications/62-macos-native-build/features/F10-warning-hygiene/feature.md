---
workspace_name: "ewe_platform"
spec_directory: "specifications/62-macos-native-build"
feature_directory: "specifications/62-macos-native-build/features/F10-warning-hygiene"
this_file: "specifications/62-macos-native-build/features/F10-warning-hygiene/feature.md"

feature: "Warning hygiene: lint inheritance, optional vfs deps, macOS signal delivery"
description: "Fix the three remaining build warnings at their source (not by suppression): workspace-wide missing_lints_inheritance, unused async-trait/foundation_errstacks dependencies, and the macOS kqueue signal handler never delivering to the bus"
status: "complete"
priority: "low"
depends_on: ["F07-selector-new-parity"]
estimated_effort: "small"
created: 2026-10-18
last_updated: 2026-10-18
author: "Main Agent"
tasks:
  completed: 3
  uncompleted: 0
  total: 3
  completion_percentage: 100%
---

# F10 — Warning hygiene

[spec](../../requirements.md)

## Overview

After F07 the macOS/Windows checks pass with zero errors, but three warning
categories remain. Two are visible in every `cargo check`; the third is a real
correctness gap hiding behind a dead-code warning. Each is fixed at its source —
no `#![allow]` suppression.

1. **`missing_lints_inheritance` (workspace-wide).** The root `Cargo.toml`
   defines `[workspace.lints.rust]` / `[workspace.lints.clippy]`, but 13
   workspace members declared no `[lints]` table, so they never inherited the
   workspace lints and cargo warned per-member.
2. **`unused dependency async-trait / foundation_errstacks`
   (`foundation_nativeapis`).** Both are declared non-optional but only used by
   the `vfs` modules, which are off in the default feature set, so the
   `unused_dependencies` lint fires on every default check.
3. **macOS `deliver_signal`/`deliver` dead-code (`foundation_nativeapis`).**
   The kqueue signal path registers `EVFILT_SIGNAL` but, unlike the Linux/Windows
   handlers, never translated the captured event back into a signal kind or
   delivered it to the bus. The kqueue `is_ready` path simply dropped the event,
   leaving the delivery functions dead on macOS.

## Root cause

All three trace to the same root: Linux-only CI. A workspace lint policy existed
but was never enforced across the whole member set; the `vfs`-only deps were
added before the crate's feature split; and the macOS kqueue handler was written
to *wait* for signals (there is no user-space handler — the kernel consumes them
into the kqueue) without closing the loop back into `deliver_signal`, which the
Linux handler does from its async-signal context and the Windows console handler
does from its own thread.

## Plan / work done

1. **Lint inheritance.** Enumerate workspace members
   (`cargo metadata --no-deps`, `source == null`) and give each a `[lints]`
   table. 11 members get `[lints] workspace = true` (pure inherit).
   `foundation_compact` already had a meaningful `[lints.rust]` override — its
   `unexpected_cfgs` `check-cfg` list for the `getrandom_*` cfgs — so it keeps
   `[lints.rust]`/`[lints.clippy]` as a manual-lints member (it still declares
   lints, so it does not warn). `infrastructure_llama_bindings` had an empty
   `[lints.clippy]` leftover table; that was removed in favour of
   `[lints] workspace = true`. A re-enumeration confirms no member is left
   without a lints table.
2. **Optional `vfs` deps.** In `foundation_nativeapis/Cargo.toml`, mark
   `async-trait` and `foundation_errstacks` `optional = true` and wire them into
   the existing feature: `vfs = ["dep:async-trait", "dep:foundation_errstacks"]`.
   The `vfs` feature and every `vfs-*` feature that implies it continue to pull
   them in; the default feature set no longer declares an unused dependency.
3. **macOS signal delivery.** In `signal/handle.rs`, the macOS `SignalHandle::is_ready`
   branch now drains the kqueue and translates each `EVFILT_SIGNAL` kevent back
   into a `SignalKind` (`signal_kind_from_kevent`: `.ident` → `SIGINT`/`SIGTERM`/
   `SIGHUP`/`SIGQUIT`), then calls `crate::signal::deliver_signal(kind)`. This
   is the macOS counterpart to the Linux handler's eventfd write + `deliver_signal`
   and the Windows console handler's `SetEvent` + `deliver_signal`, closing the
   loop so the bus is notified on macOS too.

## Acceptance criteria

1. `cargo check -p foundation_nativeapis --profile uat` (macOS default) emits
   zero warnings. ✅
2. `cargo check -p foundation_nativeapis --features vfs --profile uat` emits
   zero warnings, and the `vfs` feature still compiles. ✅
3. `cargo check --target x86_64-pc-windows-gnu -p foundation_nativeapis
   --profile uat` emits zero warnings. ✅
4. No workspace member is left without a `[lints]` table — verified by
   enumerating `cargo metadata` members. ✅

## Out of scope

- The pre-existing `unexpected_cfgs`/`unused` warnings in other crates
  (`foundation_iogate`'s `uring` cfg, `foundation_testbed`'s unused variables),
  which are unrelated to the three warnings this feature targets.
- Any `#![allow]`-based suppression; every fix here removes the warning by
  fixing its cause.
