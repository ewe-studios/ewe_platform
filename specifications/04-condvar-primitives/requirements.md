---
description: Implement CondVar (Condition Variable) primitives in foundation_nostd for no_std and WASM contexts with full std::sync::Condvar API compatibility
status: in-progress
priority: high
created: 2026-01-23
author: "Main Agent"
metadata:
  version: "1.0"
  last_updated: 2026-01-23
  estimated_effort: large
  tags:
    - no_std
    - wasm
    - condvar
    - synchronization
    - primitives
    - condition-variable
  stack_files:
    - .agents/stacks/rust.md
  skills: []
builds_on:
  - 03-wasm-friendly-sync-primitives
related_specs: []
has_features: false
has_fundamentals: true
---

# CondVar Primitives - Requirements

> **Specification Tracking**: See [tasks.md](./tasks.md) for task progress and [learnings.md](./learnings.md) for implementation insights.

## Overview

Implement comprehensive Condition Variable (CondVar) primitives in `foundation_nostd` that provide full API compatibility with `std::sync::Condvar` for use in no_std and WASM contexts. Building on the synchronization primitives from specification 03, this adds CondVar variants that work seamlessly with our existing Mutex and RwLock implementations, supporting both poisoning and non-poisoning modes for maximum flexibility.

**Key Principles:**
- Full `std::sync::Condvar` API parity for drop-in replacement
- Integration with existing primitives from spec 03 (Mutex, RwLock)
- Multiple variants: poisoning, non-poisoning, and RwLock-specific
- WASM-optimized for single-threaded and multi-threaded contexts
- Comprehensive fundamentals documentation with theory and usage
- 100% test coverage including WASM-specific scenarios
- Benchmarks and stress testing using Criterion

## Requirements Conversation Summary

### User's Initial Request

Implement CondVar (Condition Variable) in the `foundation_nostd` crate's primitives submodule, building on the work from specification 03 (wasm-friendly sync primitives).

### Clarifying Questions Asked

1. **Scope and Features**: Should this support all standard condition variable operations?
   - Answer: Yes, support all methods for 1:1 parity with std version (wait, notify_one, notify_all, wait_timeout, wait_timeout_while, wait_while)

2. **Integration with Existing Primitives**: Should this work with the Mutex from spec 03?
   - Answer: Yes, create version working with our Primitives Mutex, implement custom versions built on other available primitive types

3. **WASM Compatibility**: What WASM-specific considerations needed?
   - Answer: Follow same pattern as spec 03, use thread parking/unparking mechanisms already established

4. **API Design**: Mirror std::sync::Condvar or different design?
   - Answer: API should mirror std::sync::Condvar closely, maintain naming convention CondVar

5. **Poisoning Support**: Need poisoning like std's Condvar?
   - Answer: Implement both poisoning and non-poisoning versions - one for std compatibility, another for contexts where poisoning isn't a concern (e.g., WASM)

6. **Testing Requirements**: What level of test coverage?
   - Answer: 100% test coverage, test WASM-specific scenarios in both multi-threaded and single-threaded contexts

7. **Documentation**: Need fundamentals documentation?
   - Answer: Absolutely - create foundational documentation covering theory, usage, bit-masking techniques for state management, comparison with std::sync::Condvar

8. **Dependencies**: Can we use same dependencies from spec 03?
   - Answer: Yes, build directly into foundation_nostd crate using same dependencies and new primitives from spec 03

9. **WASM Considerations**: Memory constraints and sensible patterns?
   - Answer: Think deeply about memory constraints for WASM versions, implement sensible defaults

10. **Specification Structure**: New specification or extend spec 03?
    - Answer: Create new specification (spec 03 is marked completed), but explicitly reference spec 03 via `builds_on`

11. **Variants and Naming**: What naming for poisoning vs non-poisoning?
    - Answer: `CondVar` (poisoning), `CondVarNonPoisoning` (non-poisoning), `RwLockCondVar` (RwLock integration)

12. **Integration with RwLock**: Should CondVar integrate with RwLock?
    - Answer: Yes, create `RwLockCondVar` variant that works with RwLock

13. **Error Handling**: Return types for timeout and poisoned states?
    - Answer: Use `WaitTimeoutResult` like std, use `PoisonError<MutexGuard<T>>` like std, think deeply about WASM-specific errors

14. **Module Structure**: Where should this live?
    - Answer: `foundation_nostd::primitives::condvar` - single file within primitives module

15. **Documentation Focus**: Theory vs usage?
    - Answer: Both - users want breadth to understand what's happening, examples on how to use, theory covering bit-masking for state management, comparison table at end

16. **Success Criteria**: What does "done" look like?
    - Answer: Users can use CondVar in projects, 100% tested, well documented in theory, usage, and fundamentals

17. **Benchmarks and Stress Tests**: Need these?
    - Answer: Yes, add sensible benchmarks and stress testing via Criterion crate (only allowed outside dependency), create new modules for specific capability testing

18. **WASM Test Environments**: Specific environments to test?
    - Answer: Write tests for single-threaded situations, optionally add tests when specific target is WASM to test WASM-specific behaviors

19. **Test Orchestration**: How to run different test scenarios?
    - Answer: Create Makefile in foundation_nostd root for `make tests` with specific commands for different scenarios and architectures (e.g., WASM)

### Final Requirements Agreement

Based on the conversation, we agreed on:
- Implement three CondVar variants: `CondVar` (std-compatible with poisoning), `CondVarNonPoisoning` (simplified for WASM/embedded), and `RwLockCondVar` (RwLock integration)
- Full API parity with `std::sync::Condvar` including all wait variants and notify operations
- Integration with existing Mutex and RwLock primitives from specification 03
- Comprehensive fundamentals documentation explaining theory, bit-masking techniques, and providing usage comparison tables
- 100% test coverage with WASM-specific single-threaded and multi-threaded scenario tests
- Benchmarks and stress tests using Criterion
- Makefile for test orchestration across different architectures
- Location: `foundation_nostd/src/primitives/condvar.rs`

## Detailed Requirements

### Functional Requirements

#### 1. CondVar Variants

Implement three distinct CondVar variants:

| Variant | Description | Poisoning | Integration Point |
|---------|-------------|-----------|-------------------|
| `CondVar` | Full std::sync::Condvar compatibility | Yes | Works with `SpinMutex<T>` from spec 03 |
| `CondVarNonPoisoning` | Simplified without poisoning overhead | No | Works with `RawSpinMutex<T>` from spec 03 |
| `RwLockCondVar` | Condition variable for read-write locks | Yes | Works with `SpinRwLock<T>` from spec 03 |

#### 2. Core API Methods

Each variant must implement:

**Wait Operations:**
- `wait(&self, guard: MutexGuard<'_, T>) -> LockResult<MutexGuard<'_, T>>`
- `wait_while<F>(&self, guard: MutexGuard<'_, T>, condition: F) -> LockResult<MutexGuard<'_, T>>`
  - Where `F: FnMut(&mut T) -> bool`
- `wait_timeout(&self, guard: MutexGuard<'_, T>, dur: Duration) -> LockResult<(MutexGuard<'_, T>, WaitTimeoutResult)>`
- `wait_timeout_while<F>(&self, guard: MutexGuard<'_, T>, dur: Duration, condition: F) -> LockResult<(MutexGuard<'_, T>, WaitTimeoutResult)>`

**Notify Operations:**
- `notify_one(&self)` - Wake up one waiting thread
- `notify_all(&self)` - Wake up all waiting threads

**Non-Poisoning Variants** return unwrapped guards instead of `LockResult`.

#### 3. Error Handling

**WaitTimeoutResult:**
```rust
pub struct WaitTimeoutResult(bool);

impl WaitTimeoutResult {
    pub fn timed_out(&self) -> bool;
}
```

**PoisonError** (for poisoning variants):
- Match `std::sync::PoisonError<Guard>` behavior
- Include recovery methods: `into_inner()`, `get_ref()`, `get_mut()`

**WASM-Specific Errors:**
- Handle single-threaded WASM context gracefully (no-op or immediate return for notify operations)
- Detect and optimize for WASM runtime limitations
- Proper handling when thread parking unavailable

#### 4. Spurious Wakeups

**Requirement**: Support spurious wakeups as per std::sync::Condvar specification.

- `wait()` may wake up without `notify_one()` or `notify_all()` being called
- `wait_while()` re-checks predicate on each wakeup (handles spurious wakeups automatically)
- Documentation must clearly explain spurious wakeup behavior
- Tests should validate correct handling

#### 5. State Management

**Bit-Masking for Compact State Representation:**

Use atomic integers with bit-masking to efficiently track multiple states:
- Waiting thread count
- Notification pending flag
- Poisoned state flag (for poisoning variants)
- Lock status

Example bit layout (document in fundamentals):
```
Bits 0-30:  Waiting thread count (31 bits)
Bit 31:     Notification pending flag
Bit 32-33:  Poison state
```

Document this technique thoroughly in fundamentals with examples and rationale.

### Non-Functional Requirements

#### 1. Performance

- **Lock-free fast paths**: Check and update state atomically where possible
- **Minimal spinning**: Use proper thread parking/unparking from spec 03
- **WASM optimization**: Detect single-threaded context, avoid unnecessary atomic operations
- **Memory efficiency**: Compact state representation using bit-masking
- **Benchmark targets** (establish baselines with Criterion):
  - Wait/notify latency < 100ns in uncontended case
  - Notify_all scales linearly with waiting thread count
  - Memory overhead < 64 bytes per CondVar

#### 2. Safety and Soundness

- **No data races**: All operations properly synchronized
- **No deadlocks**: Careful lock ordering, no circular dependencies
- **Panic safety**: Proper cleanup on panic, poisoning where applicable
- **WASM safety**: Correct behavior in single-threaded WASM environments
- **Memory safety**: No unsafe code unless absolutely necessary, thoroughly documented
- **Send/Sync bounds**: Properly implement where appropriate

#### 3. WASM Compatibility

- **Single-threaded detection**: Optimize for WASM single-threaded runtime
- **Multi-threaded support**: Work correctly with WASM threads when available
- **Memory constraints**:
  - Minimize per-CondVar memory overhead (target: 32-64 bytes)
  - Avoid heap allocations in hot paths
  - Use stack-based data structures where possible
- **No wasm_bindgen dependency**: Pure Rust using `core::sync::atomic`

#### 4. Testing

**100% code coverage including:**
- All public API methods
- All variants (CondVar, CondVarNonPoisoning, RwLockCondVar)
- Error paths (timeout, poisoning)
- Edge cases (spurious wakeups, immediate timeout, zero duration)

**WASM-specific tests:**
- Single-threaded WASM context behavior
- Multi-threaded WASM with threads support
- Target-conditional tests for WASM-only behaviors

**Stress tests:**
- High contention scenarios (many threads)
- Rapid notify/wait cycles
- Long-running wait operations
- Concurrent notify_one and notify_all

**Criterion benchmarks:**
- Wait/notify latency
- Throughput under contention
- Scaling with thread count
- Comparison with std::sync::Condvar (when available)

### Technical Specifications

#### Technology Stack

- **Language**: Rust (no_std compatible)
- **Core Dependencies**:
  - `core::sync::atomic` for atomics
  - `core::time::Duration` for timeouts
  - Existing primitives from spec 03 (SpinMutex, RawSpinMutex, SpinRwLock)
- **Testing Dependencies**:
  - Criterion (benchmarks)
  - Standard Rust testing framework
- **Build Tools**:
  - Makefile for test orchestration
  - Cargo for building

#### Module Structure

**Location**: `foundation_nostd/src/primitives/condvar.rs`

**Module layout:**
```rust
// Public API
pub struct CondVar { ... }
pub struct CondVarNonPoisoning { ... }
pub struct RwLockCondVar { ... }

pub struct WaitTimeoutResult(bool);

// Internal implementation
mod inner {
    // Shared wait queue implementation
    // Thread parking/unparking integration
    // State management with bit-masking
}

// WASM-specific optimizations
#[cfg(target_family = "wasm")]
mod wasm {
    // Single-threaded detection
    // Optimized implementations
}
```

#### Integration Points

**With Mutex (from spec 03):**
- `CondVar` works with `SpinMutex<T>` and its `MutexGuard`
- `CondVarNonPoisoning` works with `RawSpinMutex<T>` and its guard
- Reuse guard types, no new lock implementations needed

**With RwLock (from spec 03):**
- `RwLockCondVar` works with `SpinRwLock<T>`
- Supports both read and write guards
- Careful handling of writer preference policy

**With Thread Parking (from spec 03):**
- Reuse existing parking/unparking mechanisms
- Integrate with wait queue implementation
- Proper wakeup ordering (FIFO)

#### Dependencies from Specification 03

This specification builds directly on:
- `SpinMutex<T>` and `RawSpinMutex<T>` - for CondVar and CondVarNonPoisoning
- `SpinRwLock<T>` - for RwLockCondVar
- Thread parking/unparking primitives - for wait/notify implementation
- WASM detection utilities - for single-threaded optimization
- Guard types (`MutexGuard`, `RwLockReadGuard`, `RwLockWriteGuard`)

## User-Facing Documentation Requirements (MANDATORY)

**CRITICAL**: This specification introduces new user-facing primitives requiring comprehensive fundamentals documentation.

### Fundamentals Documentation (REQUIRED - has_fundamentals: true)

Create the following documents in `specifications/04-condvar-primitives/fundamentals/`:

#### 1. **00-overview.md** - Introduction and Quick Start
- What are condition variables and when to use them
- Quick start guide for each variant
- Decision tree: which variant to use (CondVar vs CondVarNonPoisoning vs RwLockCondVar)
- Common patterns and anti-patterns
- Migration guide from std::sync::Condvar

#### 2. **01-condvar-theory.md** - Condition Variable Theory
- What problem do condition variables solve
- Wait/notify semantics in depth
- Spurious wakeups: what they are and why they happen
- Comparison with other synchronization primitives (semaphores, barriers)
- Memory ordering considerations
- Lock-free vs lock-based coordination

#### 3. **02-implementation-details.md** - Internal Implementation
- Wait queue data structure and algorithms
- Bit-masking technique for state management with examples
  - Show exact bit layouts
  - Explain why bit-masking is used (memory efficiency, atomic operations)
  - Code examples of bit manipulation
- Thread parking and unparking integration
- Notification mechanisms (one vs all)
- WASM single-threaded detection and optimization
- Performance characteristics and trade-offs

#### 4. **03-variants-comparison.md** - Variant Selection Guide
- Detailed comparison table of all three variants
- When to use `CondVar` (std compatibility, poisoning needed)
- When to use `CondVarNonPoisoning` (WASM, embedded, panic=abort)
- When to use `RwLockCondVar` (read-write lock coordination)
- Performance comparison (memory, latency, throughput)
- API differences between variants

#### 5. **04-usage-patterns.md** - Common Usage Patterns
- Producer-consumer queue
- Thread pool work distribution
- Barrier implementation using CondVar
- Event notification
- State machine synchronization
- Timeout patterns
- Error handling and recovery

#### 6. **05-wasm-considerations.md** - WASM-Specific Guide
- Single-threaded vs multi-threaded WASM
- WASM threading model limitations
- Which variant to use in WASM (recommendation: CondVarNonPoisoning)
- Memory constraints and optimization tips
- Testing WASM code with CondVar
- Common pitfalls and solutions

#### 7. **06-std-compatibility.md** - Migration and Compatibility
- Side-by-side API comparison with std::sync::Condvar
- Drop-in replacement guide
- Behavior differences (if any)
- Performance comparison with std
- When to use std vs foundation_nostd CondVar
- Testing strategy for migration

**Documentation Principles**:
- **Explain WHY** - Design decisions, trade-offs, not just mechanics
- **Show internals** - Bit-masking examples, state transitions with diagrams
- **Provide examples** - Compilable, real-world usage for each variant
- **Discuss trade-offs** - When to use each variant, when NOT to use
- **Be self-contained** - Reader can understand without external resources
- **Include diagrams** - State transitions, wait queue structure, thread interactions

**Add fundamentals documentation tasks to tasks.md as HIGH PRIORITY items.**

## Success Criteria

### Implementation Success

- [ ] `CondVar` fully implemented with poisoning support
- [ ] `CondVarNonPoisoning` fully implemented without poisoning
- [ ] `RwLockCondVar` fully implemented for RwLock integration
- [ ] All API methods implemented (wait, wait_while, wait_timeout, wait_timeout_while, notify_one, notify_all)
- [ ] Integration with Mutex and RwLock from spec 03 working
- [ ] Error types implemented (WaitTimeoutResult, PoisonError)
- [ ] Spurious wakeup handling correct
- [ ] WASM single-threaded optimization implemented
- [ ] Bit-masking state management implemented

### Documentation Success (has_fundamentals: true)

- [ ] All 7 fundamental documents created (00-06)
- [ ] Theory documentation comprehensive with bit-masking examples
- [ ] Usage patterns documented with real examples
- [ ] WASM-specific guide complete
- [ ] Comparison table with std::sync::Condvar included
- [ ] All code examples compile and are correct
- [ ] Trade-offs and design decisions thoroughly explained
- [ ] API documentation complete for all public items with examples

### Testing Success (MANDATORY - 100% COVERAGE)

- [ ] Unit tests for all public API methods (100% coverage)
- [ ] Tests for all three variants
- [ ] Edge case tests (zero timeout, immediate timeout, spurious wakeups)
- [ ] Poisoning tests (for CondVar and RwLockCondVar)
- [ ] Non-poisoning behavior tests (for CondVarNonPoisoning)
- [ ] WASM single-threaded tests
- [ ] WASM multi-threaded tests (with wasm32 target)
- [ ] Stress tests (high contention, many threads, rapid cycles)
- [ ] Integration tests with Mutex and RwLock
- [ ] Timeout accuracy tests
- [ ] Spurious wakeup simulation tests

### Benchmarking Success

- [ ] Criterion benchmarks suite created
- [ ] Wait/notify latency benchmarks
- [ ] Throughput under contention benchmarks
- [ ] Scaling with thread count benchmarks
- [ ] Comparison with std::sync::Condvar (when std available)
- [ ] WASM-specific performance benchmarks
- [ ] Baseline metrics established and documented

### Infrastructure Success

- [ ] Makefile created in foundation_nostd root
- [ ] `make tests` target runs all tests
- [ ] `make test-wasm` target runs WASM-specific tests
- [ ] `make bench` target runs benchmarks
- [ ] `make stress` target runs stress tests
- [ ] CI integration considerations documented

### Quality Success (MANDATORY - NO EXCEPTIONS)

- [ ] All tests passing (100%)
- [ ] Zero clippy warnings (all lints enabled)
- [ ] Zero compiler warnings
- [ ] Code properly formatted (rustfmt)
- [ ] All public items have comprehensive doc comments
- [ ] Examples in doc comments compile and run
- [ ] No unsafe code OR unsafe code thoroughly documented and justified
- [ ] Memory safety verified
- [ ] No data races (validated with thread sanitizer if possible)

## Module Documentation References

This specification modifies the following modules:

### foundation_nostd/primitives

- **Documentation**: `documentation/foundation_nostd-primitives/doc.md` (if exists, otherwise create)
- **Purpose**: No_std synchronization primitives module
- **Changes Needed**:
  - Add new `condvar.rs` file with CondVar implementations
  - Update module exports to include CondVar variants
  - Update module-level documentation to reference CondVar primitives

**CRITICAL**: Agents MUST read module documentation BEFORE making changes. If documentation doesn't exist, create it first.

## Agent Rules Reference

**MANDATORY**: All agents working on this specification MUST load the rules listed below.

### All Agents (Mandatory)

Load these rules from `.agents/rules/`:

| Rule | File | Purpose |
|------|------|---------|
| 01 | `.agents/rules/01-rule-naming-and-structure.md` | File naming conventions |
| 02 | `.agents/rules/02-rules-directory-policy.md` | Directory policies |
| 03 | `.agents/rules/03-dangerous-operations-safety.md` | Dangerous operations safety |
| 04 | `.agents/rules/04-work-commit-and-push-rules.md` | Work commit and push rules |

### By Agent Role

| Agent Type | Additional Rules to Load |
|------------|--------------------------|
| **Review Agent** | `.agents/rules/06-specifications-and-requirements.md` |
| **Implementation Agent** | `.agents/rules/13-implementation-agent-guide.md`, `.agents/stacks/rust.md` |
| **Verification Agent** | `.agents/rules/08-verification-workflow-complete-guide.md`, `.agents/stacks/rust.md` |
| **Documentation Agent** | `.agents/rules/06-specifications-and-requirements.md` |

### Stack Files

**Language**: Rust → `.agents/stacks/rust.md`

### Skills Referenced

None

---

## MANDATORY Completion and Verification Requirements

**CRITICAL**: Before marking this specification as complete, ALL of the following MUST be verified:

### 1. Task Completion Verification (100% REQUIRED)

**NO EXCEPTIONS**: Every task in `tasks.md` MUST be completed.

- [ ] Open `tasks.md` and verify ALL tasks are marked `[x]`
- [ ] Verify `completed` count in frontmatter matches actual `[x]` count
- [ ] Verify `uncompleted` count is `0`
- [ ] Verify `completion_percentage` is `100`
- [ ] NO tasks left as `[ ]` (incomplete)

**If ANY task is incomplete**:
- Status MUST remain `in-progress`
- Specification CANNOT be marked complete
- Work MUST continue until 100%

### 2. Code/Implementation Verification

- [ ] All three CondVar variants implemented: `CondVar`, `CondVarNonPoisoning`, `RwLockCondVar`
- [ ] All API methods exist and work correctly
- [ ] Integration with spec 03 primitives (Mutex, RwLock) working
- [ ] Error types (WaitTimeoutResult, PoisonError) implemented correctly
- [ ] Spurious wakeup handling verified
- [ ] WASM optimizations in place and tested
- [ ] State management with bit-masking implemented and documented
- [ ] No stub implementations, all code functional

### 3. Documentation Verification

**Fundamentals (MANDATORY - has_fundamentals: true)**:
- [ ] `fundamentals/00-overview.md` exists and is comprehensive
- [ ] `fundamentals/01-condvar-theory.md` exists with complete theory
- [ ] `fundamentals/02-implementation-details.md` exists with bit-masking examples
- [ ] `fundamentals/03-variants-comparison.md` exists with comparison table
- [ ] `fundamentals/04-usage-patterns.md` exists with real examples
- [ ] `fundamentals/05-wasm-considerations.md` exists with WASM guide
- [ ] `fundamentals/06-std-compatibility.md` exists with migration guide
- [ ] All documents follow documentation principles (WHY, internals, examples, trade-offs)
- [ ] All code examples in fundamentals compile
- [ ] Bit-masking technique thoroughly explained with diagrams

**API Documentation**:
- [ ] Every public struct has doc comments
- [ ] Every public method has doc comments with examples
- [ ] Safety considerations documented
- [ ] Examples compile and are correct
- [ ] Warnings and edge cases documented

### 4. Quality Verification (MANDATORY - NO EXCEPTIONS)

**Build Status**:
- [ ] `cargo build` succeeds with ZERO warnings
- [ ] `cargo build --release` succeeds with ZERO warnings
- [ ] `cargo build --target wasm32-unknown-unknown` succeeds (WASM target)

**Linting** (ZERO TOLERANCE):
- [ ] `cargo clippy -- -D warnings` passes with ZERO warnings
- [ ] All clippy suggestions addressed or explicitly allowed with justification
- [ ] NO clippy warnings ignored without documented reason

**Testing** (100% REQUIRED):
- [ ] `cargo test` passes - ALL tests pass, ZERO failures
- [ ] `cargo test --release` passes
- [ ] WASM tests pass: `cargo test --target wasm32-unknown-unknown`
- [ ] Integration tests pass
- [ ] Stress tests pass: `make stress` (if Makefile target exists)
- [ ] Test coverage 100% (verify with coverage tool)

**Benchmarks**:
- [ ] `cargo bench` runs successfully
- [ ] Criterion benchmarks complete without errors
- [ ] Baseline metrics established

**Formatting**:
- [ ] `cargo fmt -- --check` passes (code properly formatted)

**CRITICAL**: If ANY quality check fails, status MUST remain `in-progress`.

### 5. Specification Tracking Verification

- [ ] `PROGRESS.md` exists and documents implementation journey
- [ ] `FINAL_REPORT.md` exists with comprehensive completion summary
- [ ] `LEARNINGS.md` exists with lessons learned and insights
- [ ] `VERIFICATION_SIGNOFF.md` exists (created by Verification Agent after final verification)
- [ ] All cross-reference links working (requirements.md ↔ tasks.md ↔ learnings.md ↔ verification)

### 6. Fundamentals Priority (MANDATORY)

**CRITICAL**: Fundamentals documentation must be created FIRST, before implementation.

- [ ] Fundamentals documents created BEFORE implementation coding started
- [ ] Fundamentals tasks marked complete BEFORE implementation tasks
- [ ] API design influenced by documentation (documentation-driven development)

**If fundamentals created AFTER implementation**: VIOLATION - document why this happened in LEARNINGS.md

### 7. Verification Issue Resolution (ZERO TOLERANCE)

When Verification Agent completes verification:

- [ ] ALL issues marked as MUST FIX have been fixed
- [ ] NO issues remain with severity HIGH or CRITICAL
- [ ] ALL clippy warnings resolved (zero warnings)
- [ ] ALL test failures resolved (zero failures)
- [ ] NO "optional" fixes ignored - ALL issues fixed

**If ANY issue remains unresolved**:
- Specification CANNOT be marked complete
- Status reverted to `in-progress`
- Work continues until ALL issues resolved

### 8. Git Safety (MANDATORY)

- [ ] All changes committed following Rule 04 (atomic commits)
- [ ] All commits include verification status in message
- [ ] All commits pushed to remote (NO unpushed commits)
- [ ] Final commit includes "Specification 04 complete, all verification passed"
- [ ] Git push succeeded (verify with `git status`)

**ZERO TOLERANCE**:
- ❌ NO unpushed commits after marking complete
- ❌ NO "WIP" commits without verification
- ❌ NO skipping commit messages

---

## Final Verification Checklist

**Main Agent MUST validate before setting status to "completed"**:

```bash
# 1. Task validation
grep -c '\[ \]' specifications/04-condvar-primitives/tasks.md
# MUST return 0 (zero unchecked tasks)

# 2. File existence validation
ls specifications/04-condvar-primitives/fundamentals/
# MUST show all 7 fundamental documents (00-06)

ls specifications/04-condvar-primitives/{PROGRESS,FINAL_REPORT,LEARNINGS,VERIFICATION_SIGNOFF}.md
# MUST show all 4 tracking files

# 3. Quality validation
cd foundation_nostd
cargo clippy -- -D warnings
# MUST pass with zero warnings

cargo test
# MUST pass with zero failures

cargo test --target wasm32-unknown-unknown
# MUST pass (WASM tests)

# 4. Git validation
git status
# MUST show "nothing to commit, working tree clean"
# MUST show "Your branch is up to date with 'origin/main'"
```

**ONLY after ALL validations pass** can status be set to "completed".

---

> **Final Verification**: See [VERIFICATION_SIGNOFF.md](./VERIFICATION_SIGNOFF.md) for verification results.
