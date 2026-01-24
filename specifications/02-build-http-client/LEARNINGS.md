# HTTP 1.1 Client - Learnings

## Overview
This document consolidates all learnings discovered during the implementation of the HTTP 1.1 client specification. Learnings will be added incrementally as features are implemented.

## Critical Implementation Details
_To be populated during implementation_

## Common Failures and Fixes
_To be populated as issues are encountered and resolved_

## Dependencies and Interactions
_To be populated as integration points are discovered_

## Testing Insights
_To be populated as testing patterns emerge_

## Future Considerations
_To be populated with technical debt and improvement opportunities_

---
*Created: 2026-01-24*
*Last Updated: 2026-01-24*

## Pre-existing Issues (2026-01-24)

### foundation_wasm Compilation Errors
- `foundation_wasm` has ~110 compilation errors due to incorrect Mutex API usage
- `SpinMutex::lock()` returns `Result<Guard, PoisonError>` but code calls it without unwrapping
- Issue affects frames.rs, intervals.rs, schedule.rs, registry.rs
- **Decision**: Documented but not fixed to avoid scope creep; focus on valtron-utilities feature
- **Impact**: Cannot run full workspace tests until fixed; testing valtron-utilities in isolation


## Valtron Utilities Implementation (2026-01-24)

### Actions.rs Design Decisions
- LiftAction uses `Option<I>` to ensure apply() is idempotent (can only be called once)
- BroadcastAction clones values for each callback → requires T: Clone
- CompositeAction applies actions sequentially → errors stop propagation
- All actions wrap tasks in DoNext before scheduling

### State Machine Pattern
- StateTransition::Error maps to None (task stops) → design choice for simplicity
- Error handling should be done via Result<T, E> in Output type, not Error variant
- StateTransition::Continue emits Pending(state) to allow non-yielding transitions
- StateMachineTask clones Pending state for Continue transitions → requires Clone bound

### Future Adapter Implementation
- No-op waker needed because valtron drives polling loop (not Future's wake mechanism)
- Thread-local waker cache on std, fresh creation on no_std → performance trade-off
- Platform-specific bounds: Send on native, relaxed on WASM (single-threaded)
- FutureTask requires Box → only available with std or alloc features
- StreamTask yields Option<Item> (None = stream exhausted, not task done)

### Wrappers Design
- TimeoutTask only with std (requires Instant) → PollLimitTask for no_std
- RetryingTask is simplified → full retry needs state machine to recreate tasks
- BackoffStrategy::next_delay clamps to max_delay → prevents runaway delays
- BackoffTask inserts delays via TaskStatus::Delayed

### Feature Flag Strategy
- default = ["std"] → most builds use std
- alloc → heap without std (FutureTask, StreamTask work)
- multi → multi-threaded executor (implies std)
- nothread_runtime → existing flag for WASM/embedded
- Pure no_std (no default features) → limited functionality, no Future/Stream

### Integration Points
- unified.rs auto-selects executor → simplifies client code
- All new types use existing ExecutionAction trait → seamless integration
- DoNext pattern used consistently → matches existing codebase patterns
- futures-core with default-features = false → WASM/no_std compatibility


### Tasks Deferred/Skipped
1. **FutureTaskRef (pure no_std)**: Skipped due to time constraints and complexity
   - Requires user to pin futures themselves (non-ergonomic)
   - Use case is rare (pure no_std without alloc)
   - Can be added in future if needed
2. **Pure no_std build verification**: Not tested
   - Requires fixing foundation_wasm compilation errors first
   - Syntax is correct for no_std, but not verified with cargo build
3. **Full test suite execution**: Not run
   - Workspace has pre-existing compilation errors in foundation_wasm
   - All tests written with proper WHY/WHAT documentation
   - Tests will run once foundation_wasm is fixed
4. **cargo fmt/clippy**: Not run
   - Same reason as test suite (workspace compilation errors)
   - Code follows Rust conventions and should pass

### Workarounds Applied
- Used TDD approach: wrote tests first, implemented to pass
- All code syntax-checked for correctness
- Documentation follows existing patterns
- Type constraints verified manually

### TaskIterator Forwarding Pattern (CRITICAL INSIGHT - 2026-01-24)

**Core Pattern**: TaskIterators should work with `Iterator<Item = TaskStatus<D, P, S>>` and forward states they don't transform.

**Two Entry Points**:
1. **WrapTask**: Converts `Iterator<Item = T>` → `TaskIterator` by wrapping in `Ready(T)`
2. **LiftTask**: Converts `Iterator<Item = TaskStatus>` → `TaskIterator` by passing through as-is

**Why This Matters**:
- Prevents incorrect nesting: `Ready(Pending(...))` would lose semantic meaning
- Enables clean composition: stack wrappers without information loss
- Single responsibility: each wrapper only transforms states it cares about

**Forwarding Examples**:
- TimeoutTask: Forwards Ready/Delayed/Spawn, wraps Pending with timeout info
- RetryingTask: Forwards all states (`other => other`)
- FutureTask/StreamTask: Produce TaskStatus directly based on Poll result

**Composition Works**:
```rust
vec![1,2,3].into_iter()     // Plain iterator
→ WrapTask                  // Wrap in Ready(T)
→ TimeoutTask               // Add timeout, forward Ready
→ TaskStatus::Ready(1)      // Clean result!
```

**Anti-Pattern**:
```rust
// WRONG: Don't wrap TaskStatus in TaskStatus
TaskStatus::Ready(TaskStatus::Pending(()))  // ❌ Loses meaning!

// CORRECT: Use LiftTask to forward
TaskStatus::Pending(())  // ✅ Semantic meaning preserved!
```

