---
feature: "Tier 1 Leaf Crates Publishing"
description: "Publish 8 leaf crates with no internal workspace dependencies to crates.io"
status: "pending"
priority: "high"
depends_on: ["01-metadata-completion"]
estimated_effort: "medium"
created: 2026-06-16
last_updated: 2026-06-16
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# Feature 02: Tier 1 Leaf Crates Publishing

## WHY: Problem Statement

These 8 crates have no internal workspace dependencies — they are the foundation layer that all other crates depend on. They must be published first so that Tier 2+ crates can resolve their dependencies from crates.io.

## WHAT: Crates to Publish

| # | Crate | Version | Description | Notes |
|---|-------|---------|-------------|-------|
| 1 | `foundation_errstacks` | 0.0.1 | Minimal derive_more-friendly context-aware error traces | Leaf — no internal deps |
| 2 | `foundation_nostd` | 0.0.4 | Foundational nostd core implementation | Leaf — no internal deps |
| 3 | `foundation_macros` | 0.0.4 | Proc macros for the foundation crates | Leaf — no internal deps |
| 4 | `foundation_conditional` | 0.1.0 | Block-level conditional compilation macros | Leaf — no internal deps |
| 5 | `foundation_testing` | 0.0.1 | Reusable stress testing infrastructure | Leaf — no internal deps |
| 6 | `foundation_rng` | 0.1.0 | RNG + scru128 IDs with WASM support | Leaf — no internal deps |
| 7 | `infrastructure_llama_bindings` | 0.0.1 | Low-level bindings to llama.cpp | Leaf — FFI bindings |
| 8 | `infrastructure_llama_cpp` | 0.0.1 | llama.cpp bindings for Rust | Leaf — FFI bindings |

## HOW: Per-Crate Publish Steps

For each crate in the order above:

1. **Verify metadata** — description, readme, license, repository present
2. **Run tests** — `cargo test -p <crate>` — all must pass
3. **Dry-run publish** — `cargo publish --dry-run -p <crate>` — no errors
4. **Publish** — `cargo publish -p <crate>`
5. **Verify on crates.io** — check URL https://crates.io/crates/<crate> shows correct version

### Verification

```bash
# Test all Tier 1 crates
for crate in foundation_errstacks foundation_nostd foundation_macros foundation_conditional foundation_testing foundation_rng; do
  echo "Testing $crate..."
  cargo test -p "$crate" 2>&1 | tail -3
done

# Dry-run publish all Tier 1
for crate in foundation_errstacks foundation_nostd foundation_macros foundation_conditional foundation_testing foundation_rng infrastructure_llama_bindings infrastructure_llama_cpp; do
  echo "Dry-run: $crate"
  cargo publish --dry-run -p "$crate" 2>&1 | tail -5
done
```

### Success Criteria

- All 8 crates pass `cargo test`
- All 8 crates pass `cargo publish --dry-run`
- All 8 crates visible on crates.io with correct version and description
