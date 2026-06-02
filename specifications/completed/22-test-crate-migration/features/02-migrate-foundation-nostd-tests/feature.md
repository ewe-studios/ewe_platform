---
name: "Migrate foundation_nostd Tests"
description: "Move foundation_nostd tests from tests/backends/foundation_nostd/ to backends/foundation_nostd/tests/"
status: "completed"
priority: "medium"
dependencies: ["01-migrate-foundation-core-tests"]
estimated_effort: "small"
---

# Feature: Migrate foundation_nostd Tests

## Overview

Move foundation_nostd-related tests from `tests/backends/foundation_nostd/` into `backends/foundation_nostd/tests/`.

## Source Files to Migrate

| File | Primary Target | Uses foundation_testing? |
|------|---------------|------------------------|
| `benchmarks/mod.rs` | N/A | Module declarations |
| `benchmarks/barrier_debug.rs` | `foundation_testing` scenarios | Yes (`Barrier`) |
| `benchmarks/integration_tests.rs` | `foundation_testing` scenarios | Yes (`Barrier`, `ProducerConsumerQueue`, `ThreadPool`) |
| `benchmarks/wasm_tests.rs` | `foundation_nostd` | No |
| `integrations/mod.rs` | Empty | N/A |

## Analysis

Most of these tests are actually testing `foundation_testing` primitives (scenarios module), not `foundation_nostd`.

### Decision Needed:
These tests belong in `foundation_testing/tests/` since they test `foundation_testing::scenarios::*`.

## Destination Structure

```
backends/foundation_testing/tests/scenarios/
├── mod.rs
├── barrier_debug.rs
└── integration_tests.rs

backends/foundation_nostd/tests/
└── wasm_tests.rs (if it actually tests nostd primitives)
```

## Alternative Analysis

If `wasm_tests.rs` tests `foundation_nostd` primitives:
```
backends/foundation_nostd/tests/
└── wasm_tests.rs
```

## Verification Steps

1. Identify actual crate being tested in each file
2. Move to appropriate crate
3. Run `cargo test -p foundation_nostd` and `cargo test -p foundation_testing`

## Notes

- `barrier_debug.rs` and `integration_tests.rs` test `foundation_testing::scenarios::*`
- These may need to go to `foundation_testing/tests/` instead
