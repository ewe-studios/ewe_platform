---
feature: "File Descriptor Management + FdMonitorTask"
description: "Adapt tokio's AsyncFd patterns: FdRegistration, RegisteredFd with poll_readable/poll_writable, ReadyGuard with must_use, plus FdMonitorTask valtron convenience for arbitrary FD monitoring"
status: "pending"
priority: "medium"
depends_on: ["01-native-apis"]
estimated_effort: "large"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 15
  total: 15
  completion_percentage: 0%
---

# Feature: File Descriptor Management

## Problem

When working with raw file descriptors (sockets, pipes, inotify fds, signalfd, eventfd, etc.), users need to:

1. **Register them with a readiness poller** (epoll/kqueue/IOCP) — the same mechanism we already own in `poll::Selector`
2. **Track readiness state** — edge-triggered pollers only notify on state transitions, so we need to remember "fd is ready" and clear it when an operation blocks
3. **Poll for readiness** — check if the fd is readable/writable without blocking
4. **Execute I/O operations safely** — only perform reads/writes when the fd is actually ready, retrying on WouldBlock

Tokio's `AsyncFd` solves all of these for async Rust. We need the same capability but for our sync, task-driven model.

### Why Readiness Tracking Matters

On edge-triggered pollers (epoll with `EPOLLET`, kqueue with `EV_CLEAR`), you only get **one notification** when an fd transitions from "not ready" to "ready". If you don't read all available data, you won't get another notification until MORE data arrives.

Conversely, if you read successfully and clear the readiness state, the next poll will correctly block until new data arrives. If you forget to clear readiness after the fd is empty, every subsequent `poll()` returns immediately — the fd appears permanently ready, causing a busy-wait loop.

The `ReadyGuard` pattern solves this: you MUST explicitly choose whether to clear readiness. The `#[must_use]` attribute ensures the compiler warns you if you drop the guard without handling it.

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
    event: Option<ReadyEvent>,
}
```
After waiting for readiness, the guard must be explicitly handled:
- `guard.try_io(|fd| fd.read(buf))` — runs I/O, auto-clears readiness on WouldBlock
- `guard.clear_ready()` — manually clears readiness
- `guard.retain_ready()` — explicitly keep readiness asserted

**3. Selective readiness clearing**
```rust
guard.clear_ready_matching(Ready::READABLE);  // only clear read readiness
```
When using combined interests (`READABLE | WRITABLE`), only the specific readiness that blocked should be cleared.

**4. Registration with edge-triggered semantics**
The `Registration` stores per-fd readiness state as a bitmask. On edge-triggered systems (epoll, kqueue), this is critical — if you don't clear readiness, you'll never get another notification.

### What We Adapt

| Tokio concept | Our adaptation |
|---------------|----------------|
| `Registration` | `FdRegistration` — uses our `poll::Selector` instead of tokio's runtime |
| `AsyncFd<T>` | `RegisteredFd<T>` — same ownership model, no tokio dependency |
| `AsyncFdReadyGuard` | `ReadyGuard` — same must_use pattern, same try_io |
| `ready()`, `readable()`, `writable()` | `poll_readable()`, `poll_writable()` — sync readiness polling |
| `try_io` closure | Same pattern — executes I/O, auto-clears on WouldBlock |
| `Interest::READABLE | Interest::WRITABLE` | Reuse our existing `Interest` type from poll layer |

---

## Architecture

### Core Types

#### `Ready` — Readiness + Lifecycle Bitmask

```rust
/// Bitmask of readiness and lifecycle states observed on a file descriptor.
///
/// This is a u8 bitmask stored atomically in FdRegistration. Each bit represents
/// a state reported by the underlying poll selector.
///
/// ## Why CLOSED and ERROR flags are needed
///
/// On edge-triggered pollers, when a pipe's write end closes, epoll returns
/// `EPOLLIN | EPOLLHUP`. The `EPOLLIN` sets READABLE, but `EPOLLHUP` means
/// the connection is permanently broken — it will NEVER transition back to
/// "not ready". Without a CLOSED flag, the fd appears permanently readable,
/// causing a busy-wait loop.
///
/// Similarly, `EPOLLERR` can arrive without `EPOLLIN` or `EPOLLOUT`. Without
/// an ERROR flag, the fd would appear "not ready" when it actually has an
/// error condition that needs handling.
///
/// ## Platform mapping
/// | Flag        | epoll                    | kqueue                      | IOCP            |
/// |-------------|--------------------------|-----------------------------|-----------------|
/// | READABLE    | EPOLLIN                  | EVFILT_READ                 | read completion |
/// | WRITABLE    | EPOLLOUT                 | EVFILT_WRITE                | write completion|
/// | READ_CLOSED | EPOLLHUP or (EPOLLIN+EPOLLRDHUP) | EVFILT_READ + EV_EOF | connection closed |
/// | WRITE_CLOSED| EPOLLHUP or (EPOLLOUT+EPOLLERR)  | EVFILT_WRITE + EV_EOF | connection closed |
/// | ERROR       | EPOLLERR                 | EVFILT_READ/WRITE + EV_EOF + fflags!=0 | error completion |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ready(u8);

impl Ready {
    pub const READABLE:    Ready = Ready(0b00001);
    pub const WRITABLE:    Ready = Ready(0b00010);
    pub const READ_CLOSED: Ready = Ready(0b00100);
    pub const WRITE_CLOSED: Ready = Ready(0b01000);
    pub const ERROR:       Ready = Ready(0b10000);
    pub const EMPTY:       Ready = Ready(0b00000);

    pub fn is_readable(self)     -> bool { self.0 & Self::READABLE.0 != 0 }
    pub fn is_writable(self)     -> bool { self.0 & Self::WRITABLE.0 != 0 }
    pub fn is_read_closed(self)  -> bool { self.0 & Self::READ_CLOSED.0 != 0 }
    pub fn is_write_closed(self) -> bool { self.0 & Self::WRITE_CLOSED.0 != 0 }
    pub fn is_error(self)        -> bool { self.0 & Self::ERROR.0 != 0 }
    pub fn is_empty(self)        -> bool { self.0 == 0 }

    /// Combine readiness states.
    pub fn union(self, other: Ready) -> Ready { Ready(self.0 | other.0) }
    /// Remove specific readiness states.
    pub fn difference(self, other: Ready) -> Ready { Ready(self.0 & !other.0) }
    /// Check if this readiness contains all flags in `other`.
    pub fn contains(self, other: Ready) -> bool { self.0 & other.0 == other.0 }
}
```

#### `FdRegistration` — Per-FD Readiness Tracker

```rust
/// Tracks readiness state for a registered file descriptor.
///
/// # How readiness tracking works:
/// 1. The fd is registered with the poll::Selector via FdRegistration::new()
/// 2. When poll() runs, it queries the selector for readiness events
/// 3. For each event, the selector updates the corresponding FdRegistration's
///    readiness bitmask (atomic store with OR)
/// 4. poll_readable() checks the bitmask: if READABLE bit is set, returns a guard
/// 5. The guard's try_io() or clear_ready() clears the bitmask (atomic AND NOT)
/// 6. Next poll() will block until the fd transitions from not-ready to ready again
///
/// This is the critical loop that prevents busy-wait on edge-triggered systems.
pub struct FdRegistration {
    registry: Arc<poll::Registry>,
    token: poll::Token,
    /// Bitmask of readiness states. Updated by poll layer when selector events arrive,
    /// cleared by ReadyGuard when the user processes readiness.
    readiness: AtomicU8,
}
```

**How the readiness bitmask connects to the poll layer:**

When `poll::Poll::poll()` returns events, our code iterates through them and updates the readiness bitmasks. This is where `EPOLLHUP`, `EPOLLRDHUP`, `EPOLLERR`, and kqueue's `EV_EOF` are mapped into the `Ready` bitmask:

```rust
// This code lives in the poll layer's event dispatch:
for event in events.iter() {
    if let Some(reg) = registration_map.get(&event.token) {
        let mut new_ready = Ready::EMPTY;
        if event.is_readable()    { new_ready = new_ready.union(Ready::READABLE); }
        if event.is_writable()    { new_ready = new_ready.union(Ready::WRITABLE); }
        if event.is_read_closed() { new_ready = new_ready.union(Ready::READ_CLOSED); }
        if event.is_write_closed(){ new_ready = new_ready.union(Ready::WRITE_CLOSED); }
        if event.is_error()       { new_ready = new_ready.union(Ready::ERROR); }
        reg.readiness.fetch_or(new_ready.0, Ordering::Release);
    }
}
```

**Critical: closed/error flags are ORed in, not exclusive.** When a pipe breaks:
- epoll returns `EPOLLIN | EPOLLHUP` → both `READABLE` AND `READ_CLOSED` are set
- `poll_readable()` checks `READ_CLOSED` first → returns `Error(...)` before ever returning a guard
- This prevents the busy-wait loop where the fd appears permanently readable

The key insight: **the poll layer updates readiness, `poll_readable()` reads and clears it**. This is the handoff between the two systems.

#### `PollResult` — Result of Readiness Poll

```rust
/// Result of a readiness poll.
pub enum PollResult<T> {
    /// The fd is ready for I/O — use the guard to perform operations.
    Ready(T),
    /// The fd is not yet ready — caller should yield and retry.
    NotReady,
    /// An error occurred. This includes:
    /// - READ_CLOSED: pipe write end closed, socket peer shutdown (EOF)
    /// - WRITE_CLOSED: pipe read end closed, socket peer shutdown
    /// - ERROR: epoll error condition (EPOLLERR), socket reset
    /// - Deregistered: fd was removed from the poll selector
    Error(io::Error),
}
```

**How `poll_readable()` handles closed/error states:**

```rust
impl<T: AsRawFd> RegisteredFd<T> {
    pub fn poll_readable(&self) -> PollResult<ReadyGuard<'_, T>> {
        // Atomic load-acquire to see the latest readiness state
        let ready = Ready(self.registration.readiness.load(Ordering::Acquire));

        // 1. Check for error condition FIRST — fd has an error, cannot proceed
        if ready.is_error() {
            // Clear the error flag so next poll returns NotReady
            self.registration.readiness.fetch_and(!Ready::ERROR.0, Ordering::Release);
            return PollResult::Error(io::Error::new(
                io::ErrorKind::Other,
                "file descriptor has an error condition (EPOLLERR)",
            ));
        }

        // 2. Check for read-closed — peer shut down, pipe broken, EOF
        //    The fd is permanently in this state — no amount of waiting helps.
        if ready.is_read_closed() {
            // Clear the closed flag so next poll returns NotReady (or Error if still broken)
            self.registration.readiness.fetch_and(!Ready::READ_CLOSED.0, Ordering::Release);
            return PollResult::Error(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "read end of file descriptor is closed (EPOLLHUP / broken pipe)",
            ));
        }

        // 3. Check for readability
        if ready.is_readable() {
            // Clear the readable flag — edge-triggered: we're consuming this notification
            self.registration.readiness.fetch_and(!Ready::READABLE.0, Ordering::Release);
            return PollResult::Ready(ReadyGuard {
                fd: self,
                readiness: Ready::READABLE,
            });
        }

        // 4. No readiness detected
        PollResult::NotReady
    }

    pub fn poll_writable(&self) -> PollResult<ReadyGuard<'_, T>> {
        let ready = Ready(self.registration.readiness.load(Ordering::Acquire));

        if ready.is_error() {
            self.registration.readiness.fetch_and(!Ready::ERROR.0, Ordering::Release);
            return PollResult::Error(io::Error::new(
                io::ErrorKind::Other,
                "file descriptor has an error condition",
            ));
        }

        if ready.is_write_closed() {
            self.registration.readiness.fetch_and(!Ready::WRITE_CLOSED.0, Ordering::Release);
            return PollResult::Error(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "write end of file descriptor is closed",
            ));
        }

        if ready.is_writable() {
            self.registration.readiness.fetch_and(!Ready::WRITABLE.0, Ordering::Release);
            return PollResult::Ready(ReadyGuard {
                fd: self,
                readiness: Ready::WRITABLE,
            });
        }

        PollResult::NotReady
    }
}
```

**Why this prevents the busy-wait loop:**

When a pipe's write end closes:
1. epoll returns `EPOLLIN | EPOLLHUP`
2. Our poll layer sets both `READABLE` AND `READ_CLOSED` in the bitmask
3. `poll_readable()` checks `READ_CLOSED` **before** `READABLE` → returns `Error(...)` immediately
4. The user sees a clear error message ("read end is closed") and can exit cleanly
5. The bitmask is cleared, so subsequent polls return `NotReady` (not a perpetual `Ready`)

#### `RegisteredFd<T>` — The Main Wrapper

```rust
/// Wraps any AsRawFd type, registering it with our poll::Selector
/// and providing readiness polling + guarded I/O operations.
///
/// Adapted from tokio's AsyncFd, but works with our sync, task-driven model
/// and our cross-platform poll::Selector (epoll/kqueue/IOCP).
pub struct RegisteredFd<T: AsRawFd> {
    registration: FdRegistration,
    inner: Option<T>,
}

impl<T: AsRawFd> RegisteredFd<T> {
    /// Create a new RegisteredFd with default interest (readable + writable).
    ///
    /// The fd is immediately registered with the poll::Selector.
    /// The fd MUST be in nonblocking mode for correct operation.
    pub fn new(inner: T) -> io::Result<Self> {
        Self::with_interest(inner, Interest::READABLE | Interest::WRITABLE)
    }

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

    /// Poll for read readiness.
    ///
    /// Returns immediately with:
    /// - Ready(guard) if the fd is readable
    /// - NotReady if no data is available
    /// - Error if the fd was deregistered or closed
    ///
    /// The guard must be explicitly handled via try_io(), clear_ready(), or retain_ready().
    pub fn poll_readable(&self) -> PollResult<ReadyGuard<'_, T>>;

    /// Poll for write readiness.
    pub fn poll_writable(&self) -> PollResult<ReadyGuard<'_, T>>;

    /// Poll for any of the requested readiness states.
    pub fn poll_ready(&self, interest: Interest) -> PollResult<ReadyGuard<'_, T>>;

    /// Poll-ready variant with mutable access to inner.
    /// Use when the I/O operation requires &mut access to the inner object.
    pub fn poll_readable_mut(&mut self) -> PollResult<MutReadyGuard<'_, T>>;
    pub fn poll_writable_mut(&mut self) -> PollResult<MutReadyGuard<'_, T>>;
}
```

**Lifecycle:**

```
Construction:
  RegisteredFd::new(tcp_stream)
    1. Set fd to nonblocking (if not already)
    2. Create FdRegistration { token: Token(unique_id), readiness: AtomicU8(0) }
    3. registry.register(&mut SourceFd(fd), token, Interest::READABLE | Interest::WRITABLE)
    4. Store registration + inner

Normal usage (data flowing):
  loop {
    match fd.poll_readable() {
      PollResult::Ready(mut guard) => {
        // fd has data — read it
        guard.try_io(|registered| {
            registered.get_ref().read(&mut buf)
        })?;
      }
      PollResult::NotReady => { sleep(Duration::from_millis(50)); }
      PollResult::Error(e) => { break; }
    }
  }

Broken pipe scenario (remote closes):
  1. Remote end of pipe/socket closes
  2. epoll returns EPOLLIN | EPOLLHUP → readiness = READABLE | READ_CLOSED
  3. fd.poll_readable() → checks READ_CLOSED first → Error("read end is closed")
  4. User handles the error: logs, cleans up, exits tick loop
  5. No busy loop — the fd is permanently in error state, user decides what to do

Destruction:
  fd.into_inner() → deregisters from poll::Selector, returns the inner object
  drop(fd) → also deregisters automatically via Drop impl
```

### ReadyGuard (must_use)

```rust
/// Represents an observed readiness state on a file descriptor.
///
/// #must_use — you must explicitly choose whether to clear readiness.
/// This prevents the critical bug of forgetting to clear readiness
/// after an operation, which causes the fd to appear ready forever
/// on edge-triggered pollers.
#[must_use = "You must explicitly handle readiness via try_io(), clear_ready(), or retain_ready()"]
pub struct ReadyGuard<'a, T: AsRawFd> {
    fd: &'a RegisteredFd<T>,
    readiness: Ready,  // what readiness was observed at poll time
}

impl<'a, T: AsRawFd> ReadyGuard<'a, T> {
    /// What readiness states were observed.
    pub fn ready(&self) -> Ready;

    /// Execute an I/O operation. If it returns WouldBlock, readiness
    /// is automatically cleared so the next poll will block again.
    ///
    /// This is the primary way to use a guard. It handles the common pattern:
    /// try to read/write, if WouldBlock then clear readiness for next poll.
    pub fn try_io<R>(
        &mut self,
        f: impl FnOnce(&RegisteredFd<T>) -> io::Result<R>
    ) -> Result<io::Result<R>, TryIoError>;

    /// Manually clear all readiness flags.
    /// Call this when your I/O operation blocks or you've consumed all data.
    /// After clear_ready(), the next poll() will block until new readiness.
    pub fn clear_ready(&mut self);

    /// Clear only specific readiness flags.
    /// Use with combined interests — only clear what actually blocked.
    /// Example: if you read but couldn't write, clear only READABLE.
    pub fn clear_ready_matching(&mut self, ready: Ready);

    /// Explicitly retain readiness (no-op, satisfies must_use).
    /// Use when you've already handled the readiness and want to keep it set.
    pub fn retain_ready(&mut self);

    /// Get reference to the inner RegisteredFd.
    pub fn get_ref(&self) -> &'a RegisteredFd<T>;

    /// Get reference to the inner IO object.
    pub fn get_inner(&self) -> &'a T;
}
```

**How `try_io` works internally:**

```rust
pub fn try_io<R>(&mut self, f: impl FnOnce(&RegisteredFd<T>) -> io::Result<R>)
    -> Result<io::Result<R>, TryIoError>
{
    let result = f(self.fd);

    match &result {
        Ok(Ok(0)) if std::any::TypeId::of::<R>() == std::any::TypeId::of::<usize>() => {
            // I/O returned 0 bytes read — this is EOF (end of file / stream closed).
            // On a broken pipe, read() returns Ok(0), NOT WouldBlock.
            // We MUST clear readiness here, otherwise:
            //   1. Pipe write end closed → epoll returns EPOLLIN | EPOLLHUP
            //   2. poll_readable() sets READABLE (via EPOLLIN)
            //   3. read() returns Ok(0) — EOF
            //   4. If we don't clear readiness, next poll_readable() returns Ready again
            //   5. read() returns Ok(0) again → infinite loop
            //
            // We don't return an error — 0 is a valid read result.
            // The caller sees Ok(0) and can decide: close the fd, report EOF, etc.
            self.clear_ready();
        }
        Ok(Ok(_)) => {
            // I/O succeeded — data was consumed. Don't clear readiness yet;
            // there might be more data. The caller should try_io again until
            // WouldBlock or Ok(0) (EOF), then clear_ready is automatic.
        }
        Ok(Err(e)) if e.kind() == io::ErrorKind::WouldBlock => {
            // The fd appeared ready but is actually empty — spurious readiness.
            // This is common on edge-triggered systems: multiple consumers,
            // or the data arrived between the readiness check and the read.
            self.clear_ready();
        }
        Ok(Err(e)) => {
            // Real I/O error (connection reset, permission denied, etc.).
            // Clear readiness so the next poll can report the actual state.
            self.clear_ready();
        }
        Err(_) => {
            // Non-I/O error from the closure (e.g., encoding error).
            // Don't clear readiness — the fd might still have data.
        }
    }

    Ok(result)
}
```

**EOF handling in practice — the broken pipe scenario:**

```
Scenario: Pipe write end closes while we're monitoring the read end.

Without READ_CLOSED flag (BUG):
  1. epoll returns EPOLLIN | EPOLLHUP → READABLE set
  2. poll_readable() → Ready(guard)
  3. read() → Ok(0) (EOF)
  4. try_io: Ok(0) clears readiness (good!)
  5. BUT: epoll still has EPOLLHUP pending → next poll returns EPOLLIN again
  6. READABLE set again → poll_readable() → Ready(guard)
  7. read() → Ok(0) → clear → repeat → busy loop

With READ_CLOSED flag (FIXED):
  1. epoll returns EPOLLIN | EPOLLHUP → READABLE | READ_CLOSED set
  2. poll_readable() checks READ_CLOSED FIRST → Error("read end is closed")
  3. User sees clear error, exits cleanly
  4. No busy loop
```

### MutReadyGuard

Same as `ReadyGuard` but holds `&'a mut RegisteredFd<T>`. Needed when the I/O operation requires mutable access to the inner object (e.g., reading into a buffer stored on the task).

```rust
#[must_use = "You must explicitly handle readiness via try_io(), clear_ready(), or retain_ready()"]
pub struct MutReadyGuard<'a, T: AsRawFd> {
    fd: &'a mut RegisteredFd<T>,
    readiness: Ready,
}

impl<'a, T: AsRawFd> MutReadyGuard<'a, T> {
    // Same API as ReadyGuard, but try_io takes &mut RegisteredFd
    pub fn try_io<R>(
        &mut self,
        f: impl FnOnce(&mut RegisteredFd<T>) -> io::Result<R>
    ) -> Result<io::Result<R>, TryIoError>;
    // ... same clear_ready, retain_ready, etc.
}
```

### TryIoError

```rust
/// Error returned when try_io fails.
///
/// Wraps both the I/O error and information about whether readiness was cleared.
#[derive(Debug)]
pub struct TryIoError {
    /// The underlying I/O error (typically WouldBlock).
    pub error: io::Error,
    /// Whether readiness was automatically cleared.
    pub readiness_cleared: bool,
}
```

### Error Types

```rust
/// Error when FdRegistration fails.
#[derive(Error, Debug)]
pub enum FdRegistrationError<T: AsRawFd> {
    #[error("failed to register fd with poll selector: {0}")]
    Registration(#[source] io::Error),
    /// Returned by try_new — gives back the original value so caller can handle it.
    #[error("failed to register fd with poll selector: {0}")]
    Failed { error: io::Error, inner: T },
}

/// Generic registration error (without the inner type).
#[derive(Error, Debug)]
pub enum RegistrationError {
    #[error("failed to register fd with poll selector: {0}")]
    Registration(#[from] io::Error),
    #[error("fd is already registered with this selector")]
    AlreadyRegistered,
}
```

### AsRawFd / AsFd Implementations

```rust
impl<T: AsRawFd> AsRawFd for RegisteredFd<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_ref().unwrap().as_raw_fd()
    }
}

// If std::os::unix::io::AsFd is available (Rust 1.63+):
impl<T: AsFd> AsFd for RegisteredFd<T> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_ref().unwrap().as_fd()
    }
}
```

### Drop Implementation

```rust
impl<T: AsRawFd> Drop for RegisteredFd<T> {
    fn drop(&mut self) {
        // Deregister from the poll selector so we don't get stale events.
        // This is critical: if we drop without deregistering, the selector
        // may still have this fd registered and will return events for it,
        // causing use-after-free on the readiness AtomicU8.
        if let Err(e) = self.registration.deregister_from_selector() {
            tracing::warn!("RegisteredFd: failed to deregister on drop: {}", e);
        }
        // inner is dropped normally here (Option::drop)
    }
}
```

---

## Usage in valtron tasks

### Direct usage (user's own task)

```rust
use foundation_core::valtron::{TaskIterator, TaskStatus, BoxedSendExecutionAction};

struct MySocketTask {
    socket: RegisteredFd<TcpStream>,
    buf: [u8; 4096],
    poll_interval: Duration,
}

impl TaskIterator for MySocketTask {
    type Ready = usize;           // bytes read per tick
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.socket.poll_readable() {
            PollResult::Ready(mut guard) => {
                match guard.try_io(|fd| fd.get_ref().read(&mut self.buf)) {
                    Ok(Ok(0)) => {
                        // EOF — remote closed the connection cleanly
                        // Task terminates by returning None
                        return None;
                    }
                    Ok(Ok(n)) => {
                        // Report progress, then continue next tick
                        return Some(TaskStatus::Ready(n));
                    }
                    Ok(Err(e)) => {
                        // Non-WouldBlock error from try_io — connection reset etc.
                        tracing::error!("read error: {}", e);
                    }
                    Err(_) => {} // try_io error (non-I/O from closure)
                }
            }
            PollResult::NotReady => {}
            PollResult::Error(e) => {
                // This catches EPOLLHUP / EPOLLERR / broken pipe
                tracing::error!("FD error (broken pipe / closed): {}", e);
                return None;  // task terminates on error
            }
        }
        Some(TaskStatus::Wait(self.poll_interval))
    }
}
```

### FdMonitorTask convenience (feature 05)

For generic use cases where you just need "monitor this fd and notify me when it's readable", a convenience `FdMonitorTask` is provided as a valtron task (feature 05) that wraps `RegisteredFd` and polls it each tick.

---

## Testing Strategy

### What to Test

#### Unit Tests (in `src/`)

1. **Ready bitmask**: union, difference, contains, is_readable, is_writable
   ```rust
   assert!((Ready::READABLE | Ready::WRITABLE).contains(Ready::READABLE));
   assert!(!Ready::READABLE.contains(Ready::WRITABLE));
   assert_eq!(Ready::WRITABLE.difference(Ready::READABLE), Ready::WRITABLE);
   ```

2. **PollResult**: exhaustiveness, pattern matching ergonomics

3. **ReadyGuard try_io auto-clear**: When closure returns WouldBlock, readiness is cleared. When closure succeeds, readiness is NOT cleared (caller may have more data). When closure returns Ok(0) (EOF), readiness IS cleared (prevents busy loop on broken pipe).

#### Integration Tests (in `tests/`)

4. **fd_registration.rs**: Full end-to-end readiness tracking
   - Create a pipe (pipe() syscall)
   - Wrap read end in RegisteredFd
   - Write to write end → poll_readable() returns Ready
   - Read all data → try_io succeeds
   - Try again → poll_readable() returns NotReady (readiness was cleared)
   - Write more → poll_readable() returns Ready again

5. **fd_lifecycle.rs**: Construction and destruction
   - new() → fd is registered (verify via poll returning NotReady, not Error)
   - into_inner() → fd is deregistered (poll returns Error)
   - drop() → fd is deregistered (no panic, no double-close)

6. **fd_edge_triggered.rs**: Edge-triggered behavior verification
   - Write 10 bytes to pipe
   - poll_readable() → Ready, read only 5 bytes (don't drain)
   - poll_readable() → NotReady (edge-triggered, no new transition)
   - Write 5 more bytes → poll_readable() → Ready (new transition)

7. **fd_multi_interest.rs**: Combined READABLE | WRITABLE
   - Register with both interests
   - poll_readable() → Ready, poll_writable() → Ready
   - clear_ready_matching(Ready::READABLE) → only read readiness cleared
   - poll_writable() still returns Ready (write readiness not cleared)

8. **fd_broken_pipe.rs**: Broken pipe / EPOLLHUP / EOF handling
   - Create a pipe, wrap read end in RegisteredFd
   - Close write end → poll_readable() returns Error(ConnectionReset), not Ready
   - Verify no busy loop: call poll_readable() 100x → all return Error or NotReady
   - Verify error message is clear: "read end of file descriptor is closed"
   - Also test: don't check READ_CLOSED → read returns Ok(0) → readiness auto-cleared → next poll returns NotReady (not a perpetual Ready)

9. **fd_eof_handling.rs**: EOF (Ok(0)) doesn't cause busy loop
   - Create socket pair
   - Shutdown write end → read end sees EOF
   - poll_readable() → Ready (if READ_CLOSED not set by platform)
   - read() → Ok(0)
   - try_io auto-clears readiness on Ok(0)
   - Next poll_readable() → NotReady (readiness was cleared, no more EPOLLIN)

### Edge Cases to Test

- **Broken pipe (EPOLLHUP)**: Close write end of pipe → poll_readable() returns Error(ConnectionReset), not Ready → no busy loop
- **EOF (Ok(0))**: Close write end → poll_readable() returns Error, OR if READ_CLOSED not set yet: read returns Ok(0) → readiness cleared, no busy loop
- **EPOLLERR alone**: Trigger error condition on fd → poll_readable() returns Error, not NotReady
- **WouldBlock spurious readiness**: fd appears ready but read() returns WouldBlock → readiness auto-cleared, next poll blocks
- **Double deregister**: deregister twice → no panic, second is no-op
- **Drop during readiness**: drop(RegisteredFd) while guard is still held → guard becomes dangling (borrow checker prevents this via lifetimes)
- **Nonblocking requirement**: RegisteredFd on a blocking fd → behavior is undefined (document this, maybe add a debug assertion that checks O_NONBLOCK flag)
- **Concurrent poll_readable**: Two threads calling poll_readable() simultaneously → one gets Ready, other gets NotReady (atomic fetch-and-clear). Document that RegisteredFd is NOT designed for concurrent readiness polling from multiple threads.
- **Socket peer shutdown**: Call shutdown(SHUT_RD) on socket peer → READ_CLOSED set, poll_readable returns Error

### How to Test

```bash
# Unit tests
cargo test -p foundation_nativeapis --lib fd

# Integration tests
cargo test -p foundation_nativeapis --test fd_registration
cargo test -p foundation_nativeapis --test fd_lifecycle
cargo test -p foundation_nativeapis --test fd_edge_triggered
cargo test -p foundation_nativeapis --test fd_multi_interest
cargo test -p foundation_nativeapis --test fd_broken_pipe
cargo test -p foundation_nativeapis --test fd_eof_handling

# Platform-specific compilation
cargo check -p foundation_nativeapis --features "poll"
cargo check -p foundation_nativeapis --features "native-linux"
```

---

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
10. [ ] Implement `Drop` for `RegisteredFd<T>` — deregister on drop

#### 2. Platform Integration
11. [ ] Linux: `FdRegistration` uses our epoll selector + `SourceFd`
12. [ ] macOS/BSD: `FdRegistration` uses our kqueue selector + `SourceFd`
13. [ ] Windows: `FdRegistration` uses our IOCP selector (overlapped I/O)

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Readiness tracking | Atomic bitmask (AtomicU8) | Simple, lock-free for the hot path. Single byte, fits in cache line. |
| Guard pattern | must_use + try_io auto-clear | Prevents the critical readiness-not-cleared bug that causes infinite loops |
| Mutable vs immutable guards | Both provided | `ReadyGuard` for shared access (multiple readers), `MutReadyGuard` for exclusive (single reader/writer) |
| Generic FdMonitorTask | Provided as valtron task (feature 05) | Convenience for arbitrary FD monitoring. The sync API is still the reusable primitive. |
| Buffer management | User-provided buffers | Don't own buffers — the fd wrapper just signals readiness, user decides how to read/write |
| Nonblocking enforcement | Documented, not enforced at runtime | Checking O_NONBLOCK requires a fcntl syscall on every construction. Trust the user, but document clearly. |
| Thread safety | RegisteredFd is Send + Sync, but concurrent poll_readable is not recommended | The atomic bitmask handles concurrent updates from the poll layer, but concurrent readiness polling by users is not a supported pattern. |
| Broken pipe handling | READ_CLOSED flag checked BEFORE READABLE | Prevents busy-wait loop when EPOLLHUP arrives with EPOLLIN. Error returned immediately, no guard created. |
| EOF handling | try_io clears readiness on Ok(0) | read() returns 0 bytes on EOF — if readiness isn't cleared, next poll returns Ready again → busy loop. |
| Error reporting | Separate ERROR flag, checked before READABLE/WRITABLE | EPOLLERR can arrive without EPOLLIN/EPOLLOUT. Without ERROR flag, fd would appear "not ready" when it actually has an error. |

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_nativeapis/src/fd/mod.rs` | Create — RegisteredFd, FdRegistration, Ready, PollResult |
| `backends/foundation_nativeapis/src/fd/guard.rs` | Create — ReadyGuard, MutReadyGuard, TryIoError |
| `backends/foundation_nativeapis/src/fd/error.rs` | Create — FdRegistrationError, RegistrationError |
| `backends/foundation_nativeapis/src/lib.rs` | Edit — export fd module |
| `backends/foundation_nativeapis/tests/fd_registration.rs` | Create — readiness tracking integration tests |
| `backends/foundation_nativeapis/tests/fd_lifecycle.rs` | Create — construction/destruction tests |
| `backends/foundation_nativeapis/tests/fd_edge_triggered.rs` | Create — edge-triggered behavior tests |
| `backends/foundation_nativeapis/tests/fd_multi_interest.rs` | Create — combined interest tests |
| `backends/foundation_nativeapis/tests/fd_broken_pipe.rs` | Create — broken pipe / EPOLLHUP / EOF handling tests |
| `backends/foundation_nativeapis/tests/fd_eof_handling.rs` | Create — EOF (Ok(0)) readiness clearing tests |
| `backends/foundation_nativeapis/src/poll/sys/unix/selector/epoll.rs` | Edit — add is_read_closed, is_write_closed, is_error (EPOLLHUP/EPOLLRDHUP/EPOLLERR mapping) |
| `backends/foundation_nativeapis/src/poll/sys/unix/selector/kqueue.rs` | Edit — add is_read_closed, is_write_closed, is_error (EV_EOF mapping) |
| `backends/foundation_nativeapis/src/poll/sys/windows/selector.rs` | Edit — add connection closed / error mapping for IOCP |
| `backends/foundation_nativeapis/src/poll/event/event.rs` | Edit — add is_read_closed(), is_write_closed(), is_error() methods to Event |

---

_Created: 2026-06-01_
