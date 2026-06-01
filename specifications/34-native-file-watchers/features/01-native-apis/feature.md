---
feature: "Native File Watching APIs"
description: "foundation_nativeapis crate with NativeWatcher trait and platform-specific backends (inotify, kqueue, ReadDirectoryChangesW, poll fallback)"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "large"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 27
  total: 27
  completion_percentage: 0%
---

# Feature: Native File Watching APIs

## Problem

`crates/watchers` was a thin wrapper around `notify` + `notify-debouncer-full` with:
- Config parsing (TOML/JSON) mixed with watching logic
- Command execution bolted on
- Thread-per-watcher model with blocking mpsc channels
- No reusable abstraction — just delegated to notify
- Heavy transitive deps (crossbeam, filetime, walkdir, etc.)

Need a minimal, fast, pluggable file watcher that uses native OS primitives directly.

## Solution

A new crate `backends/foundation_nativeapis/` with:

1. **`NativeWatcher` trait** — the pluggable interface
2. **Platform backends** — one per OS, gated by `cfg`
3. **Unified `WatchEvent` type** — cross-platform event representation
4. **Poll watcher** — fallback for platforms without native support or network filesystems

### Design Philosophy

- **No debouncing** — consumer decides how to coalesce events
- **No config parsing** — just watch paths and emit events
- **No internal threads** — consumer calls `poll()` when ready
- **Minimal deps** — one thin crate per platform
- **Pluggable** — new providers implement `NativeWatcher`, done

## Architecture

### Core Trait

```rust
/// A pluggable native file watcher for a specific platform.
///
/// Implementors use the native OS mechanism:
/// - Linux: inotify
/// - macOS/BSD: kqueue
/// - Windows: ReadDirectoryChangesW
/// - Fallback: polling via metadata stat
pub trait NativeWatcher: Send + Sync {
    /// Add a path to watch (file or directory).
    ///
    /// For directories, the implementation may recursively watch subdirectories
    /// or the caller must call `watch()` on each subdirectory.
    fn watch(&mut self, path: &Path, recursive: bool) -> Result<()>;

    /// Remove a previously watched path.
    fn unwatch(&mut self, path: &Path) -> Result<()>;

    /// Poll for events with an optional timeout.
    ///
    /// Returns immediately if events are available, or blocks up to `timeout`.
    /// Returns an empty Vec on timeout.
    fn poll(&mut self, timeout: Duration) -> Result<Vec<WatchEvent>>;

    /// Remove all watches and release resources.
    fn clear(&mut self) -> Result<()>;
}
```

### Event Type

```rust
/// A single file system event from any platform backend.
#[derive(Debug, Clone)]
pub struct WatchEvent {
    /// What kind of change occurred.
    pub kind: WatchEventKind,
    /// The path that changed.
    pub path: PathBuf,
}

/// The type of file system change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEventKind {
    /// A file or directory was created.
    Created,
    /// A file or directory was modified (content or metadata).
    Modified,
    /// A file or directory was removed.
    Removed,
    /// A file was renamed (from old path, to new path).
    Renamed { from: PathBuf, to: PathBuf },
}
```

### Platform Backends

### How `io-uring` Fits In

`io-uring` is the low-level crate providing the complete kernel ABI for all `IORING_OP_*` operations. We use it as a direct dependency — no extraction, no vendoring.

**Reference sources (for future exploration):**
- **mio source**: `/home/darkvoid/Boxxed/@formulas/src.rust/src.tokio/mio` — epoll/kqueue/IOCP selectors, `Poll`, `Registry`, `SourceFd`, `Waker`, networking
- **nix source**: `/home/darkvoid/Boxxed/@formulas/src.rust/src.nix-rust` — syscall wrappers, kqueue, eventfd, fcntl, poll
- **monoio source**: `/home/darkvoid/Boxxed/@formulas/src.rust/src.bytedance/monoio/monoio` — io_uring runtime patterns, opcode wrapping, `Op` trait, `SharedFd` lifecycle, slab-based task tracking, waker integration
- **tokio-uring source**: `/home/darkvoid/Boxxed/@formulas/src.rust/src.async/tokio-uring` — async file I/O, buffer ownership, fs/net abstractions on io_uring

**What we learn from monoio + tokio-uring:**
- How to wrap `opcode` structs into higher-level operation types with proper lifecycle tracking
- `SharedFd` pattern — tracking fd state across concurrent io_uring operations
- How to handle cancellation (`AsyncCancel` opcode)
- Buffer management — tokio-uring passes buffer ownership between operations
- Slab-based operation tracking for matching completions back to submissions
- Waker integration via `IORING_OP_READ` on eventfd
- Timeout handling via `IORING_OP_TIMEOUT` with ext_arg optimization (5.11+)
- Poll operations via `IORING_OP_POLL_ADD` as a replacement for epoll

**Our approach:**
Use the `io-uring` crate directly. Build our own abstractions on top that fit valtron's execution model (not async `Future`-based). Learn the patterns from monoio and tokio-uring but design for our task-driven architecture.

#### Linux — inotify via our Selector (epoll)

```
Dependencies: inotify (for inotify_event structs), our extracted epoll Selector
Usage:
  1. inotify_init1(IN_CLOEXEC) → fd
  2. inotify_add_watch(fd, path, IN_ALL_EVENTS) → watch_descriptor
  3. Register inotify fd with our Selector:
     registry.register(&mut SourceFd(&inotify_fd), TOKEN, Interest::READABLE)
  4. poll() → our Poll::poll() → epoll_wait internally
  5. When poll returns TOKEN event, read(inotify_fd) → decode inotify_event structs
  6. IN_MOVED_FROM + IN_MOVED_TO with same cookie = Renamed
  7. Clear: deregister from selector, close inotify fd

Lines of code: ~180 (our wrapper only; our selector handles epoll)
```

#### macOS / BSD — kqueue via our Selector + EVFILT_VNODE

```
Dependencies: libc (via workspace)
Usage:
  1. Our Poll::new() → creates kqueue internally
  2. For each watched path: open(path) → file_fd
  3. EV_SET with EVFILT_VNODE (our selector uses EVFILT_READ/WRITE for sockets,
     but we add EVFILT_VNODE directly for file watching):
     EV_SET(&kevent, file_fd, EVFILT_VNODE, EV_ADD|EV_CLEAR,
            NOTE_WRITE|NOTE_DELETE|NOTE_EXTEND|NOTE_RENAME|NOTE_REVOKE, 0, 0)
  4. poll() → our Poll::poll() → kevent() internally
  5. Decode EVFILT_VNODE fflags → WatchEvent
  6. Clear: EV_DELETE each fd, close all fds

Lines of code: ~200
Note: Same kqueue fd backs both socket readiness (mio-style) and file watching.
      EVFILT_VNODE events are distinguished by their filter field.
Caveat: kqueue tracks by fd, not path. Must hold fd open per watched item.
        Renames require tracking — kqueue gives NOTE_RENAME but not the target.
```

#### Windows — ReadDirectoryChangesW via our Selector (IOCP)

```
Dependencies: windows-sys
Usage:
  1. CreateFile(dir, FILE_LIST_DIRECTORY, ...) → handle
  2. ReadDirectoryChangesW(handle, buffer, recursive, flags, &overlapped)
  3. Register handle with our IOCP selector
  4. poll() → our Poll::poll() → GetQueuedCompletionStatus internally
  5. Decode FILE_NOTIFY_INFORMATION → WatchEvent
  6. Clear: cancel overlapped I/O, close handle

Lines of code: ~200
Note: Our IOCP selector handles the completion port. File notification
      setup is manual via windows-sys.
```

#### Poll Fallback — all platforms

```
Dependencies: none (stdlib only)

Implementation:
  1. Walk watched paths, record metadata (mtime, size, exists)
  2. poll() sleeps for timeout duration
  3. On wake, compare current metadata to snapshot
  4. Emit Created/Modified/Removed based on diff

Lines of code: ~150
Caveat: Not real-time. Misses rapid changes. Use only as last resort.
```

### Factory Function

```rust
/// Create the best native watcher for the current platform.
///
/// Tries platform-specific backends in order:
///   1. Linux: inotify
///   2. macOS/BSD: kqueue
///   3. Windows: ReadDirectoryChangesW
///   4. Fallback: PollWatcher
pub fn native_watcher() -> Box<dyn NativeWatcher> {
    // #[cfg] gated — only the current platform's backend is compiled
}
```

### Crate Structure

```
backends/foundation_nativeapis/
├── Cargo.toml
├── src/
│   ├── lib.rs                        # public API: re-exports poll, uring, net, watcher modules
│   ├── error.rs                      # WatchError
│   ├── event.rs                      # WatchEvent, WatchEventKind (file watching events)
│   ├── poll/                         # extracted from mio — I/O readiness layer (cross-platform)
│   │   ├── mod.rs                    # Poll, Registry, Token, Interest, Events
│   │   ├── event/                    # Event type, event::Source trait
│   │   │   ├── mod.rs
│   │   │   ├── event.rs
│   │   │   ├── events.rs
│   │   │   └── source.rs
│   │   └── sys/                      # platform-specific selectors
│   │       ├── mod.rs
│   │       ├── unix/
│   │       │   ├── mod.rs
│   │       │   ├── selector/
│   │       │   │   ├── epoll.rs      # Linux/Android/illumos
│   │       │   │   └── kqueue.rs     # macOS/BSD/iOS
│   │       │   ├── sourcefd.rs       # register any raw fd
│   │       │   ├── waker/            # eventfd (Linux), kqueue EVFILT_USER (macOS), pipe (BSD)
│   │       │   └── net/              # TCP, UDP, Unix sockets
│   │       ├── windows/
│   │       │   ├── mod.rs
│   │       │   ├── selector.rs       # IOCP
│   │       │   └── net/              # TCP, UDP
│   │       └── shell/                # fallback stubs when os-poll disabled
│   ├── net/                          # re-exports from poll/sys networking
│   │   ├── mod.rs
│   │   ├── tcp/
│   │   ├── udp.rs
│   │   └── uds/
│   ├── uring/                        # io_uring abstractions (Linux only, built on io-uring crate)
│   │   ├── mod.rs                    # IoUring type, Builder, feature detection
│   │   ├── ops/                      # higher-level operation wrappers
│   │   │   ├── fs.rs                 # async file read/write/open/stat via io_uring
│   │   │   ├── net.rs                # accept/connect/send/recv via io_uring
│   │   │   ├── poll.rs               # IORING_OP_POLL_ADD (replaces epoll on Linux)
│   │   │   ├── splice.rs             # zero-copy splice/tee
│   │   │   ├── timer.rs              # IORING_OP_TIMEOUT
│   │   │   └── cancel.rs             # IORING_OP_ASYNC_CANCEL
│   │   ├── tracker.rs                # slab-based operation tracking (learned from monoio)
│   │   └── waker.rs                  # eventfd wakeup via IORING_OP_READ
│   ├── watcher/                      # our file watching layer
│   │   ├── mod.rs                    # NativeWatcher trait, native_watcher() factory
│   │   ├── linux/
│   │   │   └── mod.rs                # InotifyWatcher (inotify + epoll selector OR io_uring poll)
│   │   ├── unix/
│   │   │   └── mod.rs                # KqueueWatcher (EVFILT_VNODE + our kqueue selector)
│   │   ├── windows/
│   │   │   └── mod.rs                # WinWatcher (ReadDirectoryChangesW + our IOCP selector)
│   │   └── poll/
│   │       └── mod.rs                # PollWatcher (stdlib fallback)
│   └── task.rs                       # FileWatcherTask for valtron integration
└── tests/
    ├── poll_integration.rs           # selector tests (from mio)
    ├── net_integration.rs            # networking tests (from mio)
    └── watcher_integration.rs        # file watcher + valtron integration tests
```

### Cargo.toml

```toml
[package]
name = "foundation_nativeapis"
version = "0.0.1"
edition.workspace = true
# ...

[dependencies]
io-uring = "0.7"
inotify = "0.11"
tracing = "0.1"
thiserror = "2.0"
libc = "0.2"

[target.'cfg(target_os = "windows")'.dependencies]
windows-sys = { version = "0.59", features = [
    "Win32_Storage_FileSystem",
    "Win32_System_IO",
    "Win32_System_Threading",
    "Win32_Foundation",
    "Win32_Networking_WinSock",
]}
```

## Implementation Plans

### Task Breakdown

#### 1. Extract I/O Readiness Layer (from mio)
1. [ ] Create `src/poll/mod.rs` — `Poll`, `Registry`, `Token`, `Interest`, `Events` types
2. [ ] Create `src/poll/event/` — `Event` type alias per platform, `event::Source` trait
3. [ ] Create `src/poll/sys/unix/selector/epoll.rs` — Linux epoll selector
4. [ ] Create `src/poll/sys/unix/selector/kqueue.rs` — macOS/BSD kqueue selector
5. [ ] Create `src/poll/sys/unix/sourcefd.rs` — `SourceFd` for registering any raw fd
6. [ ] Create `src/poll/sys/unix/waker/` — eventfd (Linux), kqueue EVFILT_USER (macOS), pipe fallback
7. [ ] Create `src/poll/sys/windows/selector.rs` — Windows IOCP selector
8. [ ] Create `src/poll/sys/shell/` — fallback stubs when os-poll disabled
9. [ ] Create `src/net/` — re-export networking from sys layer (TcpStream, TcpListener, UdpSocket, Unix sockets)
10. [ ] Replicate mio's tests for selector, poll, networking

#### 2. Crate Scaffolding
11. [ ] Create `backends/foundation_nativeapis/` directory
12. [ ] Write `Cargo.toml` with platform-gated dependencies
13. [ ] Write `src/lib.rs` with module declarations, feature flags, re-exports
14. [ ] Write `src/error.rs` with `WatchError` enum
15. [ ] Write `src/event.rs` with `WatchEvent` and `WatchEventKind`

#### 3. Linux Backend (inotify + our epoll selector)
16. [ ] Write `src/watcher/linux/mod.rs` with `InotifyWatcher`
17. [ ] Integrate inotify fd registration with our `SourceFd` + `Interest::READABLE`
18. [ ] Decode `inotify_event` → `WatchEvent` (handle rename cookies)

#### 4. macOS/BSD Backend (kqueue + EVFILT_VNODE)
19. [ ] Write `src/watcher/unix/mod.rs` with `KqueueWatcher`
20. [ ] Register `EVFILT_VNODE` into the same kqueue fd our selector owns
21. [ ] Decode `EVFILT_VNODE` fflags → `WatchEventKind`

#### 5. Windows Backend (ReadDirectoryChangesW + our IOCP selector)
22. [ ] Write `src/watcher/windows/mod.rs` with `WinWatcher`
23. [ ] Register file notification handles with our IOCP selector
24. [ ] Decode `FILE_NOTIFY_INFORMATION` → `WatchEventKind`

#### 6. Poll Fallback
25. [ ] Write `src/watcher/poll/mod.rs` with `PollWatcher`
26. [ ] Track `HashMap<PathBuf, PathSnapshot>` — diff on each poll
27. [ ] Export as fallback in `native_watcher()` factory

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| mio extraction | Direct copy into our crate | Full control, no dependency churn, expose APIs for our own use cases |
| Debouncing | None | Consumer decides. Keep the library fast and simple. |
| Recursive watching | Per-directory for inotify, recursive flag for kqueue/Win | inotify doesn't support recursive natively. Consumer can walk dirs. |
| Rename detection | Cookie matching (inotify), best-effort (kqueue) | inotify gives us the target path via cookie. kqueue doesn't. |
| Async support | None initially | valtron task provides the async integration layer. |
| Fallback | PollWatcher in stdlib only | Works everywhere, no extra deps. Slow but reliable. |
| FSEvents on macOS | Not used | Directory-level, coalesced, lazy. kqueue is precise and fast. |

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_nativeapis/Cargo.toml` | Create |
| `backends/foundation_nativeapis/src/lib.rs` | Create |
| `backends/foundation_nativeapis/src/error.rs` | Create |
| `backends/foundation_nativeapis/src/event.rs` | Create |
| `backends/foundation_nativeapis/src/poll/` | Create — extracted from mio (Poll, Registry, Selector, Events, Token, Interest, Source) |
| `backends/foundation_nativeapis/src/poll/sys/` | Create — platform selectors (epoll, kqueue, IOCP) + shell stubs |
| `backends/foundation_nativeapis/src/poll/sys/unix/sourcefd.rs` | Create |
| `backends/foundation_nativeapis/src/poll/sys/unix/waker/` | Create — eventfd, kqueue-user, pipe |
| `backends/foundation_nativeapis/src/poll/sys/windows/` | Create — IOCP selector |
| `backends/foundation_nativeapis/src/net/` | Create — networking types (TCP, UDP, Unix sockets) |
| `backends/foundation_nativeapis/src/watcher/mod.rs` | Create — NativeWatcher trait + factory |
| `backends/foundation_nativeapis/src/watcher/linux/mod.rs` | Create — InotifyWatcher |
| `backends/foundation_nativeapis/src/watcher/unix/mod.rs` | Create — KqueueWatcher |
| `backends/foundation_nativeapis/src/watcher/windows/mod.rs` | Create — WinWatcher |
| `backends/foundation_nativeapis/src/watcher/poll/mod.rs` | Create — PollWatcher fallback |
| `backends/foundation_nativeapis/src/task.rs` | Create — FileWatcherTask for valtron |
| `backends/foundation_nativeapis/tests/` | Create — replicated mio tests + watcher integration tests |
| Root `Cargo.toml` | Edit — add `foundation_nativeapis` to workspace members |

---

_Created: 2026-06-01_
