---
description: "Migrate Stream/StreamIterator/StreamRecvIterator from mpp to valtron, add ConcurrentQueueStreamIterator with configurable max_turns polling optimization"
status: "completed"
priority: "high"
created: 2026-04-04
updated: 2026-04-04
author: "Main Agent"
metadata:
  version: "1.2"
  estimated_effort: "large"
  tags: [valtron, stream-iterator, concurrent-queue, optimization, migration]
has_features: true
features:
  completed: 2
  uncompleted: 0
  total: 2
---

# Overview

This specification defines the migration of `Stream` enum, `StreamIterator` trait, and `StreamRecvIterator` from `synca::mpp` to the `valtron` module, along with a new optimized `ConcurrentQueueStreamIterator` type with configurable polling behavior.

## The Problem

### Stream/StreamIterator/StreamRecvIterator in Wrong Module

Currently, the `Stream<D, P>` enum, `StreamIterator` trait, and `StreamRecvIterator<D, P>` are defined in `backends/foundation_core/src/synca/mpp.rs`. However:

1. **Only used in valtron** - The `Stream` enum (`Init`, `Ignore`, `Delayed`, `Pending`, `Next`) is exclusively used by valtron executors and stream combinators
2. **Conceptual mismatch** - `synca::mpp` is for message-passing primitives (channels, senders, receivers), not stream processing abstractions
3. **Circular dependency risk** - Valtron code must import from `synca::mpp` for core types, creating unclear module boundaries
4. **Discoverability** - Developers looking for valtron stream types search in `valtron/`, not `synca::mpp`
5. **Type cohesion** - `StreamRecvIterator` wraps `RecvIterator<Stream<D, P>>` and is tightly coupled to the `Stream` type; it should move with `Stream` to maintain module coherence

### No Optimized ConcurrentQueue Iterator

Currently, iterating over a `ConcurrentQueue<Stream<D, P>>` requires:

1. Using `RecvIterator` which blocks with `park_timeout()` for the entire duration
2. No way to balance between responsiveness (checking other tasks) and throughput (busy polling)
3. No `max_turns` configuration - either block indefinitely or poll once

This causes issues in multi-task valtron scenarios where:
- A task holding a long-blocking iterator starves other tasks
- No graceful yielding mechanism after N unsuccessful polls

## Goals

1. **Move `Stream` enum to valtron** - Relocate `Stream<D, P>` from `synca::mpp` to `valtron::streams`
2. **Move `StreamIterator` trait to valtron** - Relocate trait definition to `valtron::streams`
3. **Move `StreamRecvIterator` to valtron** - Relocate `StreamRecvIterator<D, P>` from `synca::mpp` to `valtron::streams`
4. **Remove types from `synca::mpp`** - Delete `Stream`, `StreamIterator`, `StreamRecvIterator` from mpp.rs (no longer needed there)
5. **Implement `ConcurrentQueueStreamIterator`** - New iterator with `max_turns` configuration
6. **Update all imports** - Fix existing code that imports from `synca::mpp`

## Key Design Principles

### max_turns Balances Responsiveness vs Throughput

The `max_turns` parameter controls how many times the iterator polls the queue before yielding:

```
max_turns = 1        → High responsiveness, lower throughput
                         (yield after 1 failed poll, check other tasks)
max_turns = 10       → Balanced (try 10 times, then yield)
max_turns = 100      → High throughput, lower responsiveness
                         (busy poll 100 times before yielding)
max_turns = usize::MAX → Maximum throughput (essentially blocking)
```

### Yield Mechanism

After `max_turns` unsuccessful polls:
- Return `Stream::Ignore` to let executor check other tasks
- OR use `std::thread::park_timeout()` for short sleep before yielding
- The executor interprets `Ignore` as "continue but check other tasks"

## Feature Index

| # | Feature | Description | Dependencies | Status |
|---|---------|-------------|--------------|--------|
| 1 | [stream-migration](./features/01-stream-migration/feature.md) | Move `Stream`, `StreamIterator`, and `StreamRecvIterator` from `synca::mpp` to `valtron::streams` + consolidate combinators | None | ✅ Completed |
| 2 | [concurrent-queue-iterator](./features/02-concurrent-queue-iterator/feature.md) | Implement `ConcurrentQueueStreamIterator` with `max_turns` polling optimization | #1 | ✅ Completed |

## High-Level Architecture

### Before (Current State)

```
synca::mpp                          valtron::*
┌─────────────────────┐            ┌─────────────────────┐
│ Stream<D, P>        │───────────►│ executors/          │
│ StreamIterator      │  (import)  │ stream_iterators.rs │
│ RecvIterator        │            │ task_iterators.rs   │
│ Sender/Receiver     │            │ drivers.rs          │
└─────────────────────┘            └─────────────────────┘
```

### After (Target State)

```
synca::mpp                          valtron::*
┌─────────────────────┐            ┌─────────────────────┐
│ Sender/Receiver     │            │ streams.rs          │
│ RecvIterator        │            │ ├── Stream          │
│                     │            │ ├── StreamIterator  │
│ (optional re-export)◄────────────│ ├── ConcurrentQueue…│
│                     │  (re-export)│ └── ...             │
└─────────────────────┘            └─────────────────────┘
         │                                       │
         │                                       │
         ▼                                       ▼
    [Message Passing]                     [Stream Processing]
```

### ConcurrentQueueStreamIterator Design

```rust
pub struct ConcurrentQueueStreamIterator<D, P> {
    chan: Arc<ConcurrentQueue<Stream<D, P>>>,
    max_turns: usize,
    park_duration: Duration,  // Configurable park timeout duration
}

impl<D, P> Iterator for ConcurrentQueueStreamIterator<D, P> {
    type Item = Stream<D, P>;
    
    fn next(&mut self) -> Option<Self::Item> {
        tracing::trace!(max_turns = self.max_turns, "starting poll cycle");
        
        for turn in 0..self.max_turns {
            match self.chan.pop() {
                Ok(value) => {
                    tracing::debug!(turn = turn, "received value from queue");
                    return Some(value);
                }
                Err(PopError::Empty) => {
                    tracing::trace!(turn = turn, "queue empty, yielding");
                    #[cfg(feature = "std")]
                    std::thread::park_timeout(self.park_duration);
                    #[cfg(not(feature = "std"))]
                    std::hint::spin_loop();
                }
                Err(PopError::Closed) => {
                    tracing::info!("queue closed, ending iteration");
                    return None;
                }
            }
        }
        
        tracing::trace!("max_turns reached, yielding Ignore");
        Some(Stream::Ignore)
    }
}
```

### Type Relationships

```
valtron::streams
├── Stream<D, P> (enum)
│   ├── Init
│   ├── Ignore
│   ├── Delayed(Duration)
│   ├── Pending(P)
│   └── Next(D)
│
├── StreamIterator (trait)
│   └── blanket impl for T: Iterator<Item = Stream<D, P>>
│
├── StreamRecvIterator<D, P> (struct)
│   ├── wraps RecvIterator<Stream<D, P>>
│   └── implements Iterator<Item = Stream<D, P>>
│
└── ConcurrentQueueStreamIterator<D, P> (struct)
    ├── new(chan: Arc<ConcurrentQueue<Stream<D, P>>>, max_turns: usize, park_duration: Duration)
    └── implements Iterator<Item = Stream<D, P>>
```

## Success Criteria

### Functionality

- [ ] `Stream` enum moved to `valtron::streams.rs`
- [ ] `StreamIterator` trait moved to `valtron::streams.rs`
- [ ] `StreamRecvIterator` moved to `valtron::streams.rs`
- [ ] `ConcurrentQueueStreamIterator` implemented with `max_turns`
- [ ] `ConcurrentQueueStreamIterator` compiles for both `std` and `no_std` targets
- [ ] All existing tests pass after migration
- [ ] New unit tests for `ConcurrentQueueStreamIterator` verify:
  - [ ] Returns items when available in queue
  - [ ] Yields `Ignore` after `max_turns` unsuccessful polls
  - [ ] Returns `None` when queue is closed and empty
  - [ ] Respects configurable `park_duration` in `std` mode
  - [ ] Uses `spin_loop()` in `no_std` mode
  - [ ] Tests are located in `backends/foundation_core/tests/stream_iterators.rs`

### Code Quality

- [ ] Zero warnings from `cargo clippy -p foundation_core -- -D warnings`
- [ ] `cargo fmt -p foundation_core -- --check` passes for all modified files
- [ ] All unit and integration tests pass
- [ ] Module documentation (`//!`) present in `valtron::streams`
- [ ] Tracing calls present: `tracing::trace!`, `tracing::debug!`, `tracing::info!` for key operations
- [ ] All tests located in `backends/foundation_core/tests/` directory

### Documentation

- [ ] `LEARNINGS.md` captures migration rationale and design decisions
- [ ] `VERIFICATION.md` produced with all verification checks passing
- [ ] `REPORT.md` created documenting final implementation

## Module References

Agents implementing features should read these files:

### Source (Current Location)

- `backends/foundation_core/src/synca/mpp.rs` - Current `Stream` enum and `StreamIterator` trait definition

### Target (New Location)

- `backends/foundation_core/src/valtron/streams.rs` - Destination for migrated types (currently empty/minimal)
- `backends/foundation_core/src/valtron/mod.rs` - Module exports to update

### Files to Update

- `backends/foundation_core/src/valtron/iterators.rs` - Currently re-exports from `synca::mpp`
- `backends/foundation_core/src/valtron/stream_iterators.rs` - Uses `Stream` and `StreamIterator`
- `backends/foundation_core/src/valtron/task_iterators.rs` - Uses `Stream` and `StreamIterator`
- `backends/foundation_core/src/valtron/executors/drivers.rs` - Uses `Stream` and `StreamIterator`
- `backends/foundation_core/src/valtron/executors/unified.rs` - Uses `Stream` and `StreamIterator`
- `backends/foundation_core/src/valtron/branches.rs` - Uses `Stream` in type signatures

### Files to Search

Search entire codebase for imports:
```bash
grep -r "synca::mpp::Stream" backends/
grep -r "synca::mpp::StreamIterator" backends/
grep -r "use.*mpp::Stream" backends/
```

---

# Features

Detailed feature specifications are in the `features/` subdirectory.

## Feature 01: Stream Migration

**File:** `features/01-stream-migration/feature.md`

Move `Stream` enum, `StreamIterator` trait, and `StreamRecvIterator` from `synca::mpp` to `valtron::streams`.

### Tasks

1. Create `valtron::streams.rs` module with `Stream` enum and `StreamIterator` trait
2. Move `StreamRecvIterator` to `valtron::streams.rs`
3. Update `valtron::mod.rs` to export new module
4. Update all valtron-internal imports
5. Add backward-compatible re-exports in `synca::mpp.rs` (optional)
6. Run tests to verify migration

## Feature 02: ConcurrentQueueStreamIterator

**File:** `features/02-concurrent-queue-iterator/feature.md`

Implement new optimized iterator for concurrent queue stream processing.

### Tasks

1. Define `ConcurrentQueueStreamIterator<D, P>` struct with configurable `park_duration`
2. Implement `Iterator` trait with `max_turns` logic and tracing calls
3. Add `std`/`no_std` conditional yielding (park_timeout vs spin_loop)
4. Implement constructor with configurable `max_turns` and `park_duration: Duration`
5. Add unit tests in `backends/foundation_core/tests/stream_iterators.rs`
6. Add integration test with valtron executor
7. Add tracing calls: `tracing::trace!`, `tracing::debug!`, `tracing::info!` for debugging

## Feature 03: Import Updates

**File:** `features/03-import-updates/feature.md`

Update all imports across the codebase and ensure clean compilation.

### Tasks

1. Search for all `synca::mpp::Stream` imports
2. Search for all `synca::mpp::StreamIterator` imports
3. Search for all `synca::mpp::StreamRecvIterator` imports
4. Update imports to `valtron::streams`
5. Remove or update re-exports in `valtron::iterators.rs`
6. Verify all code compiles
7. Run full test suite

---

_Created: 2026-04-04_
_Status: ⏳ Pending_
_Structure: Feature-based (has_features: true, 3 features)_
