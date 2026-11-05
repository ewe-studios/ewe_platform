---
feature: "`foundation_nativeapis` clippy debt reaches zero"
status: "pending"
lane: "after 112"
model: "haiku"
branch: "spec70/f113-nativeapis-clippy-debt"
crates: ["foundation_nativeapis"]
catalogue_rows: []  # from docs/bug-backlog.md §35; not a §8 catalogue row
---

# Feature 113: `foundation_nativeapis` clippy debt

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (measured 2026-11-05, PR #179 CI fix pass)

`cargo clippy --profile uat -p foundation_nativeapis --all-targets --no-deps -- -D warnings`
reports 191 warnings. The same command with `--features vfs-preload` reports 465. The
`vfs-preload` code adds most of the difference. The main kinds are
`clippy::non_std_lazy_statics` (the `once_cell::sync::Lazy` statics in
`src/native/vfs/shim/mod.rs`), `clippy::doc_markdown`, and raw-pointer lints
(`borrow_as_ptr`, `ptr_as_ptr`) in the `epoll` and VFS code.

This package does not set `clippy-deny-warnings = true` in `ci/packages.toml`, so the output
does not fail a run. The project wants zero warnings, so the debt is recorded in
`docs/bug-backlog.md` §35 and scheduled here.

## Design

1. Run the command above once with `--features vfs-preload`. Work file by file, and fix the
   cause of each warning. Do not add a blanket `#[allow]` to the crate root.
2. `non_std_lazy_statics`: replace `once_cell::sync::Lazy` with
   `std::sync::LazyLock` where the code already needs `std`. If a `once_cell` dependency
   stays for another reason, keep it and record why.
3. `doc_markdown`: add backticks to identifiers in doc comments.
4. Raw-pointer lints: use `&raw const` and `&raw mut`, or `ptr.cast::<T>()`, as clippy asks.
5. When the command above exits 0, add `clippy-deny-warnings = true` to the
   `[packages.foundation_nativeapis]` entry and add `vfs-preload` to `clippy-runs`, so the
   debt cannot return.
6. Delete the §35 clippy entry in `docs/bug-backlog.md` in the same change.

## Tests

- `cargo clippy --profile uat -p foundation_nativeapis --all-targets --no-deps -- -D warnings`
  exits 0.
- `cargo clippy --profile uat -p foundation_nativeapis --all-targets --features vfs-preload
  --no-deps -- -D warnings` exits 0.
- The package tests still pass, including the `vfs-preload` run that builds the shim and runs
  `tests/vfs_shim_vfs`.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_nativeapis --all-targets --no-deps -- -D warnings
cargo clippy --profile uat -p foundation_nativeapis --all-targets --features vfs-preload --no-deps -- -D warnings
cargo test --profile uat -p foundation_nativeapis --no-fail-fast
cargo test --profile uat -p foundation_nativeapis --no-fail-fast --features vfs-preload
```
