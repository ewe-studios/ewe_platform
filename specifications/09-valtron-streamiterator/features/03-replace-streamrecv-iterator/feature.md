---
description: "Replace all usages of StreamRecvIterator with ConcurrentQueueStreamIterator as the default iterator for valtron stream processing"
status: "pending"
priority: "high"
created: 2026-04-04
updated: 2026-04-04
author: "Main Agent"
feature_number: 3
depends_on: ["01-stream-migration", "02-concurrent-queue-iterator"]
metadata:
  estimated_effort: "medium"
  files_modified:
    - backends/foundation_core/src/valtron/executors/drivers.rs
    - backends/foundation_core/src/valtron/executors/unified.rs
    - backends/foundation_core/src/valtron/executors/threads.rs
    - backends/foundation_core/src/valtron/executors/builders.rs
    - backends/foundation_core/src/valtron/streams.rs
  impact: "high"
  risk: "medium"
---

# Feature 03: Replace StreamRecvIterator with ConcurrentQueueStreamIterator

## Summary

This feature replaces `StreamRecvIterator` with `ConcurrentQueueStreamIterator` as the default iterator for valtron stream processing. The optimized `ConcurrentQueueStreamIterator` with configurable `max_turns` provides better responsiveness in multi-task scenarios by yielding control after N unsuccessful polls, allowing the executor to check other tasks.

### Rationale

The current `StreamRecvIterator` wraps a blocking `RecvIterator` that uses `park_timeout()` for the entire duration of iteration. This causes issues in multi-task valtron scenarios where:

1. **Task starvation** - A task holding a long-blocking iterator starves other tasks
2. **No graceful yielding** - No mechanism to yield after N unsuccessful polls
3. **Inflexible polling** - Either block indefinitely or poll once, no middle ground

The `ConcurrentQueueStreamIterator` solves these issues with:
- **Configurable `max_turns`** - Controls how many poll attempts before yielding
- **Responsive scheduling** - Returns `Stream::Ignore` to let executor check other tasks
- **Tunable behavior** - Balance responsiveness vs throughput per use case

## Changes Required

### 1. Update `execute()` to Return `ConcurrentQueueStreamIterator`

**File:** `backends/foundation_core/src/valtron/executors/unified.rs`

The `execute()` function currently returns `DrivenStreamIterator<T>` which wraps `StreamRecvIterator`. Update to use `ConcurrentQueueStreamIterator`:

```rust
// Current signature (returns StreamRecvIterator-based DrivenStreamIterator)
pub fn execute<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<DrivenStreamIterator<T>>

// Remains same signature, but internal implementation changes
// DrivenStreamIterator now wraps ConcurrentQueueStreamIterator instead of StreamRecvIterator
```

**Changes:**
- Update `execute_single_stream()` to create `ConcurrentQueueStreamIterator` instead of `StreamRecvIterator`
- Update `execute_multi_stream()` to create `ConcurrentQueueStreamIterator` instead of `StreamRecvIterator`
- Update `drive_stream()` helper to accept `ConcurrentQueueStreamIterator`

### 2. Update Driver Functions

**File:** `backends/foundation_core/src/valtron/executors/drivers.rs`

Update the `drive_stream()` function and related helpers:

```rust
// Current
pub fn drive_stream<T>(
    incoming: StreamRecvIterator<T::Ready, T::Pending>,
) -> DrivenStreamIterator<T>

// Updated
pub fn drive_stream<T>(
    incoming: ConcurrentQueueStreamIterator<T::Ready, T::Pending>,
) -> DrivenStreamIterator<T>
```

Update `run_until_stream_has_value()` to work with `ConcurrentQueueStreamIterator`:

```rust
// Current
pub fn run_until_stream_has_value<T, S>(
    stream: StreamRecvIterator<T::Ready, T::Pending>,
    checker: S,
) -> StreamRecvIterator<T::Ready, T::Pending>

// Updated
pub fn run_until_stream_has_value<T, S>(
    stream: ConcurrentQueueStreamIterator<T::Ready, T::Pending>,
    checker: S,
) -> ConcurrentQueueStreamIterator<T::Ready, T::Pending>
```

### 3. Update Builder Methods

**File:** `backends/foundation_core/src/valtron/executors/builders.rs`

Update methods that return `StreamRecvIterator` to return `ConcurrentQueueStreamIterator`:

- `scheduled_stream_iter()` - Line ~116
- `stream_lift_iter()` - Line ~227
- `stream_sequenced_iter()` - Line ~304
- `stream_broadcast_iter()` - Line ~390

```rust
// Current
pub fn scheduled_stream_iter(
    self,
    wait_cycle: time::Duration,
) -> AnyResult<StreamRecvIterator<Done, Pending>, ExecutorError> {
    // ...
    Ok(StreamRecvIterator::new(RecvIterator::from_chan(iter_chan, wait_cycle)))
}

// Updated
pub fn scheduled_stream_iter(
    self,
    wait_cycle: time::Duration,
    max_turns: Option<usize>,  // NEW: configurable max_turns
) -> AnyResult<ConcurrentQueueStreamIterator<Done, Pending>, ExecutorError> {
    // ...
    Ok(ConcurrentQueueStreamIterator::new(
        iter_chan,
        max_turns.unwrap_or(10),  // Default: 10 turns
        Duration::from_nanos(20)
    ))
}
```

### 4. Update threads.rs Executor

**File:** `backends/foundation_core/src/valtron/executors/threads.rs`

Update the `stream_iter()` method in the spawn builder:

```rust
// Current
pub fn stream_iter(
    self,
    wait_cycle: time::Duration,
) -> AnyResult<StreamRecvIterator<Done, Pending>, ExecutorError> {
    // ...
    Ok(StreamRecvIterator::new(RecvIterator::from_chan(iter_chan, wait_cycle)))
}

// Updated
pub fn stream_iter(
    self,
    wait_cycle: time::Duration,
    max_turns: Option<usize>,
) -> AnyResult<ConcurrentQueueStreamIterator<Done, Pending>, ExecutorError> {
    // ...
    Ok(ConcurrentQueueStreamIterator::new(
        iter_chan,
        max_turns.unwrap_or(10),
        Duration::from_nanos(20)
    ))
}
```

### 5. Update `DrivenStreamIterator` Wrapper

**File:** `backends/foundation_core/src/valtron/executors/drivers.rs`

Update `DrivenStreamIterator` to wrap `ConcurrentQueueStreamIterator`:

```rust
// Current
pub struct DrivenStreamIterator<T>(Option<StreamRecvIterator<T::Ready, T::Pending>>)

// Updated
pub struct DrivenStreamIterator<T>(Option<ConcurrentQueueStreamIterator<T::Ready, T::Pending>>)
```

Update `DrivenNonSendStreamIterator` similarly for non-Send types.

### 6. Deprecate `StreamRecvIterator`

**File:** `backends/foundation_core/src/valtron/streams.rs`

Add deprecation notice to `StreamRecvIterator`:

```rust
/// [`StreamRecvIterator<D, P>`] wraps a [`RecvIterator<Stream<D, P>>`] to provide
/// stream iteration over channel-based streams.
///
/// # Deprecation
///
/// **Deprecated since 0.9.0**: Use [`ConcurrentQueueStreamIterator`] instead.
///
/// `StreamRecvIterator` uses blocking `park_timeout()` which can starve other tasks
/// in multi-task scenarios. `ConcurrentQueueStreamIterator` provides configurable
/// `max_turns` for better responsiveness.
///
/// # Migration
///
/// ```rust
/// // Before
/// let iter = StreamRecvIterator::new(recv_iter);
///
/// // After
/// let iter = ConcurrentQueueStreamIterator::new(chan, max_turns, park_duration);
/// ```
#[deprecated(
    since = "0.9.0",
    note = "Use ConcurrentQueueStreamIterator instead, which provides configurable max_turns for better multi-task responsiveness"
)]
pub struct StreamRecvIterator<D, P>(RecvIterator<Stream<D, P>>);
```

### 7. Update Documentation

**File:** `backends/foundation_core/src/valtron/streams.rs`

Update module-level documentation to reflect new default:

```rust
//! Stream types and traits for valtron stream processing.
//!
//! This module contains the core stream abstractions used throughout valtron:
//! - [`Stream<D, P>`] - Enum representing stream states (Init, Ignore, Delayed, Pending, Next)
//! - [`StreamIterator`] - Trait for iterators yielding Stream items
//! - [`ConcurrentQueueStreamIterator<D, P>`] - **Default** iterator with configurable max_turns
//! - [`StreamIteratorExt`] - Extension trait with stream combinators
//!
//! # Default Iterator
//!
//! The `ConcurrentQueueStreamIterator` is the default iterator for valtron stream processing.
//! It provides configurable `max_turns` behavior for balancing responsiveness against throughput.
//!
//! # Deprecated
//!
//! `StreamRecvIterator` is deprecated in favor of `ConcurrentQueueStreamIterator`.
```

### 8. Add Default max_turns Configuration

Consider adding a configuration constant or environment for default `max_turns`:

```rust
/// Default max_turns for ConcurrentQueueStreamIterator.
///
/// This value balances responsiveness (yielding to other tasks) with
/// throughput (busy polling). Adjust based on workload:
/// - Lower values (1-5): High responsiveness, lower throughput
/// - Medium values (10-20): Balanced (default)
/// - Higher values (50-100): High throughput, lower responsiveness
pub const DEFAULT_MAX_TURNS: usize = 10;
```

## Files to Modify

| File | Changes |
|------|---------|
| `backends/foundation_core/src/valtron/executors/unified.rs` | Update `execute_single_stream()`, `execute_multi_stream()` to use `ConcurrentQueueStreamIterator` |
| `backends/foundation_core/src/valtron/executors/drivers.rs` | Update `drive_stream()`, `run_until_stream_has_value()`, `DrivenStreamIterator` struct |
| `backends/foundation_core/src/valtron/executors/builders.rs` | Update `scheduled_stream_iter()`, `stream_lift_iter()`, `stream_sequenced_iter()`, `stream_broadcast_iter()` |
| `backends/foundation_core/src/valtron/executors/threads.rs` | Update `stream_iter()` method |
| `backends/foundation_core/src/valtron/streams.rs` | Add deprecation to `StreamRecvIterator`, update module docs, add `DEFAULT_MAX_TURNS` |
| `backends/foundation_core/src/valtron/mod.rs` | Update re-exports, add deprecation warnings |

## Success Criteria

### Code Changes

- [ ] `execute()` returns `DrivenStreamIterator` wrapping `ConcurrentQueueStreamIterator`
- [ ] All builder methods (`scheduled_stream_iter`, `stream_lift_iter`, etc.) return `ConcurrentQueueStreamIterator`
- [ ] `drive_stream()` accepts `ConcurrentQueueStreamIterator`
- [ ] `run_until_stream_has_value()` works with `ConcurrentQueueStreamIterator`
- [ ] `StreamRecvIterator` marked as deprecated with clear migration path
- [ ] Module documentation updated to reflect new default

### Compilation

- [ ] `cargo check -p foundation_core` passes
- [ ] `cargo clippy -p foundation_core -- -D warnings` passes
- [ ] `cargo fmt -p foundation_core -- --check` passes

### Testing

- [ ] All existing valtron tests pass
- [ ] New tests verify `ConcurrentQueueStreamIterator` is used by default
- [ ] Integration tests verify multi-task responsiveness
- [ ] Performance tests show no regression (or document expected changes)

### Documentation

- [ ] Deprecation notice includes migration example
- [ ] Module docs explain `max_turns` configuration
- [ ] API documentation builds without warnings

## Usage Example

### Before (StreamRecvIterator)

```rust
use foundation_core::valtron::{execute, StreamRecvIterator};

let task = MyTask::new();
let result = execute(task, None)?;

for item in result {
    match item {
        Stream::Next(value) => println!("Got: {}", value),
        Stream::Ignore => continue,  // Rarely seen with blocking iterator
        _ => {}
    }
}
```

### After (ConcurrentQueueStreamIterator with defaults)

```rust
use foundation_core::valtron::execute;

let task = MyTask::new();
let result = execute(task, None)?;

// Same usage, but now uses ConcurrentQueueStreamIterator internally
// with max_turns=10 by default
for item in result {
    match item {
        Stream::Next(value) => println!("Got: {}", value),
        Stream::Ignore => {
            // Now visible! Iterator yields after 10 unsuccessful polls
            // allowing executor to check other tasks
            continue;
        }
        _ => {}
    }
}
```

### Configuring max_turns (Future Enhancement)

```rust
use foundation_core::valtron::{execute_with_config, StreamConfig};

let task = MyTask::new();

// High responsiveness (yield frequently)
let config = StreamConfig::default().with_max_turns(3);
let result = execute_with_config(task, config)?;

// High throughput (busy poll longer)
let config = StreamConfig::default().with_max_turns(100);
let result = execute_with_config(task, config)?;
```

## Trade-offs and Considerations

### Performance Impact

| Scenario | StreamRecvIterator | ConcurrentQueueStreamIterator (max_turns=10) |
|----------|-------------------|---------------------------------------------|
| Single task, queue always has items | Same | Same |
| Single task, queue empty | Blocks efficiently | Yields after 10 polls (slight overhead) |
| Multi-task, mixed load | One task can starve others | Fair scheduling, all tasks get CPU time |
| High-throughput batch processing | Optimal | May be slower (tune max_turns higher) |

### Migration Risks

1. **Behavior change** - Code expecting blocking behavior may see `Stream::Ignore` more frequently
2. **Performance regression** - High-throughput scenarios may need higher `max_turns`
3. **Breaking change** - Returning different iterator type requires signature changes

### Mitigation

- Keep `StreamRecvIterator` available (deprecated) for edge cases
- Provide clear migration guide with examples
- Add `max_turns` configuration for tuning
- Document expected behavior changes

## Implementation Order

1. **Phase 1: Preparation**
   - Add `DEFAULT_MAX_TURNS` constant
   - Add deprecation to `StreamRecvIterator`
   - Update module documentation

2. **Phase 2: Core Changes**
   - Update `execute_single_stream()` and `execute_multi_stream()`
   - Update `drive_stream()` and `DrivenStreamIterator`
   - Update builder methods

3. **Phase 3: Testing**
   - Run existing tests, fix failures
   - Add new tests for `ConcurrentQueueStreamIterator` usage
   - Performance benchmarking

4. **Phase 4: Cleanup**
   - Remove any unused `StreamRecvIterator` code paths
   - Final documentation pass
   - Deprecation audit

---

_Feature 03 of 04 | Part of specification 09-valtron-streamiterator_
