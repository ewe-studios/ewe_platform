---
feature_name: "Valtron Integration"
description: "VfsTask — valtron task that consumes ObservableFs events via Broadcaster, integrating with spec-34 file watcher infrastructure for multi-subscriber event consumption and reactive workflows."
status: "pending"
priority: "medium"
phase: 5
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
  - "15-observable-fs"
tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

# Feature 10: Valtron Integration

## Overview

Bridges ObservableFs with the valtron task execution model. VfsTask is a valtron task that consumes the event stream from ObservableFs and makes it available through the valtron task infrastructure. Follows the same pattern as `FileWatcherTask` from spec-34.

ObservableFs (feature 15) handles event emission as a separate concern. This feature wires those events into valtron for reactive workflows (incremental builds, live reload, change-driven pipelines).

### How VfsTask Polls the Broadcaster Channel

ObservableFs uses `EventBroadcaster<VfsEvent>` (from spec-34) internally. VfsTask subscribes to the broadcaster and receives a `Receiver<VfsEvent>` (from `foundation_core::synca::mpp`). On each `next_status()` call:

1. VfsTask calls `self.receiver.try_recv()` (non-blocking).
2. If an event is available: broadcast to VfsTask's own subscribers (if any), return `TaskStatus::Ready(event)`.
3. If no event is available: return `TaskStatus::Depends(self.readiness.clone())` to park the task until the channel has data.

This mirrors `FileWatcherTask` from spec-34 exactly -- the only difference is the event source (mpp channel instead of NativeWatcher::poll).

### EventReadiness Implementation

VfsTask needs an `EventReadiness` impl so the valtron executor knows when to wake it. Since the event source is an mpp `Receiver<VfsEvent>` (backed by `concurrent_queue`), readiness is determined by whether the channel has pending items:

```rust
pub struct VfsEventReadiness {
    receiver: Receiver<VfsEvent>,
}

impl EventReadiness for VfsEventReadiness {
    fn is_ready(&self, _timeout: Option<Duration>) -> bool {
        // Non-blocking check: is there at least one event in the channel?
        !self.receiver.is_empty()
    }
}
```

This is an event-driven readiness model (not polling-based), so `Depends` is correct -- the executor parks the task and re-checks readiness when other tasks yield. Since mpp channels are bounded, events queue up and readiness flips to true as soon as ObservableFs emits an event.

**Important**: The `Receiver` must be shared between `VfsEventReadiness` (for readiness checks via `is_empty()`) and `VfsTask` (for actually consuming events via `try_recv()`). Use `Arc<Receiver<VfsEvent>>` or clone the receiver if the mpp channel supports it. If not, store the receiver in a shared wrapper:

```rust
pub struct VfsTask {
    shared_rx: Arc<SharedVfsReceiver>,
    stop: StopSignal,
    subscribers: EventBroadcaster<VfsEvent>,
}

struct SharedVfsReceiver {
    rx: Receiver<VfsEvent>,
    /// Events consumed from rx but not yet returned by next_status
    buffer: Mutex<VecDeque<VfsEvent>>,
}
```

### Composition with the Valtron Executor

VfsTask is spawned as a standard `TaskIterator` via `execute()`:

```rust
let observable_fs = ObservableFs::new(overlay);
let mut vfs_task = VfsTask::from_observable(&observable_fs);
let (tx, rx) = vfs_task.subscribe();  // optional: downstream subscribers
let stop = vfs_task.stop_signal();
let stream = execute(vfs_task, None)?;

// Consume events
for item in stream {
    if let Stream::Next(event) = item {
        println!("VFS event: {:?}", event);
    }
}
```

VfsTask is an **infinite task** (it never returns `None` from `next_status()` unless `StopSignal` is set). Same rules as FileWatcherTask from spec-34 feature 07:
- Use `collect_one()` for finite collection in tests
- Never use `collect_result()` (blocks forever)
- Use `StopSignal` for clean termination

VfsTask does NOT use `DelayedIterator` or `MultiIterator` -- it is a simple `TaskIterator` that returns `Depends` when idle. It could be composed with other tasks via `MultiIterator` by the caller if needed (e.g., watch both VFS events and file watcher events), but VfsTask itself is a single-source task.

### Backpressure Handling

If events arrive faster than the consumer can process them:

1. **ObservableFs to VfsTask**: The mpp bounded channel (default capacity: 64) provides natural backpressure. When the channel is full, `EventBroadcaster::broadcast()` uses `force_send` which drops the oldest event. This prevents unbounded memory growth but means events can be lost under extreme load.
2. **VfsTask to downstream subscribers**: VfsTask's own `EventBroadcaster` applies the same bounded-channel strategy. Slow subscribers lose oldest events.
3. **Detection**: VfsTask could optionally emit a `VfsEvent::EventsDropped { count }` synthetic event when channel overflow is detected (requires `concurrent_queue` to report dropped count, or maintain a separate counter).
4. **Tuning**: Channel capacity is configurable via `VfsTask::with_channel_capacity(cap: usize)`. Higher capacity = more memory, fewer drops. Lower capacity = less memory, more drops under load.

## Iron Rule: Valtron-Backed Tests Required

**This feature IS valtron integration. All VfsTask code paths MUST be tested through a valtron-initialized pool.**

Tests must follow the pattern in `backends/foundation_nativeapis/tests/valtron_executor_integration.rs`:

```rust
fn init_pool() -> PoolGuard { initialize_pool(42, Some(3)) }

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn test_vfs_task() {
    let _guard = init_pool();
    // ... test VfsTask through valtron executor
}
```

**No exceptions.** VfsTask is a valtron task — it cannot be meaningfully tested without the valtron executor. Direct `next_status()` calls that don't go through `execute()` are incomplete tests.

## Tasks

### VfsTask (`src/valtron/vfs_task.rs`)

- [ ] Define `VfsTask` struct: holds `Arc<SharedVfsReceiver>`, `StopSignal`, `EventBroadcaster<VfsEvent>`
- [ ] Define `VfsEventReadiness` struct: implements `EventReadiness` via channel `is_empty()` check
- [ ] Implement valtron `TaskIterator` for `VfsTask`:
  - `next_status()` checks `StopSignal` first (return `None` if stopped)
  - `try_recv()` from shared receiver: if event, broadcast to subscribers, return `Ready(event)`
  - No event: return `Depends(Arc::new(readiness.clone()))` to park the task
- [ ] Implement `VfsTask::from_observable(observable_fs)` -- subscribe to ObservableFs Broadcaster, wrap receiver
- [ ] Implement `VfsTask::subscribe()` -- returns `(Sender<VfsEvent>, Receiver<VfsEvent>)` for downstream consumers
- [ ] Implement `VfsTask::stop_signal()` -- returns `StopSignal` for clean termination
- [ ] Implement `VfsTask::with_channel_capacity(cap: usize)` -- configure bounded channel size (default 64)

### Tests

- [ ] Test: write through ObservableFs, VfsTask yields corresponding event via `collect_one()`
- [ ] Test: VfsTask integrates with valtron executor (spawn, poll, receive)
- [ ] Test: StopSignal terminates VfsTask cleanly (no hang)

## Verification

- Tests pass
- VfsTask works within valtron executor lifecycle
- `collect_one()` returns events, `collect_result()` is never used (infinite task)
- Integrates with existing Broadcaster/mpp from foundation_nativeapis
- Backpressure: channel overflow drops oldest events, no panic or memory growth
