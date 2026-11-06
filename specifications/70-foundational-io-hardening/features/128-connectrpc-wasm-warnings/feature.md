---
feature: "The wasm32 rpc build of foundation_connectrpc is warning-free and denied in CI"
status: "done"
lane: "priority follow-up (found by feature 123)"
model: "haiku"
branch: "spec70/f128-connectrpc-wasm-warnings"
crates: ["foundation_connectrpc"]
catalogue_rows: ["1"]
---

# Feature 128: the wasm32 rpc build of `foundation_connectrpc` is warning-free

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **gate**: the `cargo check` command for one target and feature set that the audit runs.
- **dead code**: a function or import that no compiled build reaches.

## Problem (found by feature 123 on 2026-11-06)

The native build of `foundation_connectrpc` is warning-free. The wasm32 gate is not:

```bash
cargo check --target wasm32-unknown-unknown -p foundation_connectrpc \
  --no-default-features --features rpc
```

reports three warnings in `src/shared/router/dispatch.rs`:

- the unused imports `PipeReceiver` and `PipeSender` at line 46;
- the unused function `run_unary` at line 625;
- the unused function `grpc_status_trailers` at line 1893.

The wasm gate in `ci/packages.toml` does not pass `-D warnings`, so CI passes and the debt
stays. `docs/bug-backlog.md` §1 holds the entry.

## Design

1. Find the real users of each item.
   - `grep -rn 'PipeReceiver\|PipeSender' backends/foundation_connectrpc/src/shared/router/`
     shows which functions take them.
   - `run_unary` has a deadline twin, `run_unary_until_deadline` (line 575). If the wasm
     path calls only that one, `run_unary` is native-only.
   - `grpc_status_trailers` at line 1893: find its callers the same way.
2. Gate each item with the smallest correct condition, next to the existing
   `#[cfg(not(target_family = "wasm"))]` import at line 44 that already follows this
   pattern. Use `target_family = "wasm"` or the feature that reaches the item, whichever
   is exact. Never delete a function that a build uses.
3. If an item is truly unreachable in every build, delete it and say so in the Result.
4. Make the gate deny warnings so the debt cannot return: in `ci/packages.toml`, the
   `foundation_connectrpc` entry's wasm check runs with `-D warnings` (the same flag the
   native clippy entry uses). If the wasm check runs through a per-package list, add the
   flag to the wasm list only, so the native build keeps its current settings.

## Tests

No behaviour change. The proof is the gate itself:

```bash
cargo check --target wasm32-unknown-unknown -p foundation_connectrpc \
  --no-default-features --features rpc
```

must print no warning. Also check the second wasm feature set if the crate has one
(`--features rpc_multi` where the gate runs it).

## Verify

```bash
cargo fmt --all --check
cargo check --target wasm32-unknown-unknown -p foundation_connectrpc --no-default-features --features rpc
cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_connectrpc
```

## Done when

- [x] The wasm32 rpc check prints no warning.
- [x] The gate fails on a warning, so the debt cannot return.
- [x] The native build is unchanged and its tests pass.
- [x] The backlog §1 entry is deleted.
- [x] CI green. GitHub Actions run `42205799516` (head `bc7e870d5`) passed on 2026-11-06.

## Result

2026-11-06. Done, on branch `spec70/priority-followups`. The wasm32 rpc gate
prints no warning and now denies warnings.

- Each item is gated with `#[cfg(not(target_family = "wasm"))]`, next to the
  existing native-only import at line 44. `PipeReceiver` and `PipeSender` are
  used only by the h2 functions `dispatch_h2`, `dispatch_h2_stream` and
  `run_streaming_h2`, which all sit in the native-only H2 section. `run_unary`
  is called from `dispatch_h2_stream` and from `native/h3_serve.rs`, and `native`
  is compiled only off wasm. `grpc_status_trailers` is called from
  `run_streaming_h2` and from `native/h3_serve.rs`. No function is deleted: each
  one has a native caller.
- No second wasm feature set: `ci/packages.toml` gives
  `foundation_connectrpc` one entry, `--no-default-features --features rpc`.
- The deny route is `RUSTFLAGS=-D warnings`, because this cargo
  (`1.100.0-nightly`) rejects `cargo check ... -- -D warnings`:
  `error: unexpected argument '-D' found`. `ci.py` now has a
  `wasm-deny-warnings` key, a `wasm_deny_warnings` field on `Settings`, and a
  `PlannedCommand` (argv plus per-command environment) that the runner applies
  after the package environment. The planned gate command is:
  `RUSTFLAGS=-D warnings cargo check --target wasm32-unknown-unknown -p foundation_connectrpc --no-default-features --features rpc`.
- Proof the gate fails: a temporary `use std::time::Duration;` in
  `dispatch.rs` made it exit 101 with
  `error: unused import: `std::time::Duration`` and
  `note: `-D unused-imports` implied by `-D warnings``. The line is reverted;
  the gate then exits 0.
- A new test, `scripts/ci/tests/test_ci_wasm_deny.py`, reads `ci/packages.toml`,
  builds the plan for `foundation_connectrpc`, and checks that every wasm step
  carries `RUSTFLAGS=-D warnings`. Two more tests check that a package without
  the key keeps plain commands and that `[defaults]` does not set it.
- Native build unchanged: clippy clean, and 241 tests pass. The
  `docs/bug-backlog.md` §1 entry is deleted. Version stays 0.0.5: no public API
  change.
