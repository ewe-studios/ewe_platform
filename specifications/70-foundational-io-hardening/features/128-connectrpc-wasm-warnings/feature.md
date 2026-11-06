---
feature: "The wasm32 rpc build of foundation_connectrpc is warning-free and denied in CI"
status: "pending"
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

- [ ] The wasm32 rpc check prints no warning.
- [ ] The gate fails on a warning, so the debt cannot return.
- [ ] The native build is unchanged and its tests pass.
- [ ] The backlog §1 entry is deleted.
- [ ] CI green.

## Result

2026-11-06. Plan only. Not started. Feature 123 saw the three warnings while it ran the
wasm check for the client deadline.
