---
feature: "Valtron Watcher Task"
description: "Thin valtron adapters: FileWatcherTask wraps NativeWatcher::poll() with TaskIterator and broadcast channels, FdMonitorTask wraps RegisteredFd for arbitrary FD readiness monitoring"
status: "pending"
priority: "high"
depends_on: ["01-native-apis", "02-fd-management"]
estimated_effort: "small"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
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

1. **`FileWatcherTask`** — wraps `NativeWatcher`, polls via `next_status()`, broadcasts `WatchEvent` to subscribers
2. **`FdMonitorTask<T>`** — wraps `RegisteredFd<T>`, polls each `next_status()` call, invokes user callback on readiness

Both are thin — the real work is done by the sync APIs. These tasks just schedule them in valtron's execution engine.

The `TaskIterator` trait requires implementing `next_status()` which returns `Option<TaskStatus<Ready, Pending, Spawner>>`. Returning `Some(TaskStatus::Wait(duration))` yields back to the executor. Returning `None` terminates the task.

---

## Component 1: FileWatcherTask

### What It Does

Wraps a `Box<dyn NativeWatcher>` and implements `TaskIterator`. Each `next_status()` call:
1. Calls `watcher.poll(timeout)` to get file events
2. Broadcasts events to all subscribers via `broadcast::Sender`
3. Returns `Some(TaskStatus::Wait(timeout))` to yield back to valtron

### How It Works — Lifecycle

```
Construction:
  FileWatcherTask::new()
    1. Create NativeWatcher via WatcherBuilder::default().build()
    2. Create broadcast::Sender<WatchEvent> (capacity 64)
    3. Set default poll_timeout = 50ms

  FileWatcherTask::new()
    .watch("src/", true)?     ← registers path with NativeWatcher
    .watch("Cargo.toml", false)?  ← registers another path

Subscription:
  let mut rx = watcher.subscribe();  ← gets broadcast::Receiver<WatchEvent>
  // Can be called multiple times — each call returns a new receiver

next_status (valtron calls this each iteration):
  fn next_status(&mut self) -> Option<TaskStatus<WatchEvent, (), BoxedSendExecutionAction>> {
    match self.watcher.poll(self.poll_timeout) {
      Ok(events) if !events.is_empty() => {
        // Broadcast all events, return first one as Ready
        let (first, rest) = events.split_first().unwrap();
        self.broadcaster.send(first.clone()).ok();
        for event in rest {
            let _ = self.broadcaster.send(event.clone());
        }
        Some(TaskStatus::Ready(first.clone()))
      }
      Ok(_) => {
        // No events — yield back to executor
        Some(TaskStatus::Wait(self.poll_timeout))
      }
      Err(e) => {
        tracing::error!("Watcher poll error: {}", e);
        // Continue polling — don't terminate on error
        Some(TaskStatus::Wait(self.poll_timeout))
      }
    }
  }

Subscriber receives:
  while let Ok(event) = rx.try_recv() {
      println!("{:?}: {:?}", event.kind, event.path);
  }

Dynamic watch management:
  watcher.watch("new/path/", true)?   // add watch at runtime
  watcher.unwatch("old/path/")?       // remove watch at runtime

Teardown:
  Task terminates by returning None when the watcher is dropped
  drop(watcher) → NativeWatcher dropped → closes inotify/kqueue fd
  All receivers get lagged errors (broadcast channel empty)
```

### Complete API

```rust
pub struct FileWatcherTask {
    watcher: Box<dyn NativeWatcher>,
    broadcaster: Arc<WatchEventBroadcaster>,
    poll_timeout: Duration,
}

impl FileWatcherTask {
    /// Create a new FileWatcherTask with platform-default native watcher.
    pub fn new() -> Result<Self>;

    /// Create with a specific NativeWatcher implementation.
    pub fn with_watcher(watcher: Box<dyn NativeWatcher>) -> Self;

    /// Add a path to watch. Returns self for chaining.
    pub fn watch(mut self, path: &Path, recursive: bool) -> Result<Self>;

    /// Remove a previously watched path.
    pub fn unwatch(&mut self, path: &Path) -> Result<()>;

    /// Subscribe to file events. Can be called multiple times.
    /// Each call returns a new receiver — all receive the same events.
    pub fn subscribe(&self) -> WatchEventReceiver;

    /// Set the poll timeout for each tick. Default: 50ms.
    pub fn with_poll_timeout(mut self, timeout: Duration) -> Self;

    /// Get the number of active subscribers.
    pub fn subscriber_count(&self) -> usize;
}

/// Broadcast channel for file events. Multiple subscribers can receive the same events.
pub type WatchEventBroadcaster = tokio::sync::broadcast::Sender<WatchEvent>;

/// A receiver for file events from a watcher.
pub type WatchEventReceiver = tokio::sync::broadcast::Receiver<WatchEvent>;

impl TaskIterator for FileWatcherTask {
    type Ready = WatchEvent;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.watcher.poll(self.poll_timeout) {
            Ok(events) if !events.is_empty() => {
                let (first, rest) = events.split_first().unwrap();
                let _ = self.broadcaster.send(first.clone());
                for event in rest {
                    let _ = self.broadcaster.send(event.clone());
                }
                Some(TaskStatus::Ready(first.clone()))
            }
            Ok(_) => {
                Some(TaskStatus::Wait(self.poll_timeout))
            }
            Err(e) => {
                tracing::error!("Watcher poll error: {}", e);
                Some(TaskStatus::Wait(self.poll_timeout))
            }
        }
    }
}
```

### How Subscribers Use It

```rust
use foundation_core::valtron::{execute, collect_result, Stream};

// 1. Create and configure the watcher task
let mut watcher = FileWatcherTask::new()?
    .watch("src/", true)?
    .watch("Cargo.toml", false)?;

// 2. Subscribe before spawning (get events from the start)
let mut rx = watcher.subscribe();

// 3. Execute into valtron
let stream = execute(watcher, None)?;

// 4. Collect results (blocks until task terminates)
let events = collect_result(stream);

// Or process events as they arrive:
for item in stream {
    if let Stream::Next(event) = item {
        println!("File changed: {:?} ({:?})", event.path, event.kind);
    }
}
```

### Subscriber Pattern — Independent Task

A subscriber task can run independently:

```rust
// Another task that rebuilds on .rs changes
let mut rx = watcher.subscribe();

for item in stream {
    if let Stream::Next(event) = item {
        if event.path.ends_with(".rs") {
            cargo_check(&event.path);
        }
    }
}
```

---

## Component 2: FdMonitorTask

### What It Does

Provides a generic valtron task for monitoring any registered file descriptor. Wraps a `RegisteredFd<T>`, polls `poll_readable()` each `next_status()` call, and invokes a user-provided callback when the fd becomes readable.

This saves users from writing the same task boilerplate:

```rust
// Without FdMonitorTask (user writes this):
struct MyInotifyTask {
    fd: RegisteredFd<FdWrapper>,
    poll_interval: Duration,
}

impl TaskIterator for MyInotifyTask {
    type Ready = usize;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.fd.poll_readable() {
            PollResult::Ready(mut guard) => {
                guard.try_io(|fd| self.handle_readiness(fd.get_ref()))?;
            }
            PollResult::NotReady => {}
            PollResult::Error(e) => {
                tracing::error!("FD error: {}", e);
                return None;  // terminate on error
            }
        }
        Some(TaskStatus::Wait(self.poll_interval))
    }
}

// With FdMonitorTask (user provides just the callback):
let monitor = FdMonitorTask::new(registered_fd)
    .with_callback(|fd| { /* handle readiness */ })
    .with_poll_interval(Duration::from_millis(50));
```

### How It Works — Lifecycle

```
Construction:
  FdMonitorTask::new(registered_fd)
    1. Store RegisteredFd<T>
    2. Set default poll_interval = 50ms
    3. No callback yet (user must set one)

  FdMonitorTask::new(fd)
    .with_callback(|fd| { /* handle readiness */ })
    .with_poll_interval(Duration::from_millis(100))

next_status (valtron calls this each iteration):
  fn next_status(&mut self) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
    match self.fd.poll_readable() {
      PollResult::Ready(mut guard) => {
        // User callback is invoked with the inner fd reference
        if let Some(ref mut cb) = self.callback {
          match guard.try_io(|fd| cb(fd.get_ref())) {
            Ok(Ok(0)) => {
              // EOF — peer closed. Readiness cleared by try_io.
              // Task logs and continues — user callback should handle EOF.
              tracing::info!("FdMonitorTask: EOF detected");
            }
            Ok(Ok(_)) => {}    // callback succeeded
            Ok(Err(e)) if e.kind() == WouldBlock => {},  // readiness was spurious
            Ok(Err(e)) => tracing::error!("FD I/O error: {}", e),
            Err(_) => {},      // try_io error (non-I/O from callback)
          }
        }
      }
      PollResult::NotReady => {}
      PollResult::Error(e) => {
        // This catches EPOLLHUP / broken pipe / EPOLLERR
        // poll_readable() returns Error immediately when READ_CLOSED or ERROR is set
        tracing::error!("FdMonitorTask poll error: {}", e);
      }
    }
    Some(TaskStatus::Wait(self.poll_interval))
  }

Teardown:
  drop(monitor) → RegisteredFd dropped → deregisters from poll selector
```

### Complete API

```rust
/// A valtron task that monitors a RegisteredFd for read readiness.
///
/// Each tick, polls the fd for readability. When ready, invokes the user-provided
/// callback with a reference to the inner IO object.
///
/// The callback receives the inner fd (via get_ref()) — not the RegisteredFd itself.
/// This means the callback can read from the fd but cannot deregister it.
pub struct FdMonitorTask<T: AsRawFd> {
    fd: RegisteredFd<T>,
    callback: Option<Box<dyn FnMut(&T) -> io::Result<()>>>,
    poll_interval: Duration,
    interest: Interest,
}

impl<T: AsRawFd> FdMonitorTask<T> {
    /// Create a new FdMonitorTask wrapping the given RegisteredFd.
    pub fn new(fd: RegisteredFd<T>) -> Self;

    /// Set the callback to invoke when the fd becomes readable.
    /// The callback receives a reference to the inner IO object.
    pub fn with_callback(
        mut self,
        callback: impl FnMut(&T) -> io::Result<()> + 'static,
    ) -> Self;

    /// Set the poll interval for each tick. Default: 50ms.
    pub fn with_poll_interval(mut self, interval: Duration) -> Self;

    /// Set the interest to poll for. Default: Interest::READABLE.
    pub fn with_interest(mut self, interest: Interest) -> Self;

    /// Get a reference to the inner RegisteredFd.
    pub fn fd(&self) -> &RegisteredFd<T>;

    /// Get a mutable reference to the inner RegisteredFd.
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
            PollResult::NotReady => {}
            PollResult::Error(e) => {
                tracing::error!("FdMonitorTask poll error: {}", e);
            }
        }
        Some(TaskStatus::Wait(self.poll_interval))
    }
}
```

### Usage Example: Monitor inotify fd

```rust
// Create raw inotify fd
let inotify_fd = inotify_init1(IN_CLOEXEC)?;
unsafe { libc::fcntl(inotify_fd, libc::F_SETFL, libc::O_NONBLOCK) };
inotify_add_watch(inotify_fd, "/src", IN_ALL_EVENTS)?;

// Wrap in RegisteredFd
let registered = RegisteredFd::with_interest(
    FdWrapper::from_raw(inotify_fd),
    Interest::READABLE,
)?;

// Create monitor task with callback
let monitor = FdMonitorTask::new(registered)
    .with_callback(|fd| {
        // Read inotify events from the fd
        let mut buf = [0u8; 4096];
        let n = fd.read(&mut buf)?;
        let events = decode_inotify_events(&buf[..n])?;
        for event in events {
            println!("File changed: {:?}", event);
        }
        Ok(())
    })
    .with_poll_interval(Duration::from_millis(50));

// Execute into valtron
let stream = execute(monitor, None)?;
for item in stream {
    // Process stream items if needed
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
   - Verify event received via subscriber channel
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
- **Subscriber lag**: Subscriber falls behind (broadcast buffer full) → receiver gets `RecvError::Lagged` → task should handle gracefully, not crash
- **Callback panics**: FdMonitorTask callback panics → task continues (catch_unwind in valtron engine), doesn't take down other tasks
- **Watch error recovery**: NativeWatcher::poll returns error → task logs and continues, doesn't exit
- **Multiple watches, one path fails**: watch("valid/") succeeds, watch("nonexistent/") fails → partial state, valid path still watched
- **Dynamic unwatch of non-existent path**: unwatch("path/never/watched") → returns NotWatched error
- **FdMonitorTask with no callback**: Created without with_callback → poll returns Ready, but no callback to invoke → should be a no-op, not a panic
- **FdMonitorTask fd closure**: Underlying fd closed externally (not through RegisteredFd) → poll_readable returns Error → task logs and continues

### How to Test

```bash
# Valtron integration tests
cargo test -p foundation_nativeapis --test valtron_integration
cargo test -p foundation_nativeapis --test fd_monitor_integration

# Feature-gated compilation
cargo check -p foundation_nativeapis --features "watcher"
cargo check -p foundation_nativeapis --features "native-linux"
```

---

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Broadcast vs MPSC | Broadcast | Multiple subscribers need the same events |
| Error handling | Log and continue | A poll error shouldn't kill the watcher — retry on next tick |
| Broadcast capacity | 64 (tokio default) | Enough for bursty file change events. Consumer can adjust if needed. |
| Callback ownership | Box<dyn FnMut> | User can capture state. FnMut allows mutation across calls. |
| FdMonitorTask generic over T | Generic | Works with any AsRawFd type — TcpStream, pipe, signalfd, etc. |
| tokio dependency | Feature-gated (`tokio/sync`) | Not all users need broadcast channels. Default could use crossbeam channel. |

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_nativeapis/src/task.rs` | Create — FileWatcherTask |
| `backends/foundation_nativeapis/src/task/fd_monitor.rs` | Create — FdMonitorTask |
| `backends/foundation_nativeapis/Cargo.toml` | Edit — add optional tokio dep for broadcast |
| `backends/foundation_nativeapis/src/lib.rs` | Edit — export task module |
| `backends/foundation_nativeapis/tests/valtron_integration.rs` | Create |
| `backends/foundation_nativeapis/tests/fd_monitor_integration.rs` | Create |

---

_Created: 2026-06-01_
