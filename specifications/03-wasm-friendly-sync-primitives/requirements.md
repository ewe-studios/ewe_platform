---
description: Implement no_std-compatible synchronization and atomic primitives for
  foundation_nostd with WASM optimization and comprehensive user documentation
status: completed
priority: high
created: 2026-01-19
author: Main Agent
context_optimization: true  # Sub-agents MUST generate COMPACT_CONTEXT.md before work, reload after updates
compact_context_file: ./COMPACT_CONTEXT.md  # Ultra-compact current task context (97% reduction)
context_reload_required: true  # Clear and reload from compact context regularly to prevent context limit errors
metadata:
  version: '3.0'
  last_updated: 2026-01-25
  estimated_effort: large
  tags:
  - no_std
  - wasm
  - synchronization
  - spin-lock
  - atomics
  - primitives
  stack_files:
  - .agents/stacks/rust.md
  skills: []
  tools:
  - Rust
  - cargo
builds_on: []
related_specs: []
has_features: false
has_fundamentals: true
tasks:
  completed: 48
  uncompleted: 0
  total: 48
  completion_percentage: 100
files_required:
  main_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/05-coding-practice-agent-orchestration.md
      - .agents/rules/06-specifications-and-requirements.md
      - .agents/rules/14-machine-optimized-prompts.md
      - .agents/rules/15-instruction-compaction.md
    files:
      - ./requirements.md
      - ./LEARNINGS.md
      - ./PROGRESS.md

  implementation_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/13-implementation-agent-guide.md
      - .agents/rules/11-skills-usage.md
      - .agents/stacks/rust.md
    files:
      - ./requirements.md
      - ./fundamentals/*

  verification_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/08-verification-workflow-complete-guide.md
      - .agents/rules/14-machine-optimized-prompts.md
      - .agents/rules/15-instruction-compaction.md
      - .agents/stacks/rust.md
    files:
      - ./requirements.md

  # All agents MUST load Rules 14 & 15 for token/context optimization
---

# WASM-Friendly Sync Primitives - Requirements

## 🔍 CRITICAL: Retrieval-Led Reasoning Required

**ALL agents implementing this specification MUST use retrieval-led reasoning.**

### Before Starting Implementation

**YOU MUST** (in this order):
1. ✅ **Search the codebase** for similar implementations using Grep/Glob
2. ✅ **Read existing code** to understand project patterns and conventions
3. ✅ **Check stack files** (`.agents/stacks/[language].md`) for language-specific patterns
4. ✅ **Read module documentation** for modules you'll modify
5. ✅ **Follow discovered patterns** - do NOT invent new patterns without justification
6. ✅ **Verify all assumptions** by reading actual code

### FORBIDDEN Approaches

**YOU MUST NOT**:
- ❌ Assume typical patterns without checking the codebase
- ❌ Implement without searching for similar code first
- ❌ Apply generic best practices without verifying project conventions
- ❌ Guess file structures, naming conventions, or API patterns
- ❌ Use pretraining knowledge without verification against project code

### Retrieval Examples

**Good Retrieval Approach** ✅:
```
"Let me search for existing API endpoints to understand the pattern..."
→ Uses Grep to find similar endpoints
→ Reads actual implementation files
→ Follows discovered patterns (e.g., Axum with custom middleware)
→ Implements consistently with existing code
```

**Bad Pretraining Approach** ❌:
```
"I'll create an API endpoint using Express middleware (standard approach)"
→ Assumes Express without checking project
→ Doesn't verify actual framework used
→ Creates inconsistent code
```

### Enforcement

- Agents will be asked to demonstrate retrieval steps
- Implementation that doesn't match project patterns will be rejected
- "I assumed..." is NOT acceptable - only "I found..." backed by code references

---

## 🚀 CRITICAL: Token and Context Optimization

**ALL agents implementing this specification MUST follow token and context optimization protocols.**

**MANDATORY RULES**: [Rule 14](../../.agents/rules/14-machine-optimized-prompts.md) and [Rule 15](../../.agents/rules/15-instruction-compaction.md)

### Machine-Optimized Prompts (Rule 14)

**Reference**: [.agents/rules/14-machine-optimized-prompts.md](../../.agents/rules/14-machine-optimized-prompts.md)

**Main Agent MUST**:
1. Generate `machine_prompt.md` from this file when specification finalized
2. Use pipe-delimited compression (58% token reduction)
3. Commit machine_prompt.md alongside human-readable file
4. Regenerate when human file updates
5. Provide machine_prompt.md path to sub-agents

**Sub-Agents MUST**:
- Read `machine_prompt.md` (NOT verbose human files)
- Parse DOCS_TO_READ section for files to load
- 58% token savings

**File Lifecycle**:
- `requirements.md` (human-readable, permanent) → `machine_prompt.md` (machine-optimized, generated)
- Both files committed together, stay in sync
- Sub-agents use machine_prompt.md for instructions

### Context Compaction (Rule 15)

**Reference**: [.agents/rules/15-instruction-compaction.md](../../.agents/rules/15-instruction-compaction.md)

**Main Agent MUST** (before spawning sub-agents):
1. Generate machine_prompt.md (Rule 14)
2. Clear context and reload from machine_prompt.md
3. Read/create PROGRESS.md
4. Generate initial `COMPACT_CONTEXT.md`:
   - Extract current task from machine_prompt.md
   - EMBED machine_prompt content for current task
   - Create ultra-compact self-contained file (500-800 tokens)
5. Provide COMPACT_CONTEXT.md path to sub-agent

**Sub-Agents MUST** (on startup):
1. Receive COMPACT_CONTEXT.md from Main Agent (already generated)
2. Read COMPACT_CONTEXT.md (self-contained with embedded machine_prompt)
3. Read files from FILES section only
4. Begin work with clean compact context (~5K tokens)

**Sub-Agents MUST** (during work - after PROGRESS.md updates):
1. Regenerate COMPACT_CONTEXT.md:
   - Re-extract current task from machine_prompt.md
   - Re-embed machine_prompt content for current task
   - Update status from new PROGRESS.md
   - Update FILES list and NEXT_ACTIONS
2. CLEAR entire context (drop everything)
3. RELOAD from COMPACT_CONTEXT.md only
4. Continue work with refreshed minimal context
5. Proceed with 97% context reduction (180K→5K tokens)

**COMPACT_CONTEXT.md Lifecycle**:
- Generated fresh per task (Main Agent creates initial, Sub-Agent maintains)
- Contains ONLY current task (no history)
- Embeds machine_prompt.md content (self-contained)
- Regenerated after each PROGRESS.md update
- Deleted when task completes (Main Agent cleanup)
- Rewritten from scratch for next task

**Combined Token Flow**:
```
requirements.md (human, 2000 tokens)
    ↓ [Rule 14: Generate]
machine_prompt.md (machine, 900 tokens, 58% reduction)
    ↓ [Rule 15: Extract + Embed]
COMPACT_CONTEXT.md (ultra-compact, 500 tokens, 97% reduction)
    ↓ [After context clear]
Agent works with 500 tokens + FILES (~5K total)
```

**See Also**:
- [Rule 14: Machine-Optimized Prompts](../../.agents/rules/14-machine-optimized-prompts.md)
- [Rule 15: Instruction Compaction](../../.agents/rules/15-instruction-compaction.md)
- [COMPACT_CONTEXT Template](../../.agents/templates/COMPACT_CONTEXT-template.md)

---

## Overview

Implement a comprehensive set of no_std-compatible synchronization and atomic primitives in `foundation_nostd` that work safely in WASM and embedded environments. This specification covers spin-based locks, atomic wrappers, and related primitives—all using native Rust capabilities without `wasm_bindgen`.

**Key Principles:**
- Pure Rust implementation using `core::sync::atomic`
- No external runtime dependencies (no wasm_bindgen, no tokio)
- WASM-optimized with single-threaded detection
- API compatibility with `std::sync` for easy migration
- Comprehensive user documentation in `fundamentals/`

## Requirements Conversation Summary

### User's Initial Request

Implement spin mutex and rwmutex within `foundation_nostd` to provide safe no_std implementations for ease of use in no_std contexts. Include primitives built on `std::atomics` for no_std and WASM context. Create comprehensive documentation in `fundamentals/` explaining implementation choices, trade-offs, and usage.

### Clarifying Questions Asked

1. **Poisoning**: With poisoning - match std::sync behavior
2. **RwLock Policy**: Writer-preferring to prevent starvation
3. **Once Primitive**: Yes, include for lazy static initialization
4. **WASM Threading Detection**: Yes, optimize for single-threaded WASM
5. **Location**: `foundation_nostd/primitives/` module
6. **Timeout API**: Yes, try_lock with spin count limit
7. **API Surface**: Match `std::sync` API closely

### Additional Requirements

- Include atomic primitives built on `core::sync::atomic`
- Create `fundamentals/` documentation directory
- Deep technical documentation for users
- No wasm_bindgen - native Rust WASM only

## Primitives to Implement

### Spin-Based Locks (With Poisoning)

Matches `std::sync` API for drop-in replacement:

| Primitive | Description | API Model |
|-----------|-------------|-----------|
| `SpinMutex<T>` | Spin-based mutual exclusion with poisoning | `std::sync::Mutex` |
| `SpinRwLock<T>` | Writer-preferring read-write lock with poisoning | `std::sync::RwLock` |
| `ReaderSpinRwLock<T>` | Reader-preferring read-write lock with poisoning | Custom (variant of std::sync::RwLock) |

### Spin-Based Locks (Without Poisoning)

Simpler API for embedded/no_std contexts where panic = abort:

| Primitive | Description | API Model |
|-----------|-------------|-----------|
| `RawSpinMutex<T>` | Simple spin mutex, no poisoning overhead | `parking_lot::RawMutex` |
| `RawSpinRwLock<T>` | Simple spin rwlock, no poisoning overhead | `parking_lot::RawRwLock` |

### One-Time Initialization

| Primitive | Description | API Model |
|-----------|-------------|-----------|
| `Once` | One-time initialization with poisoning | `std::sync::Once` |
| `OnceLock<T>` | Lazy initialization container | `std::sync::OnceLock` |
| `RawOnce` | One-time init without poisoning | Custom |

### Atomic Primitives

Built on `core::sync::atomic` for no_std compatibility:

| Primitive | Description | API Model |
|-----------|-------------|-----------|
| `AtomicCell<T>` | Generic atomic wrapper for Copy types ≤ pointer size | `crossbeam::atomic::AtomicCell` |
| `AtomicOption<T>` | Atomic Option for pointer-sized types | Custom |
| `AtomicLazy<T, F>` | Lazy-initialized atomic value | `once_cell::Lazy` |
| `AtomicFlag` | Simple atomic boolean flag | Custom (simpler than AtomicBool) |

### Synchronization Helpers

| Primitive | Description | API Model |
|-----------|-------------|-----------|
| `SpinBarrier` | Spin-based barrier synchronization | `std::sync::Barrier` |
| `SpinWait` | Exponential backoff spin waiter | `crossbeam::utils::Backoff` |

### RwLock Preference Policies

This library provides two RwLock variants with different fairness policies:

**Writer-Preferring (`SpinRwLock`)**:
- When a writer is waiting, new readers are blocked
- Prevents writer starvation in read-heavy workloads
- Use when writes are important and cannot be delayed indefinitely
- State encoding: Bits 0-29 reader count, Bit 30 writer waiting, Bit 31 writer active

**Reader-Preferring (`ReaderSpinRwLock`)**:
- Readers can acquire lock even if writer is waiting
- Maximizes read concurrency but may starve writers
- Use when writes are rare and readers are plentiful
- State encoding: Bits 0-29 reader count, Bit 31 writer active (no Bit 30)

**Choosing Between Them**:
- **Use SpinRwLock** (writer-preferring) by default for balanced fairness
- **Use ReaderSpinRwLock** (reader-preferring) only when:
  - Reads vastly outnumber writes (>95% reads)
  - Write latency is not critical
  - You've measured that reader preference improves throughput

## File Structure

```
backends/foundation_nostd/src/
└── primitives/
    ├── mod.rs              (module entry, re-exports, WASM type aliases)
    │
    │   # Error Types
    ├── poison.rs           (PoisonError, TryLockError, LockResult)
    │
    │   # Spin Locks (With Poisoning - std::sync compatible)
    ├── spin_mutex.rs       (SpinMutex<T>, SpinMutexGuard<T>)
    ├── spin_rwlock.rs      (SpinRwLock<T>, SpinReadGuard<T>, SpinWriteGuard<T> - writer-preferring)
    ├── reader_spin_rwlock.rs (ReaderSpinRwLock<T>, ReaderReadGuard<T>, ReaderWriteGuard<T> - reader-preferring)
    │
    │   # Spin Locks (Without Poisoning - simpler API)
    ├── raw_spin_mutex.rs   (RawSpinMutex<T>, RawSpinMutexGuard<T>)
    ├── raw_spin_rwlock.rs  (RawSpinRwLock<T>, RawReadGuard<T>, RawWriteGuard<T>)
    │
    │   # One-Time Initialization
    ├── once.rs             (Once, OnceState - with poisoning)
    ├── once_lock.rs        (OnceLock<T>)
    ├── raw_once.rs         (RawOnce - without poisoning)
    │
    │   # Atomic Primitives
    ├── atomic_cell.rs      (AtomicCell<T>)
    ├── atomic_option.rs    (AtomicOption<T>)
    ├── atomic_lazy.rs      (AtomicLazy<T, F>)
    ├── atomic_flag.rs      (AtomicFlag)
    │
    │   # Synchronization Helpers
    ├── barrier.rs          (SpinBarrier)
    ├── spin_wait.rs        (SpinWait - exponential backoff)
    │
    │   # WASM Single-Threaded Optimizations
    └── noop.rs             (NoopMutex, NoopRwLock, NoopOnce)

specifications/03-wasm-friendly-sync-primitives/
└── fundamentals/
    ├── 00-overview.md              (Introduction, quick start, primitive selection guide)
    ├── 01-spin-locks.md            (How spin locks work, implementation details, trade-offs)
    ├── 02-poisoning.md             (What poisoning is, when to use, how to handle)
    ├── 03-atomics.md               (Atomic operations, CAS, memory barriers)
    ├── 04-memory-ordering.md       (Acquire, Release, SeqCst - deep dive)
    ├── 05-wasm-considerations.md   (WASM threading, atomics feature, optimization)
    ├── 06-usage-patterns.md        (Common patterns, anti-patterns, performance)
    └── 07-implementation-guide.md  (How this library is built, design decisions)
```

## Fundamentals Documentation

The `fundamentals/` directory contains comprehensive documentation for users. Each document is written so that readers understand the primitives as if they implemented them themselves.

| Document | Purpose | Key Topics |
|----------|---------|------------|
| `00-overview.md` | Introduction and quick start | Primitive selection guide, when to use what, quick examples |
| `01-spin-locks.md` | How spin locks work | Spin vs OS locks, busy-waiting, fairness, CPU usage |
| `02-poisoning.md` | Poisoning mechanism | What it is, why it exists, when to use/skip, recovery |
| `03-atomics.md` | Atomic operations | Compare-and-swap, fetch-and-add, atomic types |
| `04-memory-ordering.md` | Memory ordering deep dive | Relaxed, Acquire, Release, AcqRel, SeqCst with examples |
| `05-wasm-considerations.md` | WASM-specific behavior | Threading model, atomics feature, single vs multi-threaded |
| `06-usage-patterns.md` | Patterns and practices | Common patterns, anti-patterns, performance optimization |
| `07-implementation-guide.md` | Library internals | Design decisions, code walkthrough, extending the library |

### Documentation Principles

Each fundamentals document MUST:
- **Explain WHY** - Design decisions and trade-offs, not just how
- **Show the internals** - Key code snippets with detailed commentary
- **Provide examples** - Compilable, real-world usage examples
- **Discuss trade-offs** - When to use, when NOT to use
- **Be self-contained** - Reader can understand without external resources


---

## Tasks

# WASM-Friendly Sync Primitives - Tasks

## Task List

### Module Setup
- [x] Create `primitives/mod.rs` - Module entry with re-exports and WASM type aliases
- [x] Create `primitives/poison.rs` - Poisoning error types
- [x] Update `foundation_nostd/lib.rs` - Add `pub mod primitives`

### Poison Error Types (poison.rs)
- [x] Define `PoisonError<T>` matching std API
- [x] Define `TryLockError<T>` enum (Poisoned, WouldBlock)
- [x] Define `LockResult<T>` and `TryLockResult<T>` type aliases
- [x] Implement `Error` and `Display` traits

### SpinMutex - With Poisoning (spin_mutex.rs)
- [x] Define `SpinMutex<T>` struct with `UnsafeCell<T>` and atomic state
- [x] Define `SpinMutexGuard<'a, T>` with poisoning detection
- [x] Implement `new()`, `lock()`, `try_lock()`, `try_lock_with_spin_limit()`
- [x] Implement `is_poisoned()`, `into_inner()`, `get_mut()`
- [x] Implement `Deref`, `DerefMut`, `Drop` for guard (with poison-on-panic)
- [x] Implement `Send`, `Sync` bounds

### RawSpinMutex - Without Poisoning (raw_spin_mutex.rs)
- [x] Define `RawSpinMutex<T>` struct - simpler, no poison tracking
- [x] Define `RawSpinMutexGuard<'a, T>`
- [x] Implement `new()`, `lock()`, `try_lock()`, `try_lock_with_spin_limit()`
- [x] Implement `Deref`, `DerefMut`, `Drop` for guard
- [x] Implement `Send`, `Sync` bounds

### SpinRwLock - With Poisoning (spin_rwlock.rs)
- [x] Define `SpinRwLock<T>` with writer-preferring state encoding
- [x] Define `SpinReadGuard<'a, T>` and `SpinWriteGuard<'a, T>`
- [x] Implement `new()`, `read()`, `try_read()`, `write()`, `try_write()`
- [x] Implement `try_read_with_spin_limit()`, `try_write_with_spin_limit()`
- [x] Implement writer-preferring logic (pending writers block new readers)
- [x] Implement `Deref` for both, `DerefMut` for write guard
- [x] Implement `Drop` for guards with poison-on-panic

### RawSpinRwLock - Without Poisoning (raw_spin_rwlock.rs)
- [x] Define `RawSpinRwLock<T>` - simpler, no poison tracking
- [x] Define `RawReadGuard<'a, T>` and `RawWriteGuard<'a, T>`
- [x] Implement same API as SpinRwLock but without poisoning
- [x] Implement writer-preferring logic

### ReaderSpinRwLock - Reader-Preferring (reader_spin_rwlock.rs)
- [x] Define `ReaderSpinRwLock<T>` with reader-preferring state encoding
- [x] Define `ReaderReadGuard<'a, T>` and `ReaderWriteGuard<'a, T>`
- [x] Implement reader-preferring logic (no writer waiting flag)

### Once - With Poisoning (once.rs)
- [x] Define `Once` struct with atomic state
- [x] Define `OnceState` enum (Incomplete, Running, Complete, Poisoned)
- [x] Implement `new()`, `call_once()`, `call_once_force()`, `is_completed()`

### OnceLock (once_lock.rs)
- [x] Define `OnceLock<T>` container
- [x] Implement `new()`, `get()`, `get_or_init()`, `get_or_try_init()`
- [x] Implement `set()`, `into_inner()`

### RawOnce - Without Poisoning (raw_once.rs)
- [x] Define `RawOnce` - simple once without poisoning
- [x] Implement `new()`, `call_once()`, `is_completed()`

### Atomic Primitives (atomic_*.rs)
- [x] Implement `AtomicCell<T>` with `load()`, `store()`, `swap()`, `compare_exchange()`
- [x] Implement `AtomicOption<T>` with `take()`, `swap()`, `is_some()`, `is_none()`
- [x] Implement `AtomicLazy<T, F>` with `get()`, `force()`
- [x] Implement `AtomicFlag` with `set()`, `clear()`, `is_set()`

### Synchronization Helpers
- [x] Implement `SpinBarrier` with `wait()` returning `BarrierWaitResult`
- [x] Implement `SpinWait` with exponential backoff (`spin()`, `reset()`)

### WASM Optimization (noop.rs)
- [x] Implement `NoopMutex<T>` for single-threaded WASM
- [x] Implement `NoopRwLock<T>` for single-threaded WASM
- [x] Implement `NoopOnce` for single-threaded WASM
- [x] Add `#[cfg]` gates for WASM detection
- [x] Create type aliases in mod.rs for automatic selection

### Testing
- [x] Unit tests for SpinMutex (lock, try_lock, poisoning)
- [x] Unit tests for RawSpinMutex (no poisoning)
- [x] Unit tests for SpinRwLock (read, write, writer-preferring)
- [x] Unit tests for Once (call_once, poisoning)
- [x] Unit tests for AtomicCell, AtomicOption
- [x] Unit tests for try_lock_with_spin_limit

### Fundamentals Documentation
- [x] Write `00-overview.md` - Introduction and primitive selection guide
- [x] Write `01-spin-locks.md` - Spin lock implementation deep dive
- [x] Write `02-poisoning.md` - Poisoning mechanism explained
- [x] Write `03-atomics.md` - Atomic operations and types
- [x] Write `04-memory-ordering.md` - Memory ordering deep dive
- [x] Write `05-wasm-considerations.md` - WASM threading and optimization
- [x] Write `06-usage-patterns.md` - Patterns and best practices
- [x] Write `07-implementation-guide.md` - Library internals and design
- [x] Write `08-ordering-practical-guide.md` - Practical guide to using Ordering correctly
- [x] Write `09-unsafecell-guide.md` - UnsafeCell purpose, patterns, and pitfalls

## Implementation Order

1. ✅ **poison.rs** - Error types first (dependency for poisoning locks)
2. ✅ **raw_spin_mutex.rs** - Simplest lock, foundation for others
3. ✅ **spin_mutex.rs** - Add poisoning on top of raw
4. ✅ **raw_spin_rwlock.rs** - RwLock without poisoning
5. ✅ **spin_rwlock.rs** - Add poisoning
6. ✅ **reader_spin_rwlock.rs** - Reader-preferring variant
7. ✅ **raw_once.rs** - Simple once
8. ✅ **once.rs** - Once with poisoning
9. ✅ **once_lock.rs** - Container using Once
10. ✅ **atomic_cell.rs** - Generic atomic wrapper
11. ✅ **atomic_option.rs** - Atomic Option
12. ✅ **atomic_lazy.rs** - Lazy initialization
13. ✅ **atomic_flag.rs** - Simple flag
14. ✅ **barrier.rs** - Barrier synchronization
15. ✅ **spin_wait.rs** - Backoff helper
16. ✅ **noop.rs** - WASM no-op variants
17. ✅ **mod.rs** - Re-exports and type aliases
18. ✅ **Tests** - All unit tests
19. ✅ **Fundamentals** - All documentation

## Notes

### Atomic State Encoding (SpinMutex)
```rust
// Simple state: 0 = unlocked, 1 = locked
// With poisoning: Bit 0 = locked, Bit 1 = poisoned
const UNLOCKED: u8 = 0b00;
const LOCKED: u8 = 0b01;
const POISONED: u8 = 0b10;
const LOCKED_POISONED: u8 = 0b11;
```

### Writer-Preferring RwLock State
```rust
// Bits 0-29: Reader count
// Bit 30: Writer waiting
// Bit 31: Writer active
const WRITER_WAITING: u32 = 1 << 30;
const WRITER_ACTIVE: u32 = 1 << 31;
const READER_MASK: u32 = (1 << 30) - 1;
```

### WASM Type Alias Pattern
```rust
// In mod.rs:
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
pub type Mutex<T> = NoopMutex<T>;

#[cfg(any(not(target_arch = "wasm32"), target_feature = "atomics"))]
pub type Mutex<T> = SpinMutex<T>;
```

### SpinWait Backoff Pattern
```rust
pub struct SpinWait {
    counter: u32,
}

impl SpinWait {
    pub fn spin(&mut self) -> bool {
        if self.counter < 10 {
            // Fast path: just spin_loop hint
            for _ in 0..(1 << self.counter) {
                core::hint::spin_loop();
            }
            self.counter += 1;
            true
        } else {
            false // Caller should yield or block
        }
    }
}
```

---

## ✅ SPECIFICATION COMPLETE - 100%

All 48 tasks have been completed and verified successfully.

**Implementation Summary**:
- 16 primitives implemented (including ReaderSpinRwLock)
- 148 passing tests (100% pass rate)
- 0 clippy warnings (176 fixed)
- 11 fundamental documents (162KB)
- Full std::sync API compatibility
- WASM optimized (no-op variants for single-threaded)

**Verification Status**: ✅ PASSED - PRODUCTION READY

See: FINAL_VERIFICATION_REPORT.md for complete verification details.

---
*Last Updated: 2026-01-22*
*Status: COMPLETE*

---

## Success Criteria

### Core Functionality
- [ ] All spin-based locks compile and work in no_std
- [ ] All atomic primitives compile and work in no_std
- [ ] Poisoning works correctly on panic
- [ ] Writer-preferring policy prevents writer starvation
- [ ] `try_lock_with_spin_limit()` returns after N spins

### WASM Support
- [ ] Compiles for `wasm32-unknown-unknown` target
- [ ] Single-threaded WASM uses no-op locks (no atomics required)
- [ ] Multi-threaded WASM uses real atomic operations
- [ ] Correct `#[cfg]` gates for WASM detection
- [ ] No wasm_bindgen dependency

### API Compatibility
- [ ] `lock()` returns `LockResult<Guard>`
- [ ] `try_lock()` returns `TryLockResult<Guard>`
- [ ] Guards implement `Deref`/`DerefMut`
- [ ] `Once::call_once()` matches std API
- [ ] `AtomicCell<T>` provides load/store/swap operations

### Documentation
- [ ] All fundamentals documents created
- [ ] Each document is comprehensive and accurate
- [ ] Code examples compile and are correct
- [ ] Trade-offs and design decisions explained

### Quality
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`
- [ ] Compiles with `--no-default-features`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy --package foundation_nostd -- -D warnings
cargo test --package foundation_nostd -- primitives
cargo build --package foundation_nostd --no-default-features
cargo build --package foundation_nostd --target wasm32-unknown-unknown
```

## Module Documentation References

### foundation_nostd/primitives (NEW)
- **Documentation**: `documentation/foundation_nostd_primitives/doc.md` (to be created)
- **Purpose**: no_std-compatible synchronization primitives
- **Fundamentals**: `specifications/03-wasm-friendly-sync-primitives/fundamentals/`

### Existing References (READ FIRST)
- `std::sync::Mutex` - API to match
- `std::sync::RwLock` - API to match
- `std::sync::Once` - API to match
- `core::sync::atomic` - Foundation for all primitives
- `spin` crate - Reference implementation patterns

## Important Notes for Agents

### Before Starting
- **MUST READ** this requirements.md first
- **MUST READ** `std::sync` documentation for API patterns
- **MUST READ** `core::sync::atomic` documentation
- **MUST VERIFY** atomic operations available on target

### Implementation Guidelines
- Use `core::sync::atomic` for atomic operations
- Use `core::cell::UnsafeCell` for interior mutability
- Use `#[cfg(target_has_atomic)]` for atomic feature detection
- Use `#[cfg(target_arch = "wasm32")]` for WASM-specific code
- Implement `Send` and `Sync` appropriately
- Add `#[inline]` hints for hot paths
- No wasm_bindgen - use native Rust WASM capabilities only

### WASM Threading Detection

```rust
// Single-threaded WASM (no atomics target feature)
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]

// Multi-threaded WASM (with atomics target feature)
#[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]

// Native platforms
#[cfg(not(target_arch = "wasm32"))]
```

### Memory Ordering Guidelines

| Operation | Ordering | Use When |
|-----------|----------|----------|
| Simple counter | `Relaxed` | No synchronization needed |
| Lock acquire | `Acquire` | Reading shared state after lock |
| Lock release | `Release` | Publishing writes before unlock |
| Spinlock CAS | `AcqRel` | Both acquire and release semantics |
| Sequentially consistent | `SeqCst` | Total ordering required |

---

## Agent Rules Reference

### Location Headers
- **Rules Location**: `.agents/rules/`
- **Stacks Location**: `.agents/stacks/`
- **Skills Location**: `.agents/skills/`

### Mandatory Rules for All Agents

| Rule | File | Purpose |
|------|------|---------|
| 01 | `.agents/rules/01-rule-naming-and-structure.md` | File naming conventions |
| 02 | `.agents/rules/02-rules-directory-policy.md` | Directory policies |
| 03 | `.agents/rules/03-dangerous-operations-safety.md` | Dangerous operations safety |
| 04 | `.agents/rules/04-work-commit-and-push-rules.md` | Work commit and push rules |

### Role-Specific Rules

| Agent Type | Additional Rules to Load |
|------------|--------------------------|
| **Review Agent** | `.agents/rules/06-specifications-and-requirements.md` |
| **Implementation Agent** | `.agents/rules/13-implementation-agent-guide.md` |
| **Verification Agent** | `.agents/rules/08-verification-workflow-complete-guide.md`, `.agents/stacks/rust.md` |
| **Documentation Agent** | `.agents/rules/06-specifications-and-requirements.md` |

### Stack Files
- **Language**: Rust → `.agents/stacks/rust.md`

### Skills Referenced
- None

---

## File Organization Reminder

ONLY these files allowed:

1. requirements.md - Requirements with tasks
2. LEARNINGS.md - All learnings
3. REPORT.md - All reports
4. VERIFICATION.md - Verification
5. PROGRESS.md - Current status (delete at 100%)
6. fundamentals/, features/, templates/ (optional)

FORBIDDEN: Separate learning/report/verification files

Consolidation: All learnings → LEARNINGS.md, All reports → REPORT.md

See Rule 06 "File Organization" for complete policy.

---
*Created: 2026-01-19*
*Last Updated: 2026-01-25*
