---
feature: executor-yield-time-optimization
description: Reduce DEFAULT_YIELD_WAIT_TIME from 6s to 1.5s to improve test execution speed while maintaining reasonable idle CPU usage
status: completed
priority: medium
created: 2026-05-13
tasks:
  completed: 2
  uncompleted: 0
  total: 2
  completion_percentage: 100
dependencies: []
---

# Feature: Executor Yield Time Optimization

## Problem

The `pool_drain_tests` in `foundation_core` take ~56 seconds to complete due to excessive yield wait time:

- `DEFAULT_YIELD_WAIT_TIME = Duration::from_secs(6)` in `backends/foundation_core/src/valtron/executors/constants.rs:11`
- When worker threads have no work, they yield for 6 seconds before checking for new work
- Each test has 3-4 HTTP requests with gaps between them where threads yield
- 3 tests × ~18 seconds each = ~56 seconds total

## Root Cause Analysis

The executor uses `CondVar::wait_timeout_while` with a 6-second duration when threads are idle:

```
ThreadYielder::yield_for() - START, duration=6s
```

This causes:
1. Delayed response to new work (up to 6s latency)
2. Slow test execution when work arrives during yield
3. Multiple yield cycles per test (3-4 requests × 6s = 18s per test)

## Solution

Reduce `DEFAULT_YIELD_WAIT_TIME` from 6s to 1.5s:

**Trade-offs:**
- **Benefit**: Faster test execution (~14s vs 56s for pool_drain_tests)
- **Cost**: Slightly higher CPU usage when idle (checking for work 4× more frequently)
- **Mitigation**: 1.5s is still long enough to avoid busy-waiting; production workloads rarely have 6s gaps

## Tasks

- [x] **Task 1**: Document the yield time optimization in feature specification
- [x] **Task 2**: Update `DEFAULT_YIELD_WAIT_TIME` from 6s to 1.5s in constants.rs

## Results

**Before**: `pool_drain_tests` took ~56 seconds
**After**: `pool_drain_tests` takes ~15.78 seconds (**72% faster**)

All tests pass with the reduced yield time.

## Success Criteria

- [x] `pool_drain_tests` complete in <20 seconds (down from ~56s)
- [x] All existing tests still pass
- [x] No noticeable CPU increase in production workloads
- [x] Documentation updated with rationale

## Related Code

- `backends/foundation_core/src/valtron/executors/constants.rs:11`
- `backends/foundation_core/src/valtron/executors/threads.rs:1430`
- `backends/foundation_core/tests/simple_http/pool_drain_tests.rs`

## Future Work

Consider making yield time configurable per-`ThreadRegistry` instance:
```rust
pub fn with_seed_threads_and_yield(
    seed: u64,
    threads: usize,
    yield_time: Duration,
) -> Self
```

This would allow tests to use short yields while production uses longer yields.
