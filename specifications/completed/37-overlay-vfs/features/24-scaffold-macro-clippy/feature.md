---
feature_name: "scaffold!() Clippy Warning Suppression"
description: "Investigate how todo!() suppresses clippy warnings (via the never type `!`) and apply the same technique to scaffold!() so that delegated trait methods don't trigger unused-variable or similar warnings."
status: "done"
priority: "low"
phase: 5
created: 2026-06-07
updated: 2026-06-07
dependencies:
  - "18-scaffold-macro"
tasks:
  completed: 4
  uncompleted: 0
  total: 4
  completion_percentage: 100%
---

## Context

`todo!()` expands to `panic!()` which has return type `!` (never). The never type unifies with any type, so rustc/clippy knows the function diverges and suppresses warnings about unused bindings, missing return values, etc.

`scaffold!()` expands to real delegation code that calls `self.<field>.<method>(args)`. When the expanded code has bindings that clippy considers unused (e.g. method parameters that are forwarded), clippy emits warnings.

## Tasks

- [x] Audit current clippy warnings produced by scaffold!() expansions across the codebase
- [x] Study how todo!() and unimplemented!() suppress warnings (never type vs #[allow] attributes)
- [x] Modify scaffold!() macro to emit `#[allow(unused)]` or equivalent on generated bindings — not needed: delegation code is already clean, no warnings at expansion sites
- [x] Verify zero clippy warnings on all scaffold!() call sites after the fix — confirmed: `cargo clippy -p foundation_nativeapis --features vfs-fuse` produces zero scaffold-related warnings. Also fixed 30 general clippy warnings in foundation_macros + foundation_nostd (commit 9e917fb8).
