---
feature: "Valtron Watcher Task"
description: "Thin valtron adapters: FileWatcherTask wraps NativeWatcher::poll() with TaskIterator and mpp-based subscriber queues, FdMonitorTask wraps RegisteredFd for arbitrary FD readiness monitoring"
status: "pending"
priority: "high"
depends_on: ["01-native-apis", "02-fd-management"]
estimated_effort: "small"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
---

# Feature: Valtron Watcher Task

## Problem

Even with minimal sync primitives (`NativeWatcher::poll()`, `RegisteredFd::poll_readable()`), valtron tasks need a way to integrate them into the execution engine. The old `crates/watchers` used thread-per-watcher with blocking channels — this means:

- Other tasks can't react to file changes through the valtron scheduler
- No queue-based event delivery — events go to a single handler closure
- No way for multiple subscribers to receive the same file events
- Arbitrary FD monitoring requires writing the same task boilerplate

## Solution

Two thin valtron adapters over the sync primitives:

1. **`FileWatcherTask`** — wraps `NativeWatcher`, polls via `next_status()`, sends `WatchEvent` to subscriber queues
2. **`FdMonitorTask<T>`** — wraps `RegisteredFd<T>`, polls each `next_status()` call, invokes user callback on readiness

Both are thin — the real work is done by the sync APIs. These tasks just schedule them in valtron's execution engine.

### Notification mechanism — no tokio needed

We use `foundation_core::synca::mpp` which provides `Sender<T>`/`Receiver<T>` backed by `concurrent_queue`. For multi-subscriber support, we build a simple `EventBroadcaster` that holds a `Vec<Sender<T>>` — each subscriber gets its own `Sender`, and the broadcaster clones events into each one. No tokio dependency.

```rust
use foundation_core::synca::mpp::{self, Sender, Receiver};

/// Multi-subscriber broadcaster built on top of mpp channels.
/// Each subscriber gets an independent Receiver<T>.
/// When the broadcaster sends, the event is pushed to every subscriber's queue.
pub struct EventBroadcaster<T: Clone> {
    subscribers: Vec<Sender<T>>,
    capacity: usize,
}

impl<T: Clone + Send + 'static> EventBroadcaster<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            subscribers: Vec::new(),
            capacity,
        }
    }

    /// Subscribe — returns a (Sender<T>, Receiver<T>) pair.
    /// The Sender is also returned so the subscriber can close their own channel.
    pub fn subscribe(&mut self) -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = mpp::bounded(self.capacity);
        self.subscribers.push(tx.clone());
        (tx, rx)
    }

    /// Send an event to all subscribers.
    /// Dead subscriber channels (receiver dropped) are cleaned up.
    pub fn broadcast(&mut self, event: T) {
        self.subscribers.retain(|tx| tx.send(event.clone()).is_ok());
    }

    /// Number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }
}
```

---

## Component 1: FileWatcherTask

### What It Does

Wraps a `Box<dyn NativeWatcher>` and implements `TaskIterator`. Each `next_status()` call:
1. Calls `watcher.poll(timeout)` to get file events
2. Sends events to all subscribers via `EventBroadcaster` (mpp channels)
3. Returns `Some(TaskStatus::Wait(timeout))` to yield back to valtron

### Complete API

```rust
use foundation_core::synca::mpp::{Sender, Receiver, ReceiverError};

pub struct FileWatcherTask {
    watcher: Box<dyn NativeWatcher>,
    broadcaster: EventBroadcaster<WatchEvent>,
    poll_timeout: Duration,
}

impl FileWatcherTask {
    pub fn new() -> Result<Self>;
    pub fn with_watcher(watcher: Box<dyn NativeWatcher>) -> Self;
    pub fn watch(mut self, path: &Path, recursive: bool) -> Result<Self>;
    pub fn unwatch(&mut self, path: &Path) -> Result<()>;

    /// Subscribe — returns a (Sender, Receiver) pair.
    pub fn subscribe(&mut self) -> (Sender<WatchEvent>, Receiver<WatchEvent>);

    pub fn with_poll_timeout(mut self, timeout: Duration) -> Self;
    pub fn subscriber_count(&self) -> usize;
}

impl TaskIterator for FileWatcherTask {
    type Ready = WatchEvent;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.watcher.poll(self.poll_timeout) {
            Ok(events) if !events.is_empty() => {
                let (first, rest) = events.split_first().unwrap();
                self.broadcaster.broadcast(first.clone());
                for event in rest {
                    self.broadcaster.broadcast(event.clone());
                }
                Some(TaskStatus::Ready(first.clone()))
            }
            Ok(_) => Some(TaskStatus::Delayed(self.poll_timeout)),
            Err(e) => {
                tracing::error!("Watcher poll error: {}", e);
                Some(TaskStatus::Delayed(self.poll_timeout))
            }
        }
    }
}
```

### How Subscribers Use It

```rust
use foundation_core::valtron::{execute, Stream};

let mut watcher = FileWatcherTask::new()?
    .watch("src/", true)?
    .watch("Cargo.toml", false)?;

let (tx, rx) = watcher.subscribe();
let stream = execute(watcher, None)?;

for item in stream {
    if let Stream::Next(event) = item {
        println!("File changed: {:?} ({:?})", event.path, event.kind);
    }
}
```

---

## Component 2: FdMonitorTask

### What It Does

Provides a generic valtron task for monitoring any registered file descriptor. Wraps a `RegisteredFd<T>`, polls `poll_readable()` each `next_status()` call, and invokes a user-provided callback when the fd becomes readable.

### Complete API

```rust
pub struct FdMonitorTask<T: AsRawFd> {
    fd: RegisteredFd<T>,
    callback: Option<Box<dyn FnMut(&T) -> io::Result<()>>>,
    poll_interval: Duration,
    interest: Interest,
}

impl<T: AsRawFd> FdMonitorTask<T> {
    pub fn new(fd: RegisteredFd<T>) -> Self;
    pub fn with_callback(
        mut self,
        callback: impl FnMut(&T) -> io::Result<()> + 'static,
    ) -> Self;
    pub fn with_poll_interval(mut self, interval: Duration) -> Self;
    pub fn with_interest(mut self, interest: Interest) -> Self;
    pub fn fd(&self) -> &RegisteredFd<T>;
    pub fn fd_mut(&mut self) -> &mut RegisteredFd<T>;
}

impl<T: AsRawFd> TaskIterator for FdMonitorTask<T> {
    type Ready = ();
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let readiness = match self.interest {
            Interest::READABLE => self.fd.poll_readable(),
            Interest::WRITABLE => self.fd.poll_writable(),
            _ => self.fd.poll_ready(self.interest),
        };

        match readiness {
            PollResult::Ready(mut guard) => {
                if let Some(ref mut cb) = self.callback {
                    if let Ok(Err(e)) = guard.try_io(|fd| cb(fd.get_ref())) {
                        tracing::error!("FdMonitorTask callback I/O error: {}", e);
                    }
                }
            }
            PollResult::NotReady => Some(TaskStatus::Delayed(self.poll_interval)),
            PollResult::Error(e) => {
                tracing::error!("FdMonitorTask poll error: {}", e);
            }
        }
        Some(TaskStatus::Wait(self.poll_interval))
    }
}
```

---

## Testing Strategy

### What to Test

#### Integration Tests (in `tests/`)

1. **valtron_integration.rs**: End-to-end file watching through valtron
   - Create FileWatcherTask, subscribe, spawn into valtron
   - Touch a file in watched directory
   - Run engine for a few ticks
   - Verify event received via mpp Receiver channel
   - Test multiple subscribers receive the same events

2. **fd_monitor_integration.rs**: FdMonitorTask with a pipe
   - Create a pipe, wrap read end in RegisteredFd
   - Create FdMonitorTask with callback that reads data
   - Spawn into valtron
   - Write to pipe's write end
   - Verify callback is invoked and data is read
   - Verify poll interval respected (callback not called before write)

### Edge Cases to Test

- **No subscribers**: FileWatcherTask polls and broadcasts, but no one receives → no panic, no memory leak
- **Subscriber disconnect**: Subscriber drops its Receiver → EventBroadcaster cleans up dead channel on next broadcast
- **Full subscriber queue**: Subscriber queue full (bounded channel) → EventBroadcaster uses force_send to drop oldest, or skips that subscriber
- **Callback panics**: FdMonitorTask callback panics → task continues (catch_unwind in valtron engine), doesn't take down other tasks
- **Watch error recovery**: NativeWatcher::poll returns error → task logs and continues, doesn't exit
- **FdMonitorTask with no callback**: Created without with_callback → poll returns Ready, but no callback to invoke → should be a no-op, not a panic
- **FdMonitorTask fd closure**: Underlying fd closed externally (not through RegisteredFd) → poll_readable returns Error → task logs and continues

### How to Test

```bash
cargo test -p foundation_nativeapis --test valtron_integration
cargo test -p foundation_nativeapis --test fd_monitor_integration
```

---

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Multi-subscriber | Vec of mpp::Sender<T> | Uses existing foundation_core types. No tokio dependency. |
| Channel capacity | Bounded (64) per subscriber | Prevents memory growth. force_send drops oldest on overflow. |
| Error handling | Log and continue | A poll error shouldn't kill the watcher — retry on next tick |
| Callback ownership | Box<dyn FnMut> | User can capture state. FnMut allows mutation across calls. |

## File Changes Summary

| File | Action | Status |
|------|--------|--------|
| `backends/foundation_nativeapis/src/task.rs` | Create — FileWatcherTask + EventBroadcaster + TaskIterator impl | ✅ Done |
| `backends/foundation_nativeapis/src/task/fd_monitor.rs` | Create — FdMonitorTask + TaskIterator impl | ✅ Done |
| `backends/foundation_nativeapis/Cargo.toml` | Edit — depends on foundation_core for mpp, task feature | ✅ Done |
| `backends/foundation_nativeapis/src/lib.rs` | Edit — export task module | ✅ Done |
| `backends/foundation_nativeapis/tests/valtron_integration.rs` | Create — 6 integration tests | ✅ Done |
| `backends/foundation_nativeapis/tests/fd_monitor_integration.rs` | Create — 4 integration tests | ✅ Done |

---

_Created: 2026-06-01_
