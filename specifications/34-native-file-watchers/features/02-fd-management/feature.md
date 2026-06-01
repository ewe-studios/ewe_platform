---
feature: "File Descriptor Management + Valtron FD Tasks"
description: "Adapt tokio's AsyncFd patterns into foundation_nativeapis: FdRegistration for tracking readiness on any raw fd, AsyncFd wrapper for sync+async use, and valtron tasks for polling/readiness operations on arbitrary file descriptors"
status: "pending"
priority: "medium"
depends_on: ["01-native-apis"]
estimated_effort: "large"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 18
  total: 18
  completion_percentage: 0%
---

# Feature: File Descriptor Management + Valtron FD Tasks

## Problem

When working with raw file descriptors (sockets, pipes, inotify fds, signalfd, eventfd, etc.), users need to:

1. **Register them with a readiness poller** (epoll/kqueue/IOCP) — the same mechanism we already own in `poll::Selector`
2. **Track readiness state** — edge-triggered pollers only notify on state transitions, so we need to remember "fd is ready" and clear it when an operation blocks
3. **Wait for readiness** — block until the fd becomes readable/writable, with a timeout
4. **Execute I/O operations safely** — only perform reads/writes when the fd is actually ready, retrying on WouldBlock
5. **Use fds in valtron tasks** — spawn a task that monitors an fd for readiness, delivers readiness events to the valtron execution engine

Tokio's `AsyncFd` solves all of these for async Rust. We need the same capability but for our valtron task model (not `Future`-based).

## What We Learn from tokio's `AsyncFd`

### Key Patterns

**1. Ownership model**
```
AsyncFd<T: AsRawFd> {
    registration: Registration,  // tracks readiness, holds waker state
    inner: Option<T>,             // owns the IO object
}
```
The fd is registered on construction, deregistered on drop. The inner object owns the fd lifetime. `into_inner()` gives it back with deregistration.

**2. ReadyGuard pattern (must_use)**
```rust
#[must_use = "You must explicitly choose whether to clear the readiness state"]
pub struct AsyncFdReadyGuard<'a, T: AsRawFd> {
    async_fd: &'a AsyncFd<T>,
    event: Option<ReadyEvent>,  // what readiness was observed
}
```
After waiting for readiness, the guard must be explicitly handled:
- `guard.try_io(|fd| fd.read(buf))` — runs I/O, auto-clears readiness on WouldBlock
- `guard.clear_ready()` — manually clears readiness
- `guard.retain_ready()` — explicitly keep readiness asserted

This prevents the critical bug: forgetting to clear readiness after an operation, causing the fd to appear ready forever.

**3. Selective readiness clearing**
```rust
guard.clear_ready_matching(Ready::READABLE);  // only clear read readiness
```
When using combined interests (`READABLE | WRITABLE`), only the specific readiness that blocked should be cleared.

**4. Poll-based API for non-async use**
```rust
pub fn poll_read_ready<'a>(&'a self, cx: &mut Context) -> Poll<io::Result<AsyncFdReadyGuard>>;
pub fn poll_write_ready<'a>(&'a self, cx: &mut Context) -> Poll<io::Result<AsyncFdReadyGuard>>;
```
The poll API is the foundation — the async methods are built on top of it. This is exactly what we need for valtron: we poll readiness, get a guard, execute I/O.

**5. Registration with edge-triggered semantics**
The tokio `Registration` stores per-fd readiness state, tracks wakers, and clears readiness when an operation blocks. On edge-triggered systems (epoll, kqueue), this is critical — if you don't clear readiness, you'll never get another notification.

### What We Adapt

| Tokio concept | Our adaptation |
|---------------|----------------|
| `Registration` | `FdRegistration` — uses our `poll::Selector` instead of tokio's runtime |
| `AsyncFd<T>` | `RegisteredFd<T>` — same ownership model, no tokio dependency |
| `AsyncFdReadyGuard` | `ReadyGuard` — same must_use pattern, same try_io |
| `ready()`, `readable()`, `writable()` async methods | `poll_readable()`, `poll_writable()` — valtron-style polling |
| `try_io` closure | Same pattern — executes I/O, auto-clears on WouldBlock |
| `Interest::READABLE \| Interest::WRITABLE` | Reuse our existing `Interest` type from poll layer |

## Architecture

### Core Types

```rust
/// Tracks readiness state for a registered file descriptor.
///
/// Registers the fd with our poll::Selector, tracks which readiness
/// states have been observed, and manages readiness clearing.
pub struct FdRegistration {
    selector: Arc<poll::Registry>,
    token: Token,
    readiness: AtomicU8,  // bitmask of Ready flags
    /// For multi-task polling: stores the waker for read readiness
    read_waker: Mutex<Option<Waker>>,
    /// For multi-task polling: stores the waker for write readiness
    write_waker: Mutex<Option<Waker>>,
}

/// Bitmask of readiness states observed on a file descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ready(u8);

impl Ready {
    pub const READABLE: Ready = Ready(0b0001);
    pub const WRITABLE: Ready = Ready(0b0010);
    pub const EMPTY: Ready = Ready(0b0000);

    pub fn is_readable(self) -> bool;
    pub fn is_writable(self) -> bool;
    pub fn is_empty(self) -> bool;
}
```

### RegisteredFd

```rust
/// Wraps any AsRawFd type, registering it with our poll::Selector
/// and providing readiness polling + guarded I/O operations.
///
/// Adapted from tokio's AsyncFd, but works with our valtron task model
/// and our cross-platform poll::Selector (epoll/kqueue/IOCP).
pub struct RegisteredFd<T: AsRawFd> {
    registration: FdRegistration,
    inner: Option<T>,
}

impl<T: AsRawFd> RegisteredFd<T> {
    /// Create a new RegisteredFd with default interest (readable + writable).
    ///
    /// The fd is immediately registered with the current poll::Selector.
    /// The fd MUST be in nonblocking mode for correct operation.
    pub fn new(inner: T) -> io::Result<Self>;

    /// Create with specific interest.
    pub fn with_interest(inner: T, interest: Interest) -> io::Result<Self>;

    /// Create, returning the original inner value on failure.
    pub fn try_new(inner: T) -> Result<Self, FdRegistrationError<T>>;

    /// Get a shared reference to the inner object.
    pub fn get_ref(&self) -> &T;

    /// Get a mutable reference to the inner object.
    pub fn get_mut(&mut self) -> &mut T;

    /// Deregister and return ownership of the inner object.
    pub fn into_inner(self) -> T;
}
```

### Poll Methods (valtron-friendly)

```rust
impl<T: AsRawFd> RegisteredFd<T> {
    /// Poll for read readiness.
    ///
    /// Returns a ReadyGuard that must be explicitly handled:
    /// - guard.try_io(|fd| inner.read(buf)) — auto-clears on WouldBlock
    /// - guard.clear_ready() — manually clear readiness
    /// - guard.retain_ready() — explicitly keep readiness
    pub fn poll_readable(&self) -> PollResult<ReadyGuard<'_, T>>;

    /// Poll for write readiness.
    pub fn poll_writable(&self) -> PollResult<ReadyGuard<'_, T>>;

    /// Poll for any of the requested readiness states.
    pub fn poll_ready(&self, interest: Interest) -> PollResult<ReadyGuard<'_, T>>;

    /// Poll-ready variant with mutable access to inner.
    pub fn poll_readable_mut(&mut self) -> PollResult<MutReadyGuard<'_, T>>;
    pub fn poll_writable_mut(&mut self) -> PollResult<MutReadyGuard<'_, T>>;
}

/// Result of a readiness poll.
pub enum PollResult<T> {
    /// The fd is ready — use the guard to perform I/O.
    Ready(T),
    /// The fd is not yet ready — caller should yield and retry.
    NotReady,
    /// An error occurred (e.g., deregistered).
    Error(io::Error),
}
```

### ReadyGuard (must_use)

```rust
/// Represents an observed readiness event on a file descriptor.
///
/// #must_use — you must explicitly choose whether to clear readiness.
/// This prevents the critical bug of forgetting to clear readiness
/// after an operation, which causes the fd to appear ready forever
/// on edge-triggered pollers.
#[must_use]
pub struct ReadyGuard<'a, T: AsRawFd> {
    fd: &'a RegisteredFd<T>,
    readiness: Ready,
}

impl<'a, T: AsRawFd> ReadyGuard<'a, T> {
    /// What readiness states were observed.
    pub fn ready(&self) -> Ready;

    /// Execute an I/O operation. If it returns WouldBlock, readiness
    /// is automatically cleared so the next poll will block again.
    pub fn try_io<R>(&mut self, f: impl FnOnce(&RegisteredFd<T>) -> io::Result<R>)
        -> Result<io::Result<R>, TryIoError>;

    /// Manually clear all readiness flags.
    /// Call this when your I/O operation blocks.
    pub fn clear_ready(&mut self);

    /// Clear only specific readiness flags.
    /// Use with combined interests — only clear what actually blocked.
    pub fn clear_ready_matching(&mut self, ready: Ready);

    /// Explicitly retain readiness (no-op, satisfies must_use).
    pub fn retain_ready(&mut self);

    /// Get reference to the inner AsyncFd.
    pub fn get_ref(&self) -> &'a RegisteredFd<T>;

    /// Get reference to the inner IO object.
    pub fn get_inner(&self) -> &'a T;
}
```

## Valtron FD Tasks

Beyond the synchronous `RegisteredFd` type, we provide valtron task types that monitor fds and deliver readiness events through valtron's execution engine.

### FD Readiness Task

```rust
/// A valtron task that monitors one or more file descriptors for readiness.
///
/// Each tick, polls registered fds for readiness and delivers events
/// to the task's readiness queue. Other tasks can subscribe to receive
/// readiness notifications.
pub struct FdMonitorTask {
    /// Registered fds being monitored
    fds: Vec<RegisteredFdEntry>,
    /// Broadcast channel for readiness events
    broadcaster: Arc<Broadcaster<FdReadinessEvent>>,
    /// Poll timeout per tick
    poll_timeout: Duration,
}

impl TaskIterator for FdMonitorTask {
    type Ready = ...;
    type Pending = ...;
    type Spawner = ...;

    fn tick(&mut self) -> ExecutionAction {
        for entry in &mut self.fds {
            match entry.fd.poll_readable() {
                PollResult::Ready(mut guard) => {
                    match guard.try_io(|fd| entry.read_buffer()) {
                        Ok(Ok(n)) => {
                            // Data read — broadcast event
                            self.broadcaster.send(FdReadinessEvent {
                                fd: entry.token,
                                kind: ReadinessKind::Readable,
                                bytes: n,
                            });
                        }
                        Ok(Err(e)) if e.kind() == WouldBlock => {
                            // Readiness was false-positive — guard clears it
                        }
                        Err(_) => {
                            // Error — remove fd from monitoring
                            entry.remove();
                        }
                    }
                }
                PollResult::NotReady => {}
                PollResult::Error(e) => {
                    tracing::error!("FD poll error: {}", e);
                }
            }
        }
        ExecutionAction::Wait(self.poll_timeout)
    }
}

/// A readiness event delivered to subscribers.
pub struct FdReadinessEvent {
    /// Which fd became ready (user-assigned token)
    pub fd: FdToken,
    /// What kind of readiness was observed
    pub kind: ReadinessKind,
    /// For readable fds: bytes read (0 if just a readiness notification)
    pub bytes: usize,
}
```

### Usage Pattern

```rust
// Monitor an inotify fd for file change events
let inotify_fd = inotify_init1(IN_CLOEXEC)?;
unsafe { libc::fcntl(inotify_fd, libc::F_SETFL, libc::O_NONBLOCK) };
inotify_add_watch(inotify_fd, "/src", IN_ALL_EVENTS)?;

let registered = RegisteredFd::with_interest(
    unsafe { FdWrapper::from_raw(inotify_fd) },
    Interest::READABLE,
)?;

// Create the monitor task
let mut monitor = FdMonitorTask::new();
monitor.add_fd(registered, Interest::READABLE)?;

// Subscribe to readiness events
let mut rx = monitor.subscribe();

// Spawn into valtron
let guard = ValtronSingleton::get_or_init(42, |pool| {
    pool.spawn::<FdMonitorTask, ...>()
        .with_resolver(Box::new(FnReady::new(|_, _| {
            while let Ok(event) = rx.try_recv() {
                if event.kind == ReadinessKind::Readable {
                    // Read actual inotify events from the fd
                    let buf = read_inotify_events(event.fd)?;
                    for ev in decode_inotify_events(&buf)? {
                        println!("File changed: {:?}", ev);
                    }
                }
            }
        })))
        .schedule()?;
});

guard.run_until_complete();
```

## Implementation Plans

### Task Breakdown

#### 1. Core FD Registration Types
1. [ ] Create `src/fd/mod.rs` — `RegisteredFd`, `FdRegistration`, `Ready`, `PollResult`
2. [ ] Create `src/fd/guard.rs` — `ReadyGuard`, `MutReadyGuard`, `TryIoError`
3. [ ] Create `src/fd/error.rs` — `FdRegistrationError<T>`, `RegistrationError`
4. [ ] Implement `FdRegistration::new()` — register fd with our `poll::Selector`
5. [ ] Implement `poll_readable()`/`poll_writable()` — check readiness bitmask, return guard
6. [ ] Implement `ReadyGuard::try_io()` — execute closure, auto-clear on WouldBlock
7. [ ] Implement `ReadyGuard::clear_ready()`/`clear_ready_matching()` — clear readiness flags
8. [ ] Implement `RegisteredFd::into_inner()` — deregister + return inner
9. [ ] Add `AsRawFd`, `AsFd` impls for `RegisteredFd<T>`

#### 2. Valtron FD Tasks
10. [ ] Create `src/task/fd_monitor.rs` — `FdMonitorTask` with `TaskIterator` impl
11. [ ] Create `src/task/fd_readiness.rs` — `FdReadinessEvent`, `ReadinessKind`, `FdToken`
12. [ ] Implement `FdMonitorTask::add_fd()` — register fd with interest
13. [ ] Implement `FdMonitorTask::subscribe()` — return broadcast receiver
14. [ ] Implement tick loop: poll each fd, execute buffered read, broadcast events

#### 3. Platform Integration
15. [ ] Linux: `FdRegistration` uses our epoll selector + `SourceFd`
16. [ ] macOS/BSD: `FdRegistration` uses our kqueue selector + `SourceFd`
17. [ ] Windows: `FdRegistration` uses our IOCP selector (overlapped I/O)
18. [ ] Wire up in `lib.rs` with feature flags, re-exports

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Readiness tracking | Atomic bitmask + mutex wakers | Simple, lock-free for the hot path (readiness check), mutex only for waker storage |
| Guard pattern | must_use + try_io auto-clear | Prevents the critical readiness-not-cleared bug that causes infinite loops |
| Mutable vs immutable guards | Both provided | `ReadyGuard` for shared access (multiple readers), `MutReadyGuard` for exclusive (single reader/writer) |
| Valtron task model | Poll-based tick, not Future | Fits valtron's execution engine — no async runtime needed |
| Buffer management | User-provided buffers | Don't own buffers — the fd wrapper just signals readiness, user decides how to read/write |

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_nativeapis/src/fd/mod.rs` | Create — RegisteredFd, FdRegistration, Ready, PollResult |
| `backends/foundation_nativeapis/src/fd/guard.rs` | Create — ReadyGuard, MutReadyGuard, TryIoError |
| `backends/foundation_nativeapis/src/fd/error.rs` | Create — FdRegistrationError, RegistrationError |
| `backends/foundation_nativeapis/src/task/fd_monitor.rs` | Create — FdMonitorTask for valtron |
| `backends/foundation_nativeapis/src/task/fd_readiness.rs` | Create — FdReadinessEvent, ReadinessKind |
| `backends/foundation_nativeapis/src/lib.rs` | Edit — export fd and task modules |
| `backends/foundation_nativeapis/src/poll/sys/unix/selector/epoll.rs` | Edit — ensure SourceFd + waker support |
| `backends/foundation_nativeapis/src/poll/sys/unix/selector/kqueue.rs` | Edit — ensure SourceFd + waker support |
| `backends/foundation_nativeapis/src/poll/sys/windows/selector.rs` | Edit — ensure IOCP waker support |

---

_Created: 2026-06-01_
