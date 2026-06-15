---
description: "Add cross-platform signal handling to foundation_nativeapis — SIGINT, SIGTERM, SIGHUP, SIGQUIT via eventfd/kqueue/Windows console events, exposed as valtron-compatible TaskIterator"
status: "complete"
priority: "high"
created: "2026-06-15"
author: "Main Agent"
metadata:
  version: "1.0"
  tags:
    - signal-handling
    - sigint
    - sigterm
    - cross-platform
    - valtron
    - foundation_nativeapis
---

# Spec 45 — Signal Handling for foundation_nativeapis

## Overview

Add a `signal` module to `foundation_nativeapis` that provides cross-platform OS signal
reception (SIGINT, SIGTERM, SIGHUP, SIGQUIT) as a **valtron `TaskIterator`**. This lets any
crate register for signal notifications without pulling in `tokio`, `ctrlc`, or platform-specific
code. The signal task uses the same `Depends(EventReadiness)` pattern as `FileWatcherTask` —
zero CPU spinning, wakes only when a signal arrives.

**Feature flag:** `signal` — enabled by default. Users who only need signal handling can do
`default-features = false, features = ["signal"]`.

## Why

Currently each crate rolls its own signal handling:
- `bin/platform` uses `ctrlc::set_handler` + `OnSignal` — manual, not valtron-integrated
- Devserver used `tokio::sync::broadcast` cancel channels — async-only
- No cross-platform abstraction for SIGHUP (reload config) or SIGTERM (graceful shutdown)
- Windows has no SIGINT/SIGTERM equivalent — uses `SetConsoleCtrlHandler` with different semantics

A single `SignalTask` in `foundation_nativeapis` solves all of this:
- Cross-platform: inotify eventfd (Linux), kqueue EVFILT_SIGNAL (macOS), SetConsoleCtrlHandler (Windows)
- Valtron-compatible: implements `TaskIterator`, yields `SignalEvent` on arrival
- Multiple subscribers: `SignalBus` broadcasts to all listeners
- No dependencies beyond libc/windows-sys (already in the crate)

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                     SignalTask                                 │
│                                                               │
│  Linux:    eventfd + sigaction → epoll wait                   │
│  macOS:    kqueue EVFILT_SIGNAL → kevent wait                 │
│  Windows:  SetConsoleCtrlHandler → WaitForSingleObject        │
│                                                               │
│  Returns: TaskStatus::Depends(signal_readiness) when idle     │
│           TaskStatus::Ready(SignalEvent) when signal arrives   │
│                                                               │
│  SignalBus: Arc-based fan-out to multiple subscribers         │
│  └─ Each subscriber gets its own ConcurrentQueue<SignalEvent> │
└──────────────────────────────────────────────────────────────┘
```

### Signal Types

```rust
/// OS signals that can be received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    /// Ctrl+C — user interrupt (SIGINT on Unix, CTRL_C_EVENT on Windows)
    Interrupt,
    /// Termination request (SIGTERM on Unix, CTRL_CLOSE_EVENT on Windows)
    Terminate,
    /// Terminal hangup — reload config (SIGHUP, Unix only)
    Hangup,
    /// Quit with core dump (SIGQUIT, Unix only)
    Quit,
}

/// A received signal event.
#[derive(Debug, Clone)]
pub struct SignalEvent {
    pub kind: SignalKind,
    pub timestamp: std::time::Instant,
}
```

### SignalBus — fan-out to multiple subscribers

```rust
use concurrent_queue::ConcurrentQueue;
use std::sync::Arc;

pub struct SignalBus {
    subscribers: Mutex<Vec<Arc<ConcurrentQueue<SignalEvent>>>>,
}

impl SignalBus {
    pub fn new() -> Self { ... }

    /// Subscribe — returns a queue that receives all signal events.
    pub fn subscribe(&self) -> Arc<ConcurrentQueue<SignalEvent>> { ... }

    /// Internal: deliver a signal to all subscribers.
    fn deliver(&self, event: SignalEvent) { ... }
}
```

### SignalTask — valtron TaskIterator

```rust
use foundation_core::valtron::{TaskIterator, TaskStatus, QueueReadiness, NoSpawner};

pub struct SignalTask {
    bus: Arc<SignalBus>,
    my_queue: Arc<ConcurrentQueue<SignalEvent>>,
    my_ready: QueueReadiness<SignalEvent>,
    // Platform-specific signal handle (eventfd, kqueue fd, etc.)
    handle: SignalHandle,
}

impl TaskIterator for SignalTask {
    type Ready = SignalEvent;
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Drain received signals
        if let Ok(event) = self.my_queue.pop() {
            return Some(TaskStatus::Ready(event));
        }
        // Park until signal arrives
        Some(TaskStatus::Depends(Arc::new(self.my_ready.clone())))
    }
}
```

### Platform Implementations

#### Linux (eventfd + sigaction)

```rust
use libc::{eventfd, sigaction, sigfillset, EFD_NONBLOCK};

struct SignalHandle {
    event_fd: libc::c_int,
    old_sigint: libc::sigaction,
    old_sigterm: libc::sigaction,
}

// sigaction handler writes to eventfd — non-blocking, signal-safe
extern "C" fn signal_handler(signum: libc::c_int) {
    // Write 1 to eventfd — signal-safe (single syscall)
    let buf: u64 = 1;
    unsafe { libc::write(EVENT_FD, &buf as *const _ as *const _, 8) };
}

// poll_layer: epoll_wait on eventfd → returns ready when signal arrives
impl EventReadiness for SignalHandle {
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        // epoll_wait with timeout — blocks until eventfd is readable
        let mut events = [libc::epoll_event { events: 0, u64: 0 }];
        let timeout = dur.map(|d| d.as_millis() as i32).unwrap_or(-1);
        let count = unsafe { libc::epoll_wait(self.epoll_fd, events.as_mut_ptr(), 1, timeout) };
        count > 0
    }
}
```

#### macOS (kqueue EVFILT_SIGNAL)

```rust
use libc::{kqueue, kevent, EVFILT_SIGNAL, EV_ADD};

struct SignalHandle {
    kq_fd: libc::c_int,
}

// kqueue natively supports signal filtering — no sigaction needed
impl EventReadiness for SignalHandle {
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        let mut events = [libc::kevent { /* zeroed */ }; 1];
        let timeout = dur.map(|d| d.into()) ; // timespec
        let count = unsafe {
            libc::kevent(self.kq_fd, std::ptr::null(), 0,
                         events.as_mut_ptr(), 1,
                         timeout.as_ref().map(|t| t as *const _).unwrap_or(std::ptr::null()))
        };
        count > 0
    }
}
```

#### Windows (SetConsoleCtrlHandler)

```rust
use windows_sys::Win32::System::Console::{
    SetConsoleCtrlHandler, WaitForSingleObject, CreateEvent,
    CTRL_CLOSE_EVENT, CTRL_C_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT,
};

struct SignalHandle {
    event: HANDLE,
}

// Console handler sets the event — signal-safe (SetEvent is async-signal-safe)
unsafe extern "system" fn console_handler(ctrl_type: u32) -> i32 {
    let kind = match ctrl_type {
        CTRL_C_EVENT => SignalKind::Interrupt,
        CTRL_CLOSE_EVENT => SignalKind::Terminate,
        CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT => SignalKind::Terminate,
        _ => return 0, // not handled
    };
    SetEvent(EVENT_HANDLE);
    DELIVER_QUEUE.push(kind);
    1 // handled
}

impl EventReadiness for SignalHandle {
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        let timeout = dur.map(|d| d.as_millis() as u32).unwrap_or(INFINITE);
        let result = WaitForSingleObject(self.event, timeout);
        result == WAIT_OBJECT_0
    }
}
```

### Feature Flag Design

```toml
[features]
default = ["signal"]

# Cross-platform signal handling (SIGINT, SIGTERM, SIGHUP, SIGQUIT)
signal = ["poll"]  # requires poll layer for epoll/kqueue

# Unix-only: SIGHUP, SIGQUIT support
signal-unix = ["signal"]

# Windows-only: console control handler
signal-windows = ["signal"]
```

Users who only want signal handling:
```toml
foundation_nativeapis = { workspace = true, default-features = false, features = ["signal"] }
```

### API Surface

```rust
// Root re-exports (always available when feature = "signal")
pub use signal::{SignalKind, SignalEvent, SignalBus, SignalTask, SignalHandle};

// Convenience constructor
pub fn signal_task() -> Result<(SignalTask, Arc<SignalBus>), SignalError> {
    let bus = Arc::new(SignalBus::new());
    let task = SignalTask::new(bus.clone())?;
    Ok((task, bus))
}
```

### Usage Examples

#### DevServer shutdown on Ctrl+C

```rust
use foundation_nativeapis::signal::{signal_task, SignalKind};
use foundation_core::valtron::{self, TaskIterator};

let (mut signal_task, _bus) = signal_task()?;

valtron::run(|| {
    let engine = valtron::engine();
    valtron::execute(signal_task, None)?;

    loop {
        if let Some(status) = signal_task.next_status() {
            if let TaskStatus::Ready(event) = status {
                if matches!(event.kind, SignalKind::Interrupt | SignalKind::Terminate) {
                    tracing::info!("Received {:?}, shutting down", event.kind);
                    break;
                }
            }
        }
    }
});
```

#### Config reload on SIGHUP

```rust
let (mut signal_task, bus) = signal_task()?;
let subscriber = bus.subscribe();

// In a separate task:
loop {
    if let Ok(event) = subscriber.pop() {
        if event.kind == SignalKind::Hangup {
            reload_config()?;
        }
    }
}
```

#### Multiple signals with QueueReadiness

```rust
// SignalTask already returns Depends(QueueReadiness) — zero spinning
// Just drive it through valtron executor:
valtron::execute(signal_task, None)?;

// The task parks on epoll/kqueue/WaitForSingleObject,
// wakes only when a signal arrives, yields SignalEvent,
// then goes back to Depends.
```

## File Changes

| File | Action |
|------|--------|
| `backends/foundation_nativeapis/src/signal/mod.rs` | Create — SignalKind, SignalEvent, SignalBus, SignalTask |
| `backends/foundation_nativeapis/src/signal/linux.rs` | Create — eventfd + sigaction + epoll |
| `backends/foundation_nativeapis/src/signal/macos.rs` | Create — kqueue EVFILT_SIGNAL |
| `backends/foundation_nativeapis/src/signal/windows.rs` | Create — SetConsoleCtrlHandler + WaitForSingleObject |
| `backends/foundation_nativeapis/src/lib.rs` | Edit — add `pub mod signal` with cfg(feature = "signal") |
| `backends/foundation_nativeapis/Cargo.toml` | Edit — add `signal` feature (default), wire dependencies |

## Task Breakdown

1. [x] Define `SignalKind`, `SignalEvent`, `SignalError` in `signal/mod.rs`
2. [x] Implement `SignalBus` (fan-out to subscribers via `ConcurrentQueue`)
3. [x] Implement `SignalTask` (valtron `TaskIterator` with `Depends` parking)
4. [x] Implement Linux backend (eventfd + sigaction + epoll)
5. [x] Implement macOS backend (kqueue EVFILT_SIGNAL)
6. [x] Implement Windows backend (SetConsoleCtrlHandler + event + WaitForSingleObject)
7. [x] Add `signal` feature flag to `Cargo.toml` (default)
8. [x] Re-export from crate root when feature enabled
9. [x] Write tests: signal delivery (Unix only — send SIGUSR1 to self)
10. [x] Verify Windows compilation (no-op stubs for SIGINT/SIGTERM mapping)

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Signal handling mechanism | eventfd (Linux), kqueue (macOS), SetConsoleCtrlHandler (Windows) | Native OS APIs, no external dependencies |
| Task integration | `TaskIterator` with `Depends(EventReadiness)` | Consistent with FileWatcherTask pattern, zero spinning |
| Fan-out | `SignalBus` with `ConcurrentQueue` per subscriber | Lock-free delivery, consistent with valtron queue model |
| Default signals | SIGINT, SIGTERM on all platforms; SIGHUP/SIGQUIT Unix-only | Common denominator, no unsupported signals on Windows |
| Feature flag | `signal` (default), `default-features = false` opt-out | Users who only need signal handling don't pull in watcher/ipc/vfs |

---

_Created: 2026-06-15_
