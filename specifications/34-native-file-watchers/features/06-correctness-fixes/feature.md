---
feature: "Correctness Fixes — Remove Stubs, Fix Error Handling, Implement Missing Backends, Fix File Structure"
description: "Fix all places where we silently drop errors, use placeholder fallbacks, leave stub implementations, or have broken file structure. Also implement missing valtron TaskIterator impls and create the net module."
status: "pending"
priority: "high"
depends_on: ["01-native-apis", "02-fd-management"]
estimated_effort: "medium"
created: 2026-06-02
last_updated: 2026-06-02
author: "Main Agent"
tasks:
  completed: 14
  uncompleted: 0
  total: 14
  completion_percentage: 100%
---

# Feature: Correctness Fixes — Remove Stubs, Fix Error Handling, Implement Missing Backends

## Problem

Across the codebase there are several categories of issues:

### Category A: Silent Error Dropping (FIXED)

| # | Location | Issue | Status |
|---|----------|-------|--------|
| 1 | `watcher/linux.rs:121` | `PathBuf::from("<unknown>")` fallback when wd not in map | ✅ Fixed — returns error to caller |
| 2 | `watcher/linux.rs:138` | Queue overflow (`IN_Q_OVERFLOW`) silently ignored | ✅ Fixed — returns error to caller |
| 3 | `watcher/unix.rs:169` | `continue` when vnode event fd not in map | ✅ Fixed — `tracing::warn!` logs it |

### Category B: Dead/Broken Code (FIXED)

| # | Location | Issue | Status |
|---|----------|-------|--------|
| 4 | `poll/sys/unix/selector/epoll.rs:48` | `unimplemented!()` dead code path | ✅ Deleted |
| 5 | `api.rs:97` | Non-Linux `NativeAPI::EPoll` returns `UnsupportedPlatform` | ✅ Wires up KqueueWatcher |
| 6 | `task/` directory | Empty directory, stale | ✅ Deleted |
| 7 | `task_fd.rs` (2 lines) wrapping `task_fd/` dir with one file | Pointless indirection | ✅ Flattened into `task/` |

### Category C: Missing Implementations (PARTIALLY FIXED)

| # | Location | Issue | Status |
|---|----------|-------|--------|
| 8 | `src/net/` | Entire networking module missing | ✅ Done — TcpStream, TcpListener, UdpSocket, UnixStream/Listener/Datagram |
| 9 | `src/watcher/windows.rs` | WinWatcher using ReadDirectoryChangesW not implemented | ✅ Done — CreateFile + ReadDirectoryChangesW overlapped |
| 10 | `task.rs` | `FileWatcherTask` has no `impl TaskIterator` | ✅ Done — implements TaskIterator |
| 11 | `task/fd_monitor.rs` | `FdMonitorTask` has no `impl TaskIterator` | ✅ Done — implements TaskIterator |
| 12 | `examples/file_watcher.rs` | Example missing | ✅ Done |
| 13 | `RegisteredFd<T>` AsFd impl | Missing AsFd trait impl | ✅ Done — conditional on unix + AsFd + AsRawFd |

### Category D: Missing Tests (PARTIALLY FIXED)

| # | Location | Issue | Status |
|---|----------|-------|--------|
| 14 | mio tests replicated | Spec says "Replicate mio's tests for selector, poll, networking" | ✅ Done — 31 tests total (13 poll_integration, 5 fd_registration, 12 watcher_integration, 2 doc) |

## What's Done

### Error Handling Fixes
- **InotifyWatcher**: `decode_events()` now returns `(Vec<WatchEvent>, Option<WatchError>)`. Unknown wd → error recorded and event skipped. Queue overflow → error recorded.
- **KqueueWatcher**: Unknown fd in vnode events → `tracing::warn!()` logs it instead of silently dropping.

### Dead Code Removal
- Deleted `unimplemented!()` `registry()` method from epoll Selector.

### File Structure Fixes
- Removed empty `task/` directory.
- Flattened `task_fd.rs` (2-line re-export) + `task_fd/fd_monitor.rs` into `task/fd_monitor.rs`.
- `task.rs` now contains `EventBroadcaster` and `FileWatcherTask` (with `TaskIterator`).
- `task/fd_monitor.rs` contains `FdMonitorTask` (with `TaskIterator`).

### TaskIterator Implementations
Both tasks now properly implement `foundation_core::valtron::TaskIterator`:
- **FileWatcherTask**: `TaskStatus::Ready(WatchEvent)` for events, `TaskStatus::Delayed(timeout)` for waiting
- **FdMonitorTask**: `TaskStatus::Ready(())` for readiness, `TaskStatus::Delayed(interval)` for waiting, returns `None` (terminates) on error

### Network Module
Created `src/net/` with poll-layer-integrated networking:
- `TcpStream` — connected TCP socket with `register()` for poll layer
- `TcpListener` — TCP server socket with `accept()` and `register()`
- `UdpSocket` — UDP socket with `register()`, `recv_from()`, `send_to()`
- `UnixStream`/`UnixListener`/`UnixDatagram` — Unix domain sockets (Linux/macOS)

### Example
- `examples/file_watcher.rs` — watches a directory and prints file changes

### Cargo.toml Updates
- Added `task` feature flag (depends on `watcher` + `fd`)
- Added `watcher-windows` feature flag

## What's Remaining

All 14 tasks are complete. 41 tests passing.

- 4 fd_monitor_integration tests (callback, no_callback, terminates_on_error, poll_interval)
- 5 fd_registration tests (bitmask, not_ready_when_empty, ready_after_poll, read_closed, cleared_after_read)
- 12 poll_integration tests (register, deregister, waker, multiple tokens, zero timeout, writable, multi-thread wake, concurrent registration, EINTR, TCP loopback, UDP loopback, Unix socket)
- 6 valtron_integration tests (broadcast multi-subscriber, broadcast cleanup, FileWatcherTask delivers events, handles poll error, no subscribers, unwatch)
- 12 watcher_integration tests (inotify creation/modification/deletion/rename/unwatch/clear, poll watcher creation/deletion/modification/unwatch/nonexistent, builder fallback)
- 2 doc tests

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Inotify errors | Return error alongside events, don't abort | Some events were still decoded — caller should see both |
| Unknown wd/fd | Error + continue vs return immediately | Continue processing remaining events in the buffer |
| File structure | Flatten into `task/` with fd_monitor submodule | Clear separation without pointless re-export files |
| net module | Build on top of poll layer | Reuses epoll/kqueue/IOCP, consistent with mio pattern |
| TaskIterator | Uses `TaskStatus::Delayed` not `TaskStatus::Wait` | `Wait` doesn't exist — valtron uses `Delayed(time::Duration)` |
| Windows watcher | Requires `windows-sys` with FILE_NOTIFY features | Already a conditional dep in Cargo.toml |

## File Changes Summary

| File | Action | Status |
|------|--------|--------|
| `src/watcher/linux.rs` | Fix decode_events to return (events, error) | ✅ Done |
| `src/watcher/unix.rs` | Log warning for unknown fd | ✅ Done |
| `src/poll/sys/unix/selector/epoll.rs` | Remove dead unimplemented!() | ✅ Done |
| `src/api.rs` | Wire up macOS kqueue watcher | ✅ Done |
| `src/task.rs` | Flatten, add TaskIterator impl for FileWatcherTask | ✅ Done |
| `src/task/fd_monitor.rs` | Move to task/, add TaskIterator impl | ✅ Done |
| `src/task_fd.rs` | Delete — merged into task.rs | ✅ Done |
| `src/task_fd/` | Delete — merged into task/ | ✅ Done |
| `src/net/mod.rs` | Create — networking re-exports | ✅ Done |
| `src/net/tcp.rs` | Create — TcpStream, TcpListener | ✅ Done |
| `src/net/udp.rs` | Create — UdpSocket | ✅ Done |
| `src/net/unix.rs` | Create — UnixStream, UnixListener, UnixDatagram | ✅ Done |
| `src/watcher/windows.rs` | Create — WinWatcher | ✅ Done |
| `Cargo.toml` | Add task feature flag, watcher-windows feature | ✅ Done |
| `examples/file_watcher.rs` | Create — demo | ✅ Done |
| `tests/poll_integration.rs` | Expand — add mio-style tests | ✅ Done |

---

_Created: 2026-06-02_
