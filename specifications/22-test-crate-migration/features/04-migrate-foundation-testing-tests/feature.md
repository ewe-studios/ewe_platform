---
name: "Migrate foundation_testing Tests"
description: "Move foundation_testing self-tests from tests/backends/tests.rs to backends/foundation_testing/tests/"
status: "pending"
priority: "medium"
dependencies: ["01-migrate-foundation-core-tests"]
estimated_effort: "small"
---

# Feature: Migrate foundation_testing Tests

## Overview

Move `tests/backends/tests.rs` (which tests `foundation_testing` itself) to `backends/foundation_testing/tests/`.

## Source File Analysis

```rust
//! Integration tests for foundation_testing crate.

use crate::stress::{StressConfig, StressHarness, sync::run_condvar_stress_test};
use crate::scenarios::{ProducerConsumerQueue, Barrier, ThreadPool};
```

This file tests:
- `StressConfig`, `StressHarness` - from `foundation_testing::stress`
- `ProducerConsumerQueue`, `Barrier`, `ThreadPool` - from `foundation_testing::scenarios`
- `run_condvar_stress_test` - stress testing utilities

## Current Issue

The test file uses `crate::` imports, assuming it's testing from within the crate. But it's in `ewe_platform_tests` which has `foundation_testing` as a regular dependency.

## Destination

```
backends/foundation_testing/tests/
└── integration_tests.rs
```

## Required Changes

### 1. Move and Update Imports
Change from:
```rust
use crate::stress::{StressConfig, StressHarness, sync::run_condvar_stress_test};
use crate::scenarios::{ProducerConsumerQueue, Barrier, ThreadPool};
```

To:
```rust
use foundation_testing::stress::{StressConfig, StressHarness, sync::run_condvar_stress_test};
use foundation_testing::scenarios::{ProducerConsumerQueue, Barrier, ThreadPool};
```

### 2. Create backends/foundation_testing/tests/mod.rs
```rust
mod integration_tests;
```

### 3. Remove from tests/backends/
Remove `tests.rs` and update `backends/mod.rs`.

## Verification Steps

1. Run `cargo test -p foundation_testing`
2. Verify stress tests compile and pass
3. Verify scenario tests compile and pass

## Benefits

- Tests run as part of `foundation_testing` CI
- Self-testing for the testing infrastructure
- No external test crate needed for self-tests
